// Sipahi — Compile-time sabitleri
// Bu dosyadaki her değer derleme zamanında sabit.
// Runtime'da değişmez. Sipahi doktrini.

/// Maksimum task sayısı (8 task × 24KB = 192KB)
pub const MAX_TASKS: usize = 8;

/// Scheduler tick periyodu (mikrosaniye)
pub const TICK_PERIOD_US: u32 = 10_000; // 10ms

/// Kernel stack boyutu (byte)
pub const KERNEL_STACK_SIZE: usize = 4096; // 4KB

/// Task stack boyutu (byte)
pub const TASK_STACK_SIZE: usize = 8192; // 8KB

/// IPC mesaj boyutu (byte) — L1 cache line
pub const IPC_MSG_SIZE: usize = 64;

/// IPC kanal slot sayısı
pub const IPC_CHANNEL_SLOTS: usize = 16;

/// Maksimum IPC kanal sayısı
pub const MAX_IPC_CHANNELS: usize = 8;

/// WASM heap boyutu (byte)
pub const WASM_HEAP_SIZE: usize = 65536; // 64KB

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
