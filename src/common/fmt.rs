//! Shared formatting helpers: print_u32, print_u64, print_hex over UART.
// Sipahi — Ortak yazdırma yardımcıları
// print_u32, print_u64, print_hex — UART üzerinden
//
// Kural: Kani derlemesinde derlenmez (uart erişimi yok)

#[cfg(not(kani))]
use crate::arch::uart;

/// Ondalık u32 yazdır.
/// U-19 GÖREV 7: defensive bound (`i < 10`) — u32 max 4_294_967_295 (10 hane)
/// olduğundan döngü garantili sonlanır. Sınır savunmacı ek katman: register
/// bozulması veya ABI ihlali halinde sonsuz döngü riski yok.
/// U-29: production build'de (debug-boot/trace OFF) hiç çağrılmaz — feature-gated
/// kullanıcılar (boot.rs task_a/task_b print) için API yüzeyi korunur.
#[cfg(not(kani))]
#[allow(dead_code)]
pub fn print_u32(mut val: u32) {
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 && i < buf.len() {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}

/// Ondalık u64 yazdır
#[cfg(not(kani))]
pub fn print_u64(mut val: u64) {
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0usize;
    while val > 0 && i < buf.len() { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}

/// Hex usize yazdır (0x prefix yok). Yalnızca debug-boot/trace gated kullanım.
/// U-19 GÖREV 7: defensive bound (`i < 16`) — usize 64-bit (max 16 hex hane).
#[cfg(not(kani))]
#[allow(dead_code)] // Production: debug-boot/trace dışında kullanılmaz
pub fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    if val == 0 { uart::putc(b'0'); return; }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while val > 0 && i < buf.len() {
        buf[i] = hex[val & 0xF];
        val >>= 4;
        i += 1;
    }
    while i > 0 { i -= 1; uart::putc(buf[i]); }
}
