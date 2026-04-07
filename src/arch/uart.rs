//! NS16550A UART driver for QEMU virt machine — putc, puts, println.
// Sipahi — UART Driver (Sprint 1)
// QEMU virt: ns16550a @ 0x10000000
//
// NOT: Donanım erişimi → Kani'de derlenmez

#[cfg(not(kani))]
use crate::common::config::UART_BASE;

#[cfg(not(kani))]
pub fn putc(c: u8) {
    // SAFETY: MMIO register access at hardware-defined address.
    unsafe {
        core::ptr::write_volatile(UART_BASE as *mut u8, c);
    }
}

#[cfg(not(kani))]
pub fn puts(s: &str) {
    for byte in s.bytes() {
        putc(byte);
    }
}

#[cfg(not(kani))]
pub fn println(s: &str) {
    puts(s);
    putc(b'\n');
}
