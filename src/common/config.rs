//! Compile-time constants: memory layout, WCET budgets, syscall IDs, tick periods.
#![allow(dead_code)]
// Sipahi — Compile-time sabitleri
// Bu dosyadaki her değer derleme zamanında sabit.
// Runtime'da değişmez. Sipahi doktrini.

/// Maksimum task sayısı (8 task × 24KB = 192KB)
pub const MAX_TASKS: usize = 8;

/// Scheduler tick periyodu (mikrosaniye)
pub const TICK_PERIOD_US: u32 = 10_000; // 10ms

/// Kernel stack boyutu (byte)
/// Sprint 13: 4KB → 16KB — Ed25519-dalek + BLAKE3 test frame'leri 4KB'ı aşıyordu
pub const KERNEL_STACK_SIZE: usize = 16384; // 16KB

/// Task stack boyutu (byte)
pub const TASK_STACK_SIZE: usize = 8192; // 8KB

/// IPC mesaj boyutu (byte) — L1 cache line
pub const IPC_MSG_SIZE: usize = 64;

/// IPC kanal slot sayısı
pub const IPC_CHANNEL_SLOTS: usize = 16;

/// Maksimum IPC kanal sayısı
pub const MAX_IPC_CHANNELS: usize = 8;

/// WASM heap boyutu (byte)
/// wasmi 1.0.9 Engine::new() + Module::new() lazy allocation ile ~4MB kullanır.
/// QEMU 512MB RAM, linker script 8MB — 4MB arena güvenli.
pub const WASM_HEAP_SIZE: usize = 4194304; // 4MB

/// Host call limiti (period başına)
pub const HOST_CALL_LIMIT: u16 = 16;

/// QEMU virt machine adresleri
pub const UART_BASE: usize = 0x1000_0000;
pub const CLINT_BASE: usize = 0x200_0000;
pub const RAM_BASE: usize = 0x8000_0000;

/// UART MMIO region sonu (PMP için)
pub const UART_END: usize = UART_BASE + 0x100;

/// RAM üst sınırı — QEMU virt 512MB
pub const RAM_END: usize = RAM_BASE + 512 * 1024 * 1024;

/// CLINT register offsetleri (Sprint 3'te kullanılacak)
pub const CLINT_MTIMECMP_OFFSET: usize = 0x4000; // hart 0 mtimecmp
pub const CLINT_MTIME_OFFSET: usize = 0xBFF8;    // mtime (64-bit)

/// CPU frekansı (Hz) — QEMU virt default
pub const CPU_FREQ_HZ: u64 = 10_000_000; // 10MHz (QEMU)
// NOT: Gerçek CVA6 = 100MHz, QEMU farklı olabilir
//
// WCET ÖLÇÜM UYARISI:
// QEMU TCG modunda rdcycle gerçek donanım cycle'ı DEĞİL,
// instruction count döner. WCET ölçümü için mtime (sabit frekanslı
// timer) daha tutarlı. rdcycle sadece göreli karşılaştırma için
// kullanılabilir. Kesin WCET ölçümü → FPGA'da (v1.5).

// ═══════════════════════════════════════════════════════
// WCET hedefleri (cycle, @100MHz)
// DURUM: Tümü estimated — FPGA ölçümü pending.
// QEMU instruction count cycle doğruluğu vermez.
// Kesin ölçüm: CVA6 FPGA + mtime counter.
// Sprint U-15: U-9 mscratch swap + U-10 UART gate sonrası recalibrated.
// ═══════════════════════════════════════════════════════

/// trap_entry WCET — estimated at 80c, FPGA measurement pending.
/// U-9 sonrası: ~33c entry (mscratch swap + 16 register save + CSR read + ecall check) plus ~30c exit. Rounded up for safety margin.
pub const WCET_TRAP_ENTRY: u64 = 80;

/// trap_handler Rust dispatch WCET — estimated at 80c, FPGA pending.
/// Timer path ~33c (tick increment, schedule_next_tick, overrun check, schedule call). Ecall path 40-80c (dispatch overhead, syscall function). Rounded up for worst-case.
pub const WCET_TRAP_HANDLER: u64 = 80;

/// Scheduler tick WCET — estimated at 350c, FPGA measurement pending.
/// Components: PMP verify(~40c) + Phase1 period loop(~64c) +
/// Phase1.5 watchdog reset(~80c) + Phase2 budget(~15c) +
/// Phase3 priority select(~48c) + Phase4 context switch(~80c) = ~327c
/// Rounded up for safety margin.
pub const WCET_SCHEDULER_TICK: u64 = 350;

/// Context switch WCET — estimated at 80c, FPGA measurement pending.
/// 14 callee-saved save(14c) + 2 CSR save(4c) + la+ld user_sp(4c)
/// + 14 callee-saved restore(14c) + 2 CSR restore(4c) + la+sd user_sp(4c)
/// + ret(1c) ≈ 45c. Rounded up for pipeline/cache effects.
pub const WCET_CONTEXT_SWITCH: u64 = 80;

/// Capability invoke cache hit WCET — estimated at 25c, FPGA pending.
/// validate_cached: 4-slot scan(12c) + ct_eq_16(8c) + tick check(3c) = ~23c
pub const WCET_CAP_INVOKE: u64 = 25;

/// IPC send WCET — estimated at 60c, FPGA pending.
/// channel bounds(3c) + ptr validate(5c) + rate limit(5c) +
/// ring buffer write(20c) + CRC optional(0c production) ≈ 33c
/// Rounded up.
pub const WCET_IPC_SEND: u64 = 60;

/// IPC recv WCET — estimated at 40c, FPGA pending.
/// channel bounds(3c) + ring buffer read(15c) + ptr write(5c) ≈ 23c
/// Rounded up.
pub const WCET_IPC_RECV: u64 = 40;

/// yield / task_info WCET — estimated at 10c, FPGA pending.
pub const WCET_YIELD: u64 = 10;

// ═══════════════════════════════════════════════════════
// Syscall ID'leri (Sprint 7'de kullanılacak)
// ═══════════════════════════════════════════════════════

pub const SYS_CAP_INVOKE: usize = 0;
pub const SYS_IPC_SEND: usize = 1;
pub const SYS_IPC_RECV: usize = 2;
pub const SYS_YIELD: usize = 3;
pub const SYS_TASK_INFO: usize = 4;
pub const SYSCALL_COUNT: usize = 5;

// ═══════════════════════════════════════════════════════
// Compute service ID'leri (Sprint 12'de kullanılacak)
// ═══════════════════════════════════════════════════════

pub const COMPUTE_COPY: u8 = 0; // Bellek kopyala, WCET ~80c (U-14: stub)
pub const COMPUTE_CRC: u8 = 1; // CRC32 bütünlük, WCET ~1500c (bit-by-bit)
pub const COMPUTE_MAC: u8 = 2; // BLAKE3 keyed hash, WCET ~350c
pub const COMPUTE_MATH: u8 = 3; // Q32.32 vektör dot, WCET ~200c

// ═══════════════════════════════════════════════════════
// Capability WCET hedefleri (Sprint 9)
// ═══════════════════════════════════════════════════════

/// Token cache hit WCET — estimated at 10c, FPGA pending.
pub const WCET_TOKEN_CACHE_HIT: u64 = 10;

/// Token full validation WCET — estimated at 400c, FPGA pending.
/// BLAKE3 keyed hash(~350c) + ct_eq_16(8c) + cache insert(10c) ≈ 368c
/// Rounded up.
pub const WCET_TOKEN_VALIDATE: u64 = 400;

// ═══════════════════════════════════════════════════════
// Budget sabitleri (Sprint 10)
// ═══════════════════════════════════════════════════════

/// Scheduler tick başına cycle sayısı (CPU_FREQ_HZ × TICK_PERIOD_US / 1_000_000)
/// 10MHz × 10ms = 100_000 cycle/tick
pub const CYCLES_PER_TICK: u32 = 100_000;

/// DAL-A bütçesi: %40 CPU, DEFAULT_PERIOD_TICKS × CYCLES_PER_TICK
pub const BUDGET_DAL_A: u32 = 400_000;

/// DAL-B bütçesi: %30 CPU
pub const BUDGET_DAL_B: u32 = 300_000;

/// DAL-C bütçesi: %20 CPU
pub const BUDGET_DAL_C: u32 = 200_000;

/// DAL-D bütçesi: %10 CPU
pub const BUDGET_DAL_D: u32 = 100_000;

/// Varsayılan period uzunluğu (tick) — 10 tick = 100ms @ 10ms/tick
pub const DEFAULT_PERIOD_TICKS: u32 = 10;

/// IPC send rate limit — tick başına maksimum send sayısı
pub const MAX_SENDS_PER_TICK: u32 = 16;

/// Windowed watchdog alt sınır — kick bu değerden önce gelirse kontrol akışı bozulmuş
/// 0 = window kontrolü devre dışı
pub const WATCHDOG_WINDOW_MIN: u32 = 3;

/// Watchdog limit (tick) — task bu kadar tick boyunca yield etmezse policy tetiklenir
/// 0 = devre dışı. 100 tick = 1 saniye @ 10ms/tick
pub const WATCHDOG_LIMIT: u32 = 100;

// ═══════════════════════════════════════════════════════
// Compute service WCET hedefleri (Sprint 13 aktivasyonu)
// Doküman: COMPUTE_COPY ~80c, COMPUTE_CRC ~120c, COMPUTE_MAC ~350c, COMPUTE_MATH ~200c
// Proof 12 bu sabitlerle aktif edildi (verify.rs).
// ═══════════════════════════════════════════════════════

/// COMPUTE_COPY WCET hedefi (cycle) — 64B bellek bloğu kopyalama
pub const WCET_COMPUTE_COPY: u64 = 80;

/// COMPUTE_CRC WCET — estimated at 1500c, FPGA pending.
/// CRC32 bit-by-bit: 64B × 8 bits × ~3c/bit ≈ 1536c.
/// Sprint U-15: önceki 120c değeri 12× düşüktü.
pub const WCET_COMPUTE_CRC: u64 = 1500;

/// COMPUTE_MAC WCET hedefi (cycle) — BLAKE3 keyed hash (32B token input)
pub const WCET_COMPUTE_MAC: u64 = 350;

/// COMPUTE_MATH WCET hedefi (cycle) — Q32.32 vektör dot product
pub const WCET_COMPUTE_MATH: u64 = 200;

// ═══════════════════════════════════════════════════════
// Blackbox sabitleri (Sprint 11)
// Doküman §BLACKBOX: 8KB / 64B kayıt = 128 kayıt
// ═══════════════════════════════════════════════════════

/// Blackbox depolama boyutu (byte) — PMP R4 bölgesi, battery-backed SRAM/FRAM
pub const BLACKBOX_SIZE: usize = 8192; // 8KB

/// Tek blackbox kaydı boyutu (byte)
/// repr(C) layout: [MAGIC:4][VER:2][PAD:2][SEQ:4][TS:4][TASK:1][EVENT:1][DATA:42][CRC:4] = 64
/// PAD: u16 version sonrası u32 seq alignment için repr(C) padding
pub const BLACKBOX_RECORD_SIZE: usize = 64;

/// Blackbox record data alanı boyutu (byte)
/// BLACKBOX_RECORD_SIZE(64) - header(18, padding dahil) - CRC(4) = 42
pub const BLACKBOX_DATA_SIZE: usize = 42;

/// Maksimum kayıt sayısı — BLACKBOX_SIZE / BLACKBOX_RECORD_SIZE
pub const BLACKBOX_MAX_RECORDS: usize = 128;
