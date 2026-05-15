//! Compile-time constants: memory layout, WCET budgets, syscall IDs, tick periods.
// U-19 GÖREV 3: blanket allow korundu — config.rs özel.
// Sabitlerin çoğu (BUDGET_DAL_*, WCET_*, SYSCALL_*, COMPUTE_*) Kani harness'i,
// linker (linker_section), veya v2.0 HSM/IOPMP/multi-hart için API yüzeyidir.
// Production binary'de tüketilmeyenler 33 kadar — blanket alternatifi 33 tekil
// allow noise'u; tek satır blanket netliği koruyor (rationale burada).
#![allow(dead_code)]
// Sipahi — Compile-time sabitleri
// Bu dosyadaki her değer derleme zamanında sabit.
// Runtime'da değişmez. Sipahi doktrini.

/// Maksimum task sayısı (8 task × 24KB = 192KB)
pub const MAX_TASKS: usize = 8;

// U-19 GÖREV 1: Magic number'lar config sabiti.
// MAX_TASKS=8 olduğundan task_id < 8 her zaman geçerli; >= 0xFE kernel sentinel.
// Aynı 0xFF değeri farklı semantiğe sahip (task_id vs channel) — okuyucu için ayrılır.
/// Kernel-level event blackbox kaydı — gerçek bir task'a ait değil
pub const SYSTEM_TASK_ID: u8 = 0xFF;
/// Kernel-level event apply_action() index — task_id >= MAX_TASKS dalı
pub const SYSTEM_TASK_INDEX: usize = SYSTEM_TASK_ID as usize;
/// Kernel boot/recover sentinel (KernelBoot blackbox event)
pub const KERNEL_BOOT_ID: u8 = 0xFE;
/// IPC channel atanmamış marker — assign_channel çağrılmamış slot
pub const CHANNEL_UNASSIGNED: u8 = 0xFF;

/// Scheduler tick periyodu (mikrosaniye)
pub const TICK_PERIOD_US: u32 = 10_000; // 10ms

/// Kernel stack boyutu (byte)
/// Sprint 13: 4KB -> 16KB — Ed25519-dalek + BLAKE3 test frame'leri 4KB'ı aşıyordu
pub const KERNEL_STACK_SIZE: usize = 16384; // 16KB

/// Task stack boyutu (byte)
pub const TASK_STACK_SIZE: usize = 8192; // 8KB

/// Trap frame boyutu (byte) — trap.S addi sp,sp,-272 ile uyumlu
/// 16 register × 8 byte + mcause + mepc + user_sp + padding = 272, 16-byte aligned
pub const TRAP_FRAME_SIZE: usize = 272;

/// User SP slot offset trap frame içinde — trap.S sd t0, 256(sp) ile uyumlu
pub const TRAP_FRAME_USER_SP_OFFSET: usize = 256;

// U-17 GÖREV 2: context.S compile-time invariant
// context.S `__stack_top - 16` hardcoded — user_sp slot trap_frame içinde
// offset 256'da. trap_frame 272 byte. user_sp slot uzaklığı = 272 - 256 = 16.
// Bu sabitler arasındaki ilişki kırılırsa context switch sessizce bozulur.
const _: () = assert!(
    TRAP_FRAME_SIZE - TRAP_FRAME_USER_SP_OFFSET == 16,
    "context.S user_sp slot offset mismatch: __stack_top - 16 invariant broken"
);

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
// kullanılabilir. Kesin WCET ölçümü -> FPGA'da (v1.5).

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

/// yield WCET — estimated at 10c, FPGA pending.
pub const WCET_YIELD: u64 = 10;

/// task_info WCET — estimated at 15c, FPGA pending.
/// U-20 GÖREV 5: Önceden WCET_YIELD ile birlikte 10c sayılıyordu — task_info
/// scheduler::query_task_info çağırıyor, küçük struct read + bitfield pack;
/// yield'dan biraz daha pahalı ama yine de küçük (≤ scheduler tick).
pub const WCET_TASK_INFO: u64 = 15;

/// SYS_EXIT WCET — estimated at 15c, FPGA pending.
/// U-23 SNTM Phase 1: voluntary task termination syscall.
/// Components: current_task_id (3c) + isolate_task state write + cap invalidate (~5c)
/// + schedule_yield Phase 3/4 only (~5c). Conservative bound 15c.
pub const WCET_EXIT: u64 = 15;

// ═══════════════════════════════════════════════════════
// Syscall ID'leri (Sprint 7 + U-23 SNTM Phase 1)
// ═══════════════════════════════════════════════════════

pub const SYS_CAP_INVOKE: usize = 0;
pub const SYS_IPC_SEND: usize = 1;
pub const SYS_IPC_RECV: usize = 2;
pub const SYS_YIELD: usize = 3;
pub const SYS_TASK_INFO: usize = 4;
pub const SYS_EXIT: usize = 5;       // U-23: SNTM task termination
pub const SYSCALL_COUNT: usize = 6;  // 5 → 6 (U-23 SYS_EXIT)

// U-22.5 G2: COMPUTE_* ID sabitleri silindi (4 sabit).
// dispatch_compute fonksiyonu WASM-tied orphan code'du; SNTM v1.5'te
// task-side typed IPC ile değişiyor. Historical WCET değerleri:
//   COMPUTE_COPY ~80c, COMPUTE_CRC ~1500c, COMPUTE_MAC ~350c, COMPUTE_MATH ~200c
// (SIPAHI_V1_TO_V2_TRANSITION.md historical note olarak korunur.)

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

// U-22.5 G2: WCET_COMPUTE_* sabitleri silindi (4 sabit).
// Compute service WCET hedefleri WASM dispatch_compute path'i için vardı.
// Historical reference (FPGA pending):
//   WCET_COMPUTE_COPY = 80c, WCET_COMPUTE_CRC = 1500c (bit-by-bit),
//   WCET_COMPUTE_MAC = 350c (BLAKE3), WCET_COMPUTE_MATH = 200c (Q32.32 dot)
// SIPAHI_V1_TO_V2_TRANSITION.md historical note olarak korunur.

// U-22 GÖREV 6 [M10]: WCET zincir -> tick budget invariant.
// wcet_ordering_consistent (verify.rs:60) tautolojikti (sabitleri kendine
// kıyaslıyordu). GERÇEK invariant: en kötü senaryoda tek tick içinde
// scheduler + token validate + en pahalı compute + IPC + task_info
// hepsi sığmalı, yoksa scheduler overrun -> DeadlineMiss policy.
//
// Bu compile-time assert; Kani gerekmez, her build'de doğrular.
// U-22.5 G3: WCET_COMPUTE_CRC silinince yerine WCET_CONTEXT_SWITCH (worst-case
// kernel hot path component). Chain re-balanced for post-WASM Sipahi baseline.
const _: () = assert!(
    (WCET_SCHEDULER_TICK
        + WCET_TOKEN_VALIDATE
        + WCET_CONTEXT_SWITCH
        + WCET_IPC_SEND
        + WCET_TASK_INFO) < CYCLES_PER_TICK as u64,
    "WCET worst-case syscall chain exceeds CYCLES_PER_TICK budget — \
     scheduler overrun risk, DAL bütçesi yeniden hesaplanmalı"
);

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
