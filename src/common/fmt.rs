// Sipahi — Ortak yazdırma yardımcıları
// print_u32, print_u64, print_hex — UART üzerinden
//
// Kural: Kani derlemesinde derlenmez (uart erişimi yok)

use crate::arch::uart;

/// Ondalık u32 yazdır
#[cfg(not(kani))]
pub fn print_u32(mut val: u32) {
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}

/// Ondalık u64 yazdır
#[cfg(not(kani))]
pub fn print_u64(mut val: u64) {
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0usize;
    while val > 0 && i < 20 { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}

/// Hex usize yazdır (0x prefix yok)
#[cfg(not(kani))]
pub fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while val > 0 { buf[i] = hex[val & 0xF]; val >>= 4; i += 1; }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}
