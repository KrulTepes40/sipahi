//! RISC-V CSR (Control and Status Register) read/write helpers.
#![allow(dead_code)]
// Sipahi — CSR (Control and Status Register) İşlemleri
// Sprint 2-3: mtvec, mcause, mepc, mstatus, mie okuma/yazma
//
// NOT: Bu fonksiyonlar RISC-V asm! kullanır.
// Kani x86_64'te çalışır → #[cfg(not(kani))] ile korunur.

#[cfg(not(kani))]
use core::arch::asm;

// ═══════════════════════════════════════════════════════
// CSR Okuma
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
#[inline(always)]
pub fn read_mtvec() -> usize {
    let val: usize;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrr {}, mtvec", out(reg) val) };
    val
}

#[cfg(not(kani))]
#[inline(always)]
pub fn read_mcause() -> usize {
    let val: usize;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrr {}, mcause", out(reg) val) };
    val
}

#[cfg(not(kani))]
#[inline(always)]
pub fn read_mepc() -> usize {
    let val: usize;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrr {}, mepc", out(reg) val) };
    val
}

#[cfg(not(kani))]
#[inline(always)]
pub fn read_mstatus() -> usize {
    let val: usize;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrr {}, mstatus", out(reg) val) };
    val
}

#[cfg(not(kani))]
#[inline(always)]
pub fn read_mhartid() -> usize {
    let val: usize;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrr {}, mhartid", out(reg) val) };
    val
}

// ═══════════════════════════════════════════════════════
// CSR Yazma
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
#[inline(always)]
pub fn write_mtvec(addr: usize) {
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrw mtvec, {}", in(reg) addr) };
}

#[cfg(not(kani))]
#[inline(always)]
pub fn write_mepc(addr: usize) {
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrw mepc, {}", in(reg) addr) };
}

#[cfg(not(kani))]
#[inline(always)]
pub fn enable_machine_interrupts() {
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrsi mstatus, 0x8") };
}

#[cfg(not(kani))]
#[inline(always)]
pub fn disable_machine_interrupts() {
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrci mstatus, 0x8") };
}

#[cfg(not(kani))]
#[inline(always)]
pub fn enable_timer_interrupt() {
    let mtie: usize = 1 << 7;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe { asm!("csrs mie, {}", in(reg) mtie) };
}
