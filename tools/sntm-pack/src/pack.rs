//! ELF → per-section .bin packer body.
//!
//! SNTM design v0.8 §4.8.5: per-section ayrı .bin dosyaları üretilir:
//!   .text   → flat machine code (raw bytecode, ELF header SİLİNİR)
//!   .rodata → flat read-only data (optional, boş olabilir)
//!   .data   → flat initialized data (optional, boş olabilir)
//!   .bss    → NOLOAD (kernel boot'ta zero-fill, dump edilmez)
//!   .stack  → NOLOAD (RAM reserve)

use object::{Object, ObjectSection};
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub struct PackStats {
    pub text:   usize,
    pub rodata: usize,
    pub data:   usize,
}

pub fn pack_elf(
    elf_path:        &Path,
    out_text_path:   &Path,
    out_rodata_path: &Path,
    out_data_path:   &Path,
) -> Result<PackStats, String> {
    let bytes = fs::read(elf_path)
        .map_err(|e| format!("read {}: {}", elf_path.display(), e))?;
    let obj = object::File::parse(&*bytes)
        .map_err(|e| format!("ELF parse: {}", e))?;

    // .text MANDATORY — task entry point burada.
    let text = extract_section_required(&obj, ".text")?;
    // .rodata + .data optional — task minimal ise boş olabilir.
    let rodata = extract_section_optional(&obj, ".rodata");
    let data   = extract_section_optional(&obj, ".data");

    write_bin(out_text_path,   &text)?;
    write_bin(out_rodata_path, &rodata)?;
    write_bin(out_data_path,   &data)?;

    Ok(PackStats {
        text:   text.len(),
        rodata: rodata.len(),
        data:   data.len(),
    })
}

fn extract_section_required(obj: &object::File, name: &str) -> Result<Vec<u8>, String> {
    let section = obj.section_by_name(name)
        .ok_or_else(|| format!("section '{}' bulunamadı (mandatory)", name))?;
    let data = section.data()
        .map_err(|e| format!("section '{}' data: {}", name, e))?;
    Ok(data.to_vec())
}

fn extract_section_optional(obj: &object::File, name: &str) -> Vec<u8> {
    obj.section_by_name(name)
        .and_then(|s| s.data().ok().map(|d| d.to_vec()))
        .unwrap_or_default()
}

fn write_bin(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut f = fs::File::create(path)
        .map_err(|e| format!("create {}: {}", path.display(), e))?;
    f.write_all(data)
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(())
}
