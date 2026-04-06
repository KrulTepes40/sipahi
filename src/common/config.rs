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
// WCET hedefleri (cycle cinsinden, CPU_FREQ_HZ'e göre)
// v10.0 dokümanından — FPGA'da yeniden ölçülecek
// ═══════════════════════════════════════════════════════

/// trap_entry WCET hedefi (cycle)
pub const WCET_TRAP_ENTRY: u64 = 30; // ≤0.3μs @ 100MHz

/// trap_handler WCET hedefi (cycle)
pub const WCET_TRAP_HANDLER: u64 = 50; // ≤0.5μs @ 100MHz

/// scheduler_tick WCET hedefi (cycle)
pub const WCET_SCHEDULER_TICK: u64 = 80; // ≤0.8μs @ 100MHz

/// cap_invoke WCET hedefi (cycle)
pub const WCET_CAP_INVOKE: u64 = 120; // ≤1.2μs @ 100MHz

/// ipc_send WCET hedefi (cycle)
pub const WCET_IPC_SEND: u64 = 60; // ≤0.6μs @ 100MHz

/// ipc_recv WCET hedefi (cycle)
pub const WCET_IPC_RECV: u64 = 40; // ≤0.4μs @ 100MHz

/// yield / task_info WCET hedefi (cycle)
pub const WCET_YIELD: u64 = 10; // ≤0.1μs @ 100MHz

// ═══════════════════════════════════════════════════════
// Syscall ID'leri (Sprint 7'de kullanılacak)
// ═══════════════════════════════════════════════════════

pub const SYS_CAP_INVOKE: u64 = 0;
pub const SYS_IPC_SEND: u64 = 1;
pub const SYS_IPC_RECV: u64 = 2;
pub const SYS_YIELD: u64 = 3;
pub const SYS_TASK_INFO: u64 = 4;

// ═══════════════════════════════════════════════════════
// Compute service ID'leri (Sprint 12'de kullanılacak)
// ═══════════════════════════════════════════════════════

pub const COMPUTE_COPY: u8 = 0; // Bellek kopyala, WCET ~80c
pub const COMPUTE_CRC: u8 = 1; // CRC32 bütünlük, WCET ~120c
pub const COMPUTE_MAC: u8 = 2; // BLAKE3 keyed hash, WCET ~350c
pub const COMPUTE_MATH: u8 = 3; // Q32.32 vektör dot, WCET ~200c

// ═══════════════════════════════════════════════════════
// Capability WCET hedefleri (Sprint 9)
// ═══════════════════════════════════════════════════════

/// Token cache hit WCET (cycle) — 4-slot sabit zamanlı tarama
pub const WCET_TOKEN_CACHE_HIT: u64 = 10; // ≤0.1μs @ 100MHz

/// Token full validation WCET (cycle) — SipahiMAC + ct_eq + cache insert
/// Sprint 13'te BLAKE3 (~350c) ile güncellenecek — gerçek ölçüm FPGA'da
pub const WCET_TOKEN_VALIDATE: u64 = 400; // ≤4μs @ 100MHz

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

// ═══════════════════════════════════════════════════════
// Compute service WCET hedefleri (Sprint 13 aktivasyonu)
// Doküman: COMPUTE_COPY ~80c, COMPUTE_CRC ~120c, COMPUTE_MAC ~350c, COMPUTE_MATH ~200c
// Proof 12 bu sabitlerle aktif edildi (verify.rs).
// ═══════════════════════════════════════════════════════

/// COMPUTE_COPY WCET hedefi (cycle) — 64B bellek bloğu kopyalama
pub const WCET_COMPUTE_COPY: u64 = 80;

/// COMPUTE_CRC WCET hedefi (cycle) — CRC32 hesaplama (64B input)
pub const WCET_COMPUTE_CRC: u64 = 120;

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

/// Tek blackbox kaydı boyutu (byte) — [MAGIC:4][VER:2][SEQ:2][TS:4][TASK:1][EVENT:1][DATA:46][CRC:4]
pub const BLACKBOX_RECORD_SIZE: usize = 64;

/// Maksimum kayıt sayısı — BLACKBOX_SIZE / BLACKBOX_RECORD_SIZE
pub const BLACKBOX_MAX_RECORDS: usize = 128;
