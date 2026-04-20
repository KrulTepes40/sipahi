//! PMP region setup — .text RX, .rodata R, .data+bss+stack RW, UART RW.
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
// Sprint U-5: Task stacks ve WASM arena Entry 5 dışına taşındı.
// Entry 5 = [data_start, __pmp_data_end) — .data + .bss + kernel_stack
// Task stacks: .task_stacks section, per-task NAPOT Entry 8
// WASM arena: .wasm_arena section, M-mode only (U-mode deny)

use crate::arch::pmp;
use crate::arch::uart;
use crate::common::sync::SingleHartCell;

static PMP_SHADOW: SingleHartCell<u64> = SingleHartCell::new(0);

/// PMP entry 0-7 address shadow — boot'ta kaydedilir, her tick'te doğrulanır
static PMP_SHADOW_ADDRS: SingleHartCell<[usize; 8]> = SingleHartCell::new([0; 8]);

/// PMP entry 8 shadow — per-task NAPOT stack region
pub(crate) static PMP_SHADOW_ADDR8: SingleHartCell<usize> = SingleHartCell::new(0);
pub(crate) static PMP_SHADOW_CFG2: SingleHartCell<usize> = SingleHartCell::new(0);

// Linker script'ten gelen semboller
extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __bss_end: u8;
    static __stack_top: u8;
    /// _end: data + bss + stack sonrası — PMP RW TOR üst sınırı
    static _end: u8;
    /// __pmp_data_end: PMP Entry 5 TOR üst sınırı (task stacks ve WASM arena dışarıda)
    static __pmp_data_end: u8;
    /// __task_stacks_start: task stacks bölgesinin başlangıcı
    static __task_stacks_start: u8;
    /// __task_stacks_end: task stacks bölgesinin sonu
    static __task_stacks_end: u8;
}

/// PMP bölgelerini ayarla
pub(crate) fn init_pmp() {
    // SAFETY: Linker-provided symbol address — valid for duration of program.
    let text_start = unsafe { &__text_start as *const u8 as usize };
    let text_end = unsafe { &__text_end as *const u8 as usize };
    let rodata_start = unsafe { &__rodata_start as *const u8 as usize };
    let rodata_end = unsafe { &__rodata_end as *const u8 as usize };
    let data_start = unsafe { &__data_start as *const u8 as usize };
    let pmp_data_end = unsafe { &__pmp_data_end as *const u8 as usize };

    use crate::common::config;
    let uart_start = config::UART_BASE;
    let uart_end   = config::UART_END;

    // ─── PMP Adres Register'ları (linker script sırasıyla) ───
    //
    // Bellek düzeni: .text → .rodata → .data → .bss → kernel_stack
    //   → __pmp_data_end (Entry 5 TOR sınırı)
    //   → .task_stacks (per-task NAPOT Entry 8)
    //   → .wasm_arena (M-mode only, U-mode deny)
    //
    //   pmpaddr0 = text_start    (Entry 0: OFF, alt sınır)
    //   pmpaddr1 = text_end      (Entry 1: TOR RX)  → .text
    //   pmpaddr2 = rodata_start  (Entry 2: OFF, alt sınır)
    //   pmpaddr3 = rodata_end    (Entry 3: TOR R)   → .rodata
    //   pmpaddr4 = data_start    (Entry 4: OFF, alt sınır)
    //   pmpaddr5 = pmp_data_end  (Entry 5: TOR RW)  → .data+bss+kernel_stack
    //   pmpaddr6 = uart_start    (Entry 6: OFF, alt sınır)
    //   pmpaddr7 = uart_end      (Entry 7: TOR RW)  → UART MMIO

    pmp::write_pmpaddr(0, text_start);
    pmp::write_pmpaddr(1, text_end);
    pmp::write_pmpaddr(2, rodata_start);
    pmp::write_pmpaddr(3, rodata_end);
    pmp::write_pmpaddr(4, data_start);
    pmp::write_pmpaddr(5, pmp_data_end);
    pmp::write_pmpaddr(6, uart_start);
    pmp::write_pmpaddr(7, uart_end);

    // ─── PMP Config (pmpcfg0) ───
    // L-bit: Entry kilitleme — M-mode da bu izinlere tabi.
    // Eşleşmeyen adresler (CLINT 0x200_0000) → M-mode tam erişir (spec).
    // Scheduler PMP değiştirmediği için L-bit güvenle eklenebilir.
    let configs: [u8; 8] = [
        0,                                                       // Entry 0: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X | pmp::PMP_L,   // Entry 1: .text RX (locked)
        0,                                                       // Entry 2: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_L,                 // Entry 3: .rodata R (locked)
        0,                                                       // Entry 4: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W | pmp::PMP_L,   // Entry 5: .data+bss+stack RW (locked)
        0,                                                       // Entry 6: OFF (alt sınır)
        pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W | pmp::PMP_L,   // Entry 7: UART RW (locked)
    ];

    let packed = pmp::pack_pmpcfg(configs);
    pmp::write_pmpcfg0(packed);

    // Shadow kaydet — her tick'te doğrulama için
    unsafe {
        *PMP_SHADOW.get_mut() = packed;
        // pmpaddr0-7 shadow (write_pmpaddr >> 2 yapıyor, read_pmpaddr da >> 2 döner)
        let addrs = PMP_SHADOW_ADDRS.get_mut();
        addrs[0] = text_start >> 2;
        addrs[1] = text_end >> 2;
        addrs[2] = rodata_start >> 2;
        addrs[3] = rodata_end >> 2;
        addrs[4] = data_start >> 2;
        addrs[5] = pmp_data_end >> 2;
        addrs[6] = uart_start >> 2;
        addrs[7] = uart_end >> 2;
    }

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
    print_hex(pmp_data_end);
    uart::println("");

    uart::puts("[PMP]   UART     RW  0x");
    print_hex(uart_start);
    uart::puts(" - 0x");
    print_hex(uart_end);
    uart::println("");

    uart::puts("[PMP]   pmpcfg0 = 0x");
    print_hex(pmp::read_pmpcfg0() as usize);
    uart::println("");
    uart::println("[PMP]   Catch-all: U-mode implicit DENY (RISC-V spec)");
    uart::println("[PMP]   Task stacks: .task_stacks, per-task NAPOT Entry 8");
    uart::println("[PMP]   WASM arena: .wasm_arena, M-mode only (U-mode deny)");
}

use crate::common::fmt::print_hex;

/// Task stacks bölgesi adres aralığı (trap handler stack overflow detection için)
/// Dönüş: (start, end) — linker symbol adresleri
#[cfg(not(kani))]
pub(crate) fn task_stacks_range() -> (usize, usize) {
    // SAFETY: Linker-provided symbols — valid for duration of program.
    let start = unsafe { &__task_stacks_start as *const u8 as usize };
    let end   = unsafe { &__task_stacks_end   as *const u8 as usize };
    (start, end)
}

/// PMP bütünlük doğrulama — shadow ile karşılaştır
#[cfg(not(kani))]
pub(crate) fn verify_pmp_integrity() -> bool {
    // pmpcfg0 shadow
    let current = pmp::read_pmpcfg0();
    let shadow = unsafe { *PMP_SHADOW.get() };
    if current != shadow { return false; }

    // pmpaddr0-7 shadow (defense-in-depth, L-bit kilitli)
    let shadow_addrs = unsafe { PMP_SHADOW_ADDRS.get() };
    let mut i = 0;
    while i < 8 {
        if pmp::read_pmpaddr(i) != shadow_addrs[i] { return false; }
        i += 1;
    }

    // Task PMP shadow (entry 8)
    let cfg2 = pmp::read_pmpcfg2();
    let addr8 = pmp::read_pmpaddr8();
    let shadow_cfg2 = unsafe { *PMP_SHADOW_CFG2.get() };
    let shadow_addr8 = unsafe { *PMP_SHADOW_ADDR8.get() };
    cfg2 == shadow_cfg2 && addr8 == shadow_addr8
}
