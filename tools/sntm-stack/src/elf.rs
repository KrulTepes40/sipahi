//! ELF parser — `.stack_sizes`, `.symtab`, `.text`, `.rela.text` çıkarımı.
//!
//! Sub-workspace pattern: object crate `=0.36.5` (riscv-bin-verify ile aynı pin).

use object::elf::{R_RISCV_CALL, R_RISCV_CALL_PLT, R_RISCV_JAL};
use object::read::elf::{ElfFile64, FileHeader, SectionHeader};
use object::{Endianness, Object, ObjectSection, ObjectSymbol};

#[derive(Debug)]
pub enum ElfError {
    Parse(String),
    MissingStackSizes,
    MissingSymtab,
    NotRiscV,
    NotElf64,
    UlebOverflow,
}

impl std::fmt::Display for ElfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(s)         => write!(f, "elf parse: {}", s),
            Self::MissingStackSizes => write!(f, ".stack_sizes section eksik (build `-Z emit-stack-sizes` gerek)"),
            Self::MissingSymtab    => write!(f, ".symtab eksik (stripped binary; analiz çözülmez)"),
            Self::NotRiscV         => write!(f, "e_machine != EM_RISCV (243); RV64IMAC bekleniyor"),
            Self::NotElf64         => write!(f, "ELFCLASS64 değil"),
            Self::UlebOverflow     => write!(f, ".stack_sizes ULEB128 32-bit overflow"),
        }
    }
}

impl std::error::Error for ElfError {}

/// ELF'ten toplanan analiz girdileri.
pub struct ElfStackInfo {
    /// (function address, frame size in bytes) — `.stack_sizes` parse sonucu.
    pub frames: Vec<(u64, u32)>,
    /// (address → name) — STT_FUNC sembolleri.
    pub symbols: Vec<(u64, String)>,
    /// Direct call relocations — (from_offset_in_text, target_addr).
    /// `.rela.text` R_RISCV_CALL / R_RISCV_CALL_PLT / R_RISCV_JAL.
    pub direct_calls: Vec<(u64, u64)>,
    /// `.text` section base address ve bytes — JALR indirect scan için.
    pub text_base: u64,
    pub text_bytes: Vec<u8>,
}

const EM_RISCV: u16 = 243;
/// ELF64 SymbolType (st_info & 0xF) STT_FUNC = 2.
const STT_FUNC: u8 = 2;

pub fn parse(data: &[u8]) -> Result<ElfStackInfo, ElfError> {
    let file = object::File::parse(data).map_err(|e| ElfError::Parse(e.to_string()))?;
    if !file.is_64() {
        return Err(ElfError::NotElf64);
    }
    let machine = match &file {
        object::File::Elf64(ef) => ef.elf_header().e_machine(Endianness::Little),
        _ => return Err(ElfError::Parse("not ELF64".into())),
    };
    if machine != EM_RISCV {
        return Err(ElfError::NotRiscV);
    }

    let ef: &ElfFile64<Endianness> = match &file {
        object::File::Elf64(ef) => ef,
        _ => return Err(ElfError::Parse("not ELF64 RISC-V".into())),
    };
    let endian = ef.endian();

    let stack_sizes_section = ef
        .section_by_name(".stack_sizes")
        .ok_or(ElfError::MissingStackSizes)?;
    let stack_sizes_data = stack_sizes_section
        .data()
        .map_err(|e| ElfError::Parse(format!(".stack_sizes data: {}", e)))?;
    let frames = parse_stack_sizes(stack_sizes_data)?;

    let mut symbols: Vec<(u64, String)> = Vec::new();
    for sym in ef.symbols() {
        let kind = sym.flags();
        let st_info = match kind {
            object::SymbolFlags::Elf { st_info, .. } => st_info,
            _ => continue,
        };
        if st_info & 0xF != STT_FUNC {
            continue;
        }
        if sym.address() == 0 {
            continue;
        }
        let name = sym.name().unwrap_or("?").to_string();
        symbols.push((sym.address(), name));
    }
    symbols.sort_by_key(|(addr, _)| *addr);
    if symbols.is_empty() {
        return Err(ElfError::MissingSymtab);
    }

    let text_section = ef
        .section_by_name(".text")
        .ok_or_else(|| ElfError::Parse(".text section eksik".into()))?;
    let text_base = text_section.address();
    let text_bytes = text_section
        .data()
        .map_err(|e| ElfError::Parse(format!(".text data: {}", e)))?
        .to_vec();

    let mut direct_calls: Vec<(u64, u64)> = Vec::new();
    let rela_text = ef.section_by_name(".rela.text");
    if let Some(rela) = rela_text {
        let raw = rela
            .data()
            .map_err(|e| ElfError::Parse(format!(".rela.text data: {}", e)))?;
        let entry_size = rela.elf_section_header().sh_entsize(endian) as usize;
        if entry_size != 0 && raw.len() % entry_size == 0 {
            let mut cursor = 0;
            while cursor + entry_size <= raw.len() {
                let bytes = &raw[cursor..cursor + entry_size];
                let r_offset = read_u64(bytes, 0);
                let r_info = read_u64(bytes, 8);
                let r_type = (r_info & 0xFFFF_FFFF) as u32;
                let r_sym = (r_info >> 32) as u32;
                if matches!(r_type, R_RISCV_CALL | R_RISCV_CALL_PLT | R_RISCV_JAL) {
                    let target = resolve_symbol_addr(ef, r_sym as usize);
                    if let Some(addr) = target {
                        direct_calls.push((r_offset, addr));
                    }
                }
                cursor += entry_size;
            }
        }
    }

    Ok(ElfStackInfo {
        frames,
        symbols,
        direct_calls,
        text_base,
        text_bytes,
    })
}

fn read_u64(buf: &[u8], off: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(v)
}

fn resolve_symbol_addr(ef: &ElfFile64<Endianness>, idx: usize) -> Option<u64> {
    let mut count = 0;
    for sym in ef.symbols() {
        count += 1;
        if count == idx {
            return Some(sym.address());
        }
    }
    None
}

/// LLVM `.stack_sizes` format: per function — 8 byte LE address + ULEB128
/// frame size, repeated. Bilinmeyen padding yok (LLVM spec).
pub(crate) fn parse_stack_sizes(data: &[u8]) -> Result<Vec<(u64, u32)>, ElfError> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while cursor < data.len() {
        if cursor + 8 > data.len() {
            return Err(ElfError::Parse(format!(
                ".stack_sizes truncated address at offset {}", cursor
            )));
        }
        let addr = read_u64(data, cursor);
        cursor += 8;
        let (size, next) = read_uleb128(data, cursor)?;
        cursor = next;
        out.push((addr, size));
    }
    Ok(out)
}

fn read_uleb128(buf: &[u8], mut cursor: usize) -> Result<(u32, usize), ElfError> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        if cursor >= buf.len() {
            return Err(ElfError::Parse("ULEB128 truncated".into()));
        }
        let byte = buf[cursor];
        cursor += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if value > u32::MAX as u64 {
            return Err(ElfError::UlebOverflow);
        }
        if byte & 0x80 == 0 {
            return Ok((value as u32, cursor));
        }
        shift += 7;
        if shift >= 35 {
            return Err(ElfError::Parse("ULEB128 too long".into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb128_single_byte() {
        let (v, n) = read_uleb128(&[0x00], 0).unwrap();
        assert_eq!(v, 0); assert_eq!(n, 1);
        let (v, n) = read_uleb128(&[0x7F], 0).unwrap();
        assert_eq!(v, 127); assert_eq!(n, 1);
    }

    #[test]
    fn uleb128_two_bytes_128() {
        let (v, n) = read_uleb128(&[0x80, 0x01], 0).unwrap();
        assert_eq!(v, 128); assert_eq!(n, 2);
    }

    #[test]
    fn uleb128_overflow_rejected() {
        // 5 byte 0xFF = 5*7 = 35 bit set → > u32::MAX.
        let err = read_uleb128(&[0xFF, 0xFF, 0xFF, 0xFF, 0x1F], 0).unwrap_err();
        match err { ElfError::UlebOverflow => {}, _ => panic!("expected overflow") }
    }

    #[test]
    fn stack_sizes_parse_two_functions() {
        // addr1 = 0x80600000, size = 0
        // addr2 = 0x80600008, size = 128 (ULEB 0x80 0x01)
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x8060_0000u64.to_le_bytes());
        buf.push(0x00);
        buf.extend_from_slice(&0x8060_0008u64.to_le_bytes());
        buf.extend_from_slice(&[0x80, 0x01]);
        let v = parse_stack_sizes(&buf).unwrap();
        assert_eq!(v, vec![(0x8060_0000, 0), (0x8060_0008, 128)]);
    }

    #[test]
    fn stack_sizes_truncated_fails() {
        // 7 byte (incomplete 8-byte address).
        let buf = vec![0u8; 7];
        assert!(parse_stack_sizes(&buf).is_err());
    }
}
