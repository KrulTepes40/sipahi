// Sipahi — Memory Protection (Sprint 5)
// PMP ile kernel bellek bölgelerini koruma
//
// TOR (Top of Range) modu:
//   pmpaddr(i-1) = alt sınır (OFF)
//   pmpaddr(i)   = üst sınır (TOR config)
//   Bölge = [pmpaddr(i-1), pmpaddr(i))
//
// Bölge düzeni (linker script sırasıyla):
//   Entry 0-1: .text     (RX)  — kernel kodu
//   Entry 2-3: .rodata   (R)   — salt okunur veri
//   Entry 4-5: .data+bss+stack (RW) — yazılabilir veri
//   Entry 6-7: UART MMIO (RW)  — seri port
//
// Catch-all: RISC-V spec gereği PMP eşleşmeyen adresler:
//   M-mode → erişim İZİN VERİLİR (şu anki mod)
//   U-mode → erişim REDDEDİLİR (Sprint 7'de otomatik koruma)
//   Yani U-mode'da ayrı catch-all entry gerekmez.
//
// NOT: Task stack'leri şu an PMP ile korunmuyor.
// Sprint 7'de U-mode + task-bazlı PMP eklenecek:
//   Context switch'te PMP entry'leri aktif task'a göre değiştirilecek.

use crate::arch::pmp;
use crate::arch::uart;

// Linker script'ten gelen semboller
extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __bss_end: u8;
    static __stack_top: u8;
}

/// PMP bölgelerini ayarla
pub fn init_pmp() {
    let text_start = unsafe { &__text_start as *const u8 as usize };
    let text_end = unsafe { &__text_end as *const u8 as usize };
    let rodata_start = unsafe { &__rodata_start as *const u8 as usize };
    let rodata_end = unsafe { &__rodata_end as *const u8 as usize };
    let data_start = unsafe { &__data_start as *const u8 as usize };
    let stack_top = unsafe { &__stack_top as *const u8 as usize };

    const UART_START: usize = 0x1000_0000;
    const UART_END: usize = 0x1000_0100;

    // ─── PMP Adres Register'ları (linker script sırasıyla) ───
    //
    // Bellek düzeni: .text → .rodata → .data → .bss → stack
    //
    //   pmpaddr0 = text_start    (Entry 0: OFF, alt sınır)
    //   pmpaddr1 = text_end      (Entry 1: TOR RX)  → .text
    //   pmpaddr2 = rodata_start  (Entry 2: OFF, alt sınır)
    //   pmpaddr3 = rodata_end    (Entry 3: TOR R)   → .rodata
    //   pmpaddr4 = data_start    (Entry 4: OFF, alt sınır)
    //   pmpaddr5 = stack_top     (Entry 5: TOR RW)  → .data+bss+stack
    //   pmpaddr6 = UART_START    (Entry 6: OFF, alt sınır)
    //   pmpaddr7 = UART_END      (Entry 7: TOR RW)  → UART MMIO

    pmp::write_pmpaddr(0, text_start);
    pmp::write_pmpaddr(1, text_end);
    pmp::write_pmpaddr(2, rodata_start);
    pmp::write_pmpaddr(3, rodata_end);
    pmp::write_pmpaddr(4, data_start);
    pmp::write_pmpaddr(5, stack_top);
    pmp::write_pmpaddr(6, UART_START);
    pmp::write_pmpaddr(7, UART_END);

    // ─── PMP Config (pmpcfg0) ───
    let configs: [u8; 8] = [
        0,                                           // Entry 0: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X,     // Entry 1: .text RX
        0,                                           // Entry 2: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R,                   // Entry 3: .rodata R
        0,                                           // Entry 4: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W,     // Entry 5: .data+bss+stack RW
        0,                                           // Entry 6: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W,     // Entry 7: UART RW
    ];

    pmp::write_pmpcfg0(pmp::pack_pmpcfg(configs));

    // ─── Doğrulama çıktısı ───
    uart::println("[PMP] Memory protection configured:");

    uart::puts("[PMP]   .text    RX  0x");
    print_hex(text_start);
    uart::puts(" - 0x");
    print_hex(text_end);
    uart::println("");

    uart::puts("[PMP]   .rodata  R   0x");
    print_hex(rodata_start);
    uart::puts(" - 0x");
    print_hex(rodata_end);
    uart::println("");

    uart::puts("[PMP]   .data    RW  0x");
    print_hex(data_start);
    uart::puts(" - 0x");
    print_hex(stack_top);
    uart::println("");

    uart::puts("[PMP]   UART     RW  0x");
    print_hex(UART_START);
    uart::puts(" - 0x");
    print_hex(UART_END);
    uart::println("");

    uart::puts("[PMP]   pmpcfg0 = 0x");
    print_hex(pmp::read_pmpcfg0() as usize);
    uart::println("");
    uart::println("[PMP]   Catch-all: U-mode implicit DENY (RISC-V spec)");
    uart::println("[PMP]   Task stack PMP: Sprint 7 (U-mode + context switch)");
}

fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = 0;
    if val == 0 {
        uart::putc(b'0');
        return;
    }
    while val > 0 {
        buf[i] = hex[val & 0xF];
        val >>= 4;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        uart::putc(buf[i]);
    }
}
