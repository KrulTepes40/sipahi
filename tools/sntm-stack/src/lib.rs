//! sntm-stack — SAFE-4 CR-2 Plan B host stack analyzer.
//!
//! Substrate: rustc `-Z emit-stack-sizes` → ELF `.stack_sizes` PROGBITS section
//! (LLVM format: 8-byte LE address + ULEB128 frame size per function,
//! repeated). Bu tool:
//!   1. ELF parse (object crate) — `.stack_sizes`, `.symtab`, `.text` + ilgili
//!      `.rela.*` sectionları.
//!   2. Frame map (addr → name, size) — `.stack_sizes` × `.symtab` join.
//!   3. Direct call graph — `.rela.text` R_RISCV_CALL/JAL/CALL_PLT
//!      relocations.
//!   4. Indirect call detect — `.text` JALR (rd != x0) ve RVC `c.jalr` scan.
//!   5. Recursion detect — DFS over direct call graph (back-edge ⇒ cycle).
//!   6. Sum-of-frames over-approximation — analiz semantiği call-graph-aware
//!      transitive değildir; tüm fonksiyonların aynı anda yığılma worst-case'i.
//!      Raporda `caveat:` satırı ile AÇIK belirtilir (kullanıcı doktrini).
//!
//! Status semantik:
//!   PASS → max_stack_bytes = sum_of_frames; SAFE gate margin'le kıyaslar.
//!   FAIL → max_stack_bytes = 0xFFFFFFFF (UNKNOWN sentinel) + reason satırı.
//!   FAIL nedenleri: indirect call, recursion, .stack_sizes eksik, symbol
//!   çözülemedi.

pub mod analysis;
pub mod decode;
pub mod elf;
pub mod report;

pub use analysis::{analyze, AnalysisReport, Frame, Status};
pub use elf::{ElfStackInfo, ElfError};

/// SAFE-4 Section 8 CR-4: cert + sntm-validate ortak UNKNOWN sentinel.
/// max_stack_bytes == 0xFFFF_FFFF ⇒ analiz çözülemedi ⇒ DAL audit reject.
pub const UNKNOWN_SENTINEL: u32 = 0xFFFF_FFFF;

/// SAFE-4 Plan B çıktısı versiyon tag'i — golden fixture parser drift guard.
pub const REPORT_VERSION: &str = "SNTM-STACK v1.0";
