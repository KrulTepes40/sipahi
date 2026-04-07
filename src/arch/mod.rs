//! Architecture-specific support: boot, UART, PMP, CLINT, CSR, trap entry.
// Katman 0: Donanım Soyutlama — Arch
// RISC-V 64-bit, CVA6, M/S/U Mode

#[cfg(not(kani))]
use core::arch::global_asm;

#[cfg(not(kani))]
global_asm!(include_str!("boot.S"));

#[cfg(not(kani))]
global_asm!(include_str!("trap.S"));

#[cfg(not(kani))]
global_asm!(include_str!("context.S"));

pub mod uart;
pub mod csr;
pub mod trap;
pub mod clint;
pub mod pmp;
