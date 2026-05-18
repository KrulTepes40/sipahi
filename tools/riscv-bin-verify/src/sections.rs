//! Section + relocation + W^X check.
//!
//! Reject classes (§17.3):
//!   - Forbidden section names: .got/.got.plt/.plt/.eh_frame/.eh_frame_hdr/
//!     .init_array/.fini_array/.ctors/.dtors
//!   - W^X violation: SHF_WRITE | SHF_EXECINSTR (writable + executable)
//!   - PIE/PIC: e_type == ET_DYN
//!   - Relocation residue: R_RISCV_RELAX leftovers (G1 scaffold; parser.rs
//!     relocations vec G1'de boş, doldurmak SAFE-4 incremental refinement
//!     veya bu sprint sonu — şu an boş skip).

use object::elf::{SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};

use crate::parser::{is_pie, ParsedElf};
use crate::{Category, Violation};

const FORBIDDEN_SECTIONS: &[&str] = &[
    ".got",
    ".got.plt",
    ".plt",
    ".eh_frame",
    ".eh_frame_hdr",
    ".init_array",
    ".fini_array",
    ".ctors",
    ".dtors",
];

pub fn check_sections(parsed: &ParsedElf) -> Vec<Violation> {
    let mut violations = Vec::new();

    // ── PIE/PIC reject ──
    if is_pie(parsed) {
        violations.push(Violation {
            category: Category::Relocation,
            message: "ELF e_type = ET_DYN (PIE/PIC); static executable required".into(),
        });
    }

    for section in &parsed.sections {
        // ── Forbidden section names ──
        if FORBIDDEN_SECTIONS.contains(&section.name.as_str()) {
            violations.push(Violation {
                category: Category::ForbiddenSection,
                message: format!(
                    "section '{}' forbidden (global init / unwinding / dynamic linking)",
                    section.name
                ),
            });
            continue; // Don't double-flag W^X on already-forbidden sections.
        }
        // ── W^X violation ──
        let alloc = section.sh_flags & (SHF_ALLOC as u64) != 0;
        let write = section.sh_flags & (SHF_WRITE as u64) != 0;
        let exec  = section.sh_flags & (SHF_EXECINSTR as u64) != 0;
        if alloc && write && exec {
            violations.push(Violation {
                category: Category::WxViolation,
                message: format!(
                    "section '{}' has SHF_WRITE + SHF_EXECINSTR (W^X violation, base=0x{:x})",
                    section.name, section.sh_addr
                ),
            });
        }
    }

    // ── Relocation residue ──
    // G3 scaffold: parser.rs şu an relocations'ı doldurmuyor (release ELF
    // genelde tamamen relocated, kalıntı yok). PIE detection üstte yapıldı
    // ki PIC binary'leri reject ediyoruz; R_RISCV_RELAX legacy linker
    // relax artefaktı için SAFE-4 refinement bekleniyor. Bu sprintte
    // pie + forbidden sections + W^X yeterli kuvvetli denetim.
    for reloc in &parsed.relocations {
        // R_RISCV_RELAX = 51, R_RISCV_TLS_*  = relocation residue / TLS
        // SAFE-3 v1.8 sade reject: type değeri 0..7 (NONE/32/64/RELATIVE/COPY/
        // JUMP_SLOT/TLS_DTPMOD/TLS_DTPREL) dışı her şey REJECT.
        if !matches!(reloc.r_type, 0..=7) {
            violations.push(Violation {
                category: Category::Relocation,
                message: format!(
                    "relocation type {} residue at offset 0x{:x} (R_RISCV_RELAX or TLS leftover)",
                    reloc.r_type, reloc.offset
                ),
            });
        }
    }

    violations
}
