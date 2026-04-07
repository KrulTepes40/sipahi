//! RISC-V CLINT timer driver — mtime/mtimecmp access for periodic scheduler ticks.
// Sipahi — CLINT Timer Driver (Sprint 3)
// QEMU virt: CLINT @ 0x2000000
//
// NOT: Donanım erişimi RISC-V'e özgü.
// Kani x86_64'te çalışır → #[cfg(not(kani))] ile korunur.

#[cfg(not(kani))]
use crate::common::config::{CLINT_BASE, CLINT_MTIME_OFFSET, CLINT_MTIMECMP_OFFSET, TICK_PERIOD_US};

#[cfg(not(kani))]
pub fn read_mtime() -> u64 {
    let addr = (CLINT_BASE + CLINT_MTIME_OFFSET) as *const u64;
    // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
    unsafe { core::ptr::read_volatile(addr) }
}

#[cfg(not(kani))]
pub fn write_mtimecmp(value: u64) {
    let addr = (CLINT_BASE + CLINT_MTIMECMP_OFFSET) as *mut u64;
    // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
    unsafe { core::ptr::write_volatile(addr, value) }
}

#[cfg(not(kani))]
pub fn init_timer() {
    let now = read_mtime();
    write_mtimecmp(now + ticks_per_period());
}

#[cfg(not(kani))]
pub fn schedule_next_tick() {
    let current = read_mtime();
    write_mtimecmp(current + ticks_per_period());
}

#[cfg(not(kani))]
const fn ticks_per_period() -> u64 {
    const CLINT_FREQ: u64 = 10_000_000;
    CLINT_FREQ * (TICK_PERIOD_US as u64) / 1_000_000
}
