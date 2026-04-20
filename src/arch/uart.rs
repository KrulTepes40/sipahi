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
    // BOUNDED: Max 1000 iterations (~3000c worst case), then drop.
    // Hard real-time: unbounded loop yasak. Karakter kaybolması UART
    // hang'ten daha güvenli. Blackbox zaten kaydediyor.
    // 1000 iter × ~3c = ~3000c = 30μs — CVA6 FPGA'da 115200 baud
    // UART FIFO (16B derinlik) drain için yeterli (~5.5μs/byte).
    // Tick period 10ms içinde %0.3 overhead — kabul edilebilir.
    unsafe {
        let lsr_addr = (UART_BASE + 5) as *const u8;
        let mut attempts: u32 = 0;
        while core::ptr::read_volatile(lsr_addr) & 0x20 == 0 {
            attempts += 1;
            if attempts >= 1000 {
                return; // drop karakter — bounded WCET
            }
        }
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
