//! Forbidden opcode scanner — Section 8 CR-10 doctrine.
//!
//! decoder.rs `decode32` / `decode_rvc` çıktısı `Op` enum'unu kategorize eder;
//! bu modül her instruction'ı tarayıp `Violation` listesi üretir.
//!
//! Forbidden:
//!   - Op::Ebreak (debug breakpoint — production'da olmamalı)
//!   - Op::Csr (M/S-mode CSR ops; U-mode trap ederdi ama defense-in-depth)
//!   - Op::Mret / Op::Sret / Op::Uret
//!   - Op::Wfi (M-mode only)
//!   - Op::SfenceVma / Op::SfenceWInval
//!   - Op::FloatF / Op::FloatD (RV64IMAC: F + D forbidden)
//!   - Op::CompressedFloat (RVC FP load/store)
//!
//! ALLOW:
//!   - Op::Ecall (task syscall API — CR-10 EXPLICIT exception)
//!   - Op::Plain
//!   - Op::Jal / Op::Jalr (G4 region check'te kernel-range hedef detect)

use crate::decoder::{decode32, decode_rvc, Op};
use crate::parser::ParsedElf;
use crate::{Category, Violation};

pub fn scan_forbidden_opcodes(parsed: &ParsedElf) -> Vec<Violation> {
    let mut violations = Vec::new();
    for instr in &parsed.text_words {
        let op = if instr.is_rvc {
            decode_rvc(instr.raw32 as u16)
        } else {
            decode32(instr.raw32, instr.abs_addr)
        };
        if let Some(v) = classify(op, instr.abs_addr, instr.is_rvc) {
            violations.push(v);
        }
    }
    violations
}

fn classify(op: Op, addr: u64, is_rvc: bool) -> Option<Violation> {
    let width = if is_rvc { "16-bit" } else { "32-bit" };
    let v = |cat: Category, name: &str| Violation {
        category: cat,
        message: format!("{name} at 0x{addr:x} ({width})"),
    };
    Some(match op {
        // ALLOW
        Op::Ecall | Op::Plain | Op::Jal { .. } | Op::Jalr => return None,
        // PrivilegedOp class
        Op::Ebreak       => v(Category::PrivilegedOp, "ebreak"),
        Op::Csr          => v(Category::PrivilegedOp, "csr*"),
        Op::Mret         => v(Category::PrivilegedOp, "mret"),
        Op::Sret         => v(Category::PrivilegedOp, "sret"),
        Op::Uret         => v(Category::PrivilegedOp, "uret"),
        Op::Wfi          => v(Category::PrivilegedOp, "wfi"),
        Op::SfenceVma    => v(Category::PrivilegedOp, "sfence.vma"),
        Op::SfenceWInval => v(Category::PrivilegedOp, "sfence.w.inval"),
        // FloatOp class
        Op::FloatF          => v(Category::FloatOp, "F-extension op"),
        Op::FloatD          => v(Category::FloatOp, "D-extension op"),
        Op::CompressedFloat => v(Category::FloatOp, "RVC FP op"),
        // Unknown — defansif: warn only via Plain bypass (skipped). Eğer
        // ELF malformed instr içerirse parser.rs zaten yakalar; bu noktada
        // unknown'u sessiz geç.
        Op::Unknown => return None,
    })
}
