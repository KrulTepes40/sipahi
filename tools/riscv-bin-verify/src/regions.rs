//! Symbol region boundary check — Section 8 CR-11 doctrine.
//!
//! ELF symbol filter (yalnız bunlar region check'e tabi):
//!   1. Defined ve allocated (st_shndx != UNDEF/ABS/Reserved, sh_flags & SHF_ALLOC)
//!   2. Type STT_FUNC veya STT_OBJECT (STT_FILE/STT_SECTION SKIP)
//!   3. st_value != 0 (placeholder skip)
//!   4. EDGE_SYMBOLS allowlist (`_end`/`_etext`/... boundary equality OK)
//!
//! Plus immediate `jal` kernel-range hedef detect (FIX-A scope: yalnız
//! immediate dalları, indirect `jalr` register-tracked best-effort = WARN
//! only, hâlihazırda Op::Jalr Plain'le aynı ALLOW grubunda).

use crate::decoder::{decode32, Op};
use crate::manifest::Manifest;
use crate::parser::{section_is_alloc, ParsedElf, SymSection, SymType};
use crate::{Category, Violation};

/// SAFE-3 CR-11: linker-emitted edge symbols region boundary'de izinli.
/// Bu symbol'ler genelde region.end ile birebir adres tutar (off-by-one
/// false positive guard).
const EDGE_SYMBOLS: &[&str] = &[
    "_end",
    "_etext",
    "_edata",
    "__bss_start",
    "__bss_end",
    "_data_start",
    "_data_end",
    "_rodata_start",
    "_rodata_end",
    "_sdata",
    "_sbss",
    "_estack",
    "__global_pointer$",
];

/// SAFE-3 CR-11: defined alloc symbol mı? Sadece bunlar region check'e
/// tabi tutulur.
fn should_check_symbol(parsed: &ParsedElf, sym: &crate::parser::ParsedSymbol) -> bool {
    if sym.st_value == 0 {
        return false; // placeholder / relocation pending
    }
    match &sym.st_type {
        SymType::File | SymType::Section => return false,
        _ => {}
    }
    match &sym.st_shndx {
        SymSection::Undef | SymSection::Absolute | SymSection::Common | SymSection::Reserved => {
            return false;
        }
        SymSection::Index(idx) => {
            if !section_is_alloc(parsed, *idx) {
                return false; // .debug_*, .comment etc. — runtime'da yok
            }
        }
    }
    // STT_FUNC / STT_OBJECT / STT_NOTYPE'a kadar geldik. Edge symbol skip.
    if EDGE_SYMBOLS.contains(&sym.name.as_str()) {
        return false;
    }
    true
}

pub fn check_regions(parsed: &ParsedElf, manifest: &Manifest, task_name: &str) -> Vec<Violation> {
    let mut violations = Vec::new();

    let task = match manifest.tasks.iter().find(|t| t.name == task_name) {
        Some(t) => t,
        None => {
            violations.push(Violation {
                category: Category::ParseError,
                message: format!(
                    "task '{}' not found in manifest [[task]] entries",
                    task_name
                ),
            });
            return violations;
        }
    };

    // ── Defined alloc symbol region cross-ref ──
    for sym in &parsed.symbols {
        if !should_check_symbol(parsed, sym) {
            continue;
        }
        let addr = sym.st_value as usize;
        if !task.contains_addr(addr) {
            violations.push(Violation {
                category: Category::RegionBoundary,
                message: format!(
                    "symbol '{}' at 0x{:x} outside task '{}' region map",
                    sym.name, addr, task_name
                ),
            });
        }
    }

    // ── Immediate `jal` kernel-range target reject ──
    // Section 4 FIX-A: yalnız immediate dalları; jalr register-tracked = WARN.
    let kernel_base = 0x8000_0000u64;
    let kernel_end = kernel_base + manifest.kernel.reserved_size as u64;
    for instr in &parsed.text_words {
        if instr.is_rvc { continue; }
        let op = decode32(instr.raw32, instr.abs_addr);
        if let Op::Jal { target } = op {
            if target >= kernel_base && target < kernel_end {
                violations.push(Violation {
                    category: Category::KernelRangeJal,
                    message: format!(
                        "jal at 0x{:x} targets kernel range 0x{:x} (forward-edge CFI)",
                        instr.abs_addr, target
                    ),
                });
            }
        }
    }

    violations
}
