# Sipahi Microkernel — Technical Feature Document

**Version:** v1.5 · **Architecture:** RISC-V RV64IMAC · **Language:** Rust no_std  
**Total:** ~8,315 Rust + ~265 ASM lines · 42 source files · 191 Kani harnesses · 7/7 TLA+ verified  
**Philosophy:** Maximum speed while preserving determinism. Zero heap, zero panic, zero float.

---

## 1. Core Design Decisions

### 1.1 Why RISC-V?

Sipahi targets the RISC-V RV64IMAC ISA. Not ARM or x86 because RISC-V is an open-source ISA — no licensing fees, fully customizable. In military and aviation systems, dependence on a foreign ISA poses a strategic risk. With RISC-V, the entire hardware chain can be kept under local control. RV64IMAC profile: 64-bit integer (I), multiply/divide (M), atomics (A), compressed instructions (C). Float extensions (F/D) are intentionally excluded — floating-point is forbidden in Sipahi; all computation is done in Q32.32 fixed-point.

### 1.2 Why Rust?

Rust was chosen over C because Rust's ownership system guarantees memory safety at compile time. In safety-critical C code, 70% of memory errors originate from use-after-free, buffer overflow, and dangling pointers. Rust makes these impossible at the language level. Runs in bare-metal environments with `no_std` + `no_alloc` (kernel level). The `alloc` crate is used only for the WASM sandbox (Wasmi); kernel code performs no heap allocation.

### 1.3 Why No Floating-Point?

IEEE 754 floating-point arithmetic can be non-deterministic — the same computation may produce different results on different hardware (rounding mode, denormalized number handling). This is unacceptable in safety-critical systems. Sipahi performs all computations in Q32.32 fixed-point (`i64`): ±2³¹ range, ~2.3×10⁻¹⁰ precision. If float opcodes are detected in WASM modules, the module is rejected — `is_float_opcode()` scans the 0x43–0xBF range.

### 1.4 Why Microkernel?

Microkernel over monolithic because the attack surface is small. The kernel contains only the scheduler, IPC, capability, policy, and trap handler. WASM sandbox, blackbox, and secure boot run outside the kernel. If a component crashes, the kernel stays alive. A small, verifiable kernel is required for DO-178C DAL-A certification — 191 Kani harnesses (90 symbolic proofs + 101 concrete/compile-time assertions) and 7/7 TLA+ specs formally verify critical invariants (scheduler selection correctness, policy escalation, IPC integrity, memory safety).

---

## 2. Privilege Separation — M-mode / U-mode

### 2.1 Architecture

Sipahi uses two privilege levels. The kernel runs in M-mode (Machine mode) — full access to CSRs, PMP registers, and MMIO. Tasks run in U-mode (User mode) — no CSR access, memory access restricted by PMP.

**Why no S-mode (Supervisor)?** RISC-V S-mode requires an MMU (virtual memory). Sipahi is a bare-metal microkernel — page table overhead and TLB flush non-determinism are undesirable. M/U separation provides physical memory protection via PMP, without MMU complexity.

### 2.2 U-mode Transition Mechanism

When a task is created, `mstatus.MPP = 00` (U-mode) and `mstatus.MPIE = 1` (interrupt enable) are set. The `start_first_task()` function drops to U-mode via `csrw mepc, entry` + `csrw mstatus, val` + `mret`. During context switches, `task_trampoline` (assembly) is used: `switch_context` → `ret` → `task_trampoline` → `mret` → U-mode. This trampoline executes `mret` without compiler interference (no prologue/epilogue).

**Why assembly trampoline?** When defined as a Rust function, the compiler adds a prologue; LTO behaves differently. In assembly, it's just `mret` — one instruction, zero overhead, zero ambiguity.

### 2.3 mstatus.MPP Verification

After every U-mode ecall, the trap handler checks the `mstatus.MPP` bits. If MPP ≠ 0 (i.e., the task is attempting to escalate to M-mode), a fault injection attack has been detected — `PRIVILEGE ESCALATION DETECTED → SHUTDOWN`. This check is skipped during M-mode ecalls (boot tests) because MPP=3 is correct during boot.

---

## 3. Memory Protection — PMP

### 3.1 Region Layout

RISC-V PMP supports 16 entries. Sipahi uses TOR (Top of Range) mode because region sizes are not powers of two — NAPOT cannot be used. 8 entries are used for kernel regions; 8 entries are reserved for task regions (pmpcfg2, to be activated in Sprint U-3).

| Entry | Region | Permission | Description |
|-------|--------|------------|-------------|
| 0-1 | .text | RX + Lock | Kernel code — write forbidden (W^X) |
| 2-3 | .rodata | R + Lock | Read-only data — write/execute forbidden |
| 4-5 | .data+bss+kernel_stack | RW + Lock | Writable data + kernel stack (__pmp_data_end boundary) |
| 6-7 | UART MMIO | RW + Lock | Serial port access |
| 8 | Task stack (NAPOT) | RW | Per-task, reprogrammed on context switch |
| 9-15 | Reserved | — | Future use |

**Why L-bit (Lock)?** When L-bit is set, PMP rules are enforced at all privilege levels including M-mode. This prevents accidental overwrites of kernel code (.text) even in M-mode. In U-mode, addresses not matching any PMP entry are automatically denied (RISC-V spec §3.7.1).

### 3.2 PMP Shadow Register

At boot, PMP register values are saved to the `PMP_SHADOW` static. Every scheduler tick, the actual register is read via `read_pmpcfg0()` and compared with the shadow. A mismatch indicates a fault injection attack → `PmpViolation → SHUTDOWN`. Cost: 1 CSR read + 1 compare = O(1), ~5 cycles/tick.

**Why shadow register?** PMP registers can be corrupted through hardware fault injection (glitching, laser). Shadow comparison detects this attack at the software level.

### 3.3 Per-Task PMP (NAPOT)

Task stacks are placed in the `.task_stacks` linker section, outside the PMP Entry 5 boundary. This ensures U-mode tasks cannot access all RAM via Entry 5 — since there is no PMP match for the task stacks region, U-mode implicit DENY applies (RISC-V spec §3.7.1). Each task accesses its own stack solely through PMP entry 8 NAPOT. NAPOT mode is used — 8KB = 2^13 is a power-of-2, requiring only one entry. On every context switch, entry 8 is reprogrammed to the new task's stack region. Config: R+W, X=0 (W^X), L=0 (unlocked — changes on switch). The WASM arena is also placed in `.wasm_arena` section — U-mode DENY, M-mode accesses it (Wasmi interpreter runs in M-mode).

NAPOT task stack protection has been tested on QEMU virt machine. QEMU PMP granularity parameter (G) is platform-dependent. CVA6 target expects G=0 (4-byte granularity). On different hardware, G may affect the NAPOT minimum region size — to be verified during FPGA validation.

---

## 4. Scheduler

### 4.1 Fixed-Priority Preemptive Scheduler

Sipahi uses a fixed-priority preemptive scheduler. Fixed priority over round-robin or EDF (Earliest Deadline First) because WCET analysis is simpler and preferred for DO-178C certification. Priority 0 = highest (DAL-A), priority 15 = lowest (DAL-D).

**select_highest_priority()** performs an O(N) linear scan (N = MAX_TASKS = 8). Hash tables or priority queues were not used because the overhead is unnecessary for 8 tasks and worst case = best case = 8 iterations. Branchless — always scans all tasks, constant-time guarantee.

### 4.2 Budget Enforcement

Each task is assigned a CPU budget: DAL-A 40%, DAL-B 30%, DAL-C 20%, DAL-D 10%. Budget is decremented via `saturating_sub(CYCLES_PER_TICK)` — overflow impossible. When budget is exhausted, a `BudgetExhausted` policy event is triggered.

**Why saturating_sub?** With `wrapping_sub`, the budget could underflow past 0 and wrap to a large positive number — the task would gain infinite budget. `saturating_sub` stops at 0, safe.

### 4.3 Period-Based Task Model

Each task has a period (default 10 ticks = 100ms). When the period expires, the budget is replenished and Suspended tasks transition to Ready. This model is compatible with rate-monotonic scheduling in aviation.

### 4.4 Windowed Watchdog

Each task has a watchdog counter. It increments every tick and resets on `sys_yield` or `watchdog_kick()`. Two-directional protection:

**Upper bound:** `watchdog_counter >= watchdog_limit` → task has stopped, not responding → `WatchdogTimeout` policy event. Limit 0 = watchdog disabled.

**Lower bound (Windowed):** If a kick arrives when `watchdog_counter < watchdog_window_min` → task is running too fast, control flow is corrupted (rapid loop instead of normal operation) → `WatchdogTimeout` policy event. `WATCHDOG_WINDOW_MIN = 3` ticks.

**Why windowed?** A simple watchdog asks "did the task stop?" A windowed watchdog asks "is the task running at the CORRECT RATE?" Windowed watchdogs are mandatory in ISO 26262 and DO-178C. Cost: +1 compare/kick (~1 cycle).

### 4.5 schedule() Three-Phase

The scheduler runs three phases every tick. Phase 1: period advancement + watchdog + IPC rate reset. Phase 2: apply policy decisions (budget, watchdog). Phase 3: select highest-priority Ready/Running task, context switch.

### 4.6 Context Switch

The `switch_context` assembly function saves/restores 16 registers (14 callee-saved + mepc + mstatus) = 128-byte TaskContext. `ret` to `task_trampoline` → `mret` → U-mode transition. Thanks to the callee-saved convention, s0-s11 are not saved in the trap handler — the Rust calling convention already preserves them.

---

## 5. Syscall Dispatch

### 5.1 O(1) Jump Table

Function pointer table for 5 syscalls: `cap_invoke`, `ipc_send`, `ipc_recv`, `yield`, `task_info`. Index bounds check → single comparison → direct jump. No match/branch — O(1) dispatch.

**Why table instead of match?** Match is compiler-dependent — may generate a jump table or an if-else chain. A function pointer table is always O(1), deterministic.

### 5.2 Pointer Validation

`is_valid_user_ptr()` applies five-layer validation: null check, `checked_add` overflow protection, kernel memory boundary (`ptr < kernel_end`), RAM upper bound (`end > RAM_END`), 8-byte alignment check.

**Why RAM_END check?** Without it, absurd addresses like 0xFFFF_FFFF_FFFF_0000 would be considered valid. PMP would block it, but having the kernel accept the pointer and then fall into a PMP trap makes WCET unpredictable.

### 5.3 WCET Measurement

Each syscall measures start/end cycle count via `rdcycle`, recorded in the `WCET_MAX` array. `check_wcet_limits()` compares current WCET against targets.

**Why rdcycle?** In QEMU TCG, it returns instruction count (not real cycles). This measurement is sufficient for relative comparison; precise WCET → on FPGA.

### 5.4 Syscall Counter

On every dispatch call, the active task's `syscall_count` field is incremented via `wrapping_add(1)`. Anomaly detection — if a task makes an abnormal number of syscalls, it may be a DoS attempt. Cost: 1 instruction/syscall.

### 5.5 IPC Rate Limiter

`check_ipc_rate()` check inside `sys_ipc_send`. Per-tick limit of `MAX_SENDS_PER_TICK = 16` messages. If exceeded, `E_RATE_LIMITED` is returned. Counter resets every tick. DoS protection — a malicious task cannot flood an IPC channel.

### 5.6 Kernel Pointer Sanitization

If the syscall handler return value falls within the kernel address range (`RAM_BASE..kernel_end`), `E_INTERNAL` is returned. Kernel pointers are never leaked to U-mode — info leak protection. Cost: 2 compares/syscall.

### 5.7 Argument Truncation Protection

Inside `sys_cap_invoke`: `cap > u8::MAX`, `resource > u16::MAX`, `action > u8::MAX` checks. Silent truncation like `cap=256 → cap as u8 = 0` is prevented.

---

## 6. Capability Token System

### 6.1 Token Structure

32-byte `#[repr(C)]` token: id (u8), task_id (u8), resource (u16), action (u8), dal (u8), padding (2B), expires (u32), nonce (u32), MAC (16B). Stack-only, no heap. Fixed size — easy PMP protection, DMA transfer, serialization.

**Why 32 bytes?** Half of an L1 cache line (64B). Two tokens fit in one cache line. Smaller would reduce MAC field size (weaker security); larger would increase cache misses.

### 6.2 BLAKE3 Keyed Hash MAC

Token integrity is protected by BLAKE3 keyed hash. A 32-byte key is written once at boot via `provision_key()`. `validate_full()` computes a 16-byte MAC from the token header and compares it with the token's MAC using constant-time comparison.

**Why BLAKE3, not HMAC-SHA256?** BLAKE3 is Rust-native, `no_std` compatible, ~350 cycles (3-5x faster than SHA-256). Deterministic, timing side-channel protected. BLAKE3 uses the portable backend (SIMD optimization disabled, `default-features = false`). This ensures platform-independent deterministic execution.

**Verification scope:** Kani proofs verify BLAKE3 API memory safety (using a Kani stub — a trivial impl returning the first 16 bytes of the key). Cryptographic correctness is NOT proven by Kani — the BLAKE3 crate has been externally audited (Runtime Verification, Stellar Dev Foundation sponsorship, Dec 2025). Ed25519: `ed25519-dalek` 2.x, RUSTSEC-2022-0093 patched. Sipahi's responsibility is correct crate invocation + input bounds checking — these are verified by Kani.

### 6.3 4-Slot Constant-Time Cache

`TokenCache` performs a 4-slot constant-time scan — always compares all 4 entries, no early exit. Hit accumulation via bitwise AND, branchless. Cache hit ~10 cycles, full validation ~400 cycles.

**Why 4 slots?** 8 tasks × a few resources = 4 active tokens is sufficient in practice. Hash tables were not used because hash computation can cause non-deterministic cache misses. 4-slot linear scan always takes the same number of cycles.

**Why no early exit?** Early exit creates a timing side-channel. A token found in slot 1 vs slot 4 takes different time — an attacker can measure which tokens are cached.

### 6.4 Cache TTL

Each cache entry has an `expires` field. During `lookup()`, a `get_tick() <= expires` check is performed. Expired tokens automatically drop from the cache. `expires = 0` → infinite validity.

### 6.5 Per-Task Nonce (Replay Guard)

`LAST_NONCE: [u32; MAX_TASKS]` — each task tracks an independently monotonically increasing nonce. `token.nonce <= last_nonce[task_id]` → replay attack → REJECT. Per-task instead of a single global nonce because Task A's nonce should not affect Task B.

### 6.6 Token Expiry

`token.expires > 0` and `get_tick() > expires as u64` → expired token → REJECT. `get_tick()` returns an epoch-based monotonic u64 — the 49-day u32 wrap issue is resolved (`BB_BOOT_EPOCH << 32 | BB_TICK`).

### 6.7 ct_eq_16 (Constant-Time Compare)

16-byte MAC comparison uses bitwise XOR + OR accumulation. `memcmp` is not used because it exits at the first differing byte — timing side-channel. `ct_eq_16` always scans all 16 bytes.

---

## 7. Policy Engine — 5+1 Mode Failure Policy

### 7.1 Design

`decide_action(event, restart_count, dal)` is a pure function — no static mut, no side effects. 9 event types, 6 FailureModes: Restart, Degrade, Isolate, Failover, Alert, Shutdown. Match table — every path takes constant cycles.

**5+1 mode:** In v1.0, Failover falls back to Degrade — hot-standby task switch mechanism is planned for v2.0. `decide_action → Failover → runtime applies Degrade`, while the blackbox records it as `PolicyFailover` event (for forensics).

### 7.2 Escalation Chains

| Event | Initial Response | Repeated | Final |
|-------|-----------------|----------|-------|
| BudgetExhausted | Restart | After MAX_RESTART_BUDGET | Degrade |
| StackOverflow | Restart | After MAX_RESTART_FAULT | Isolate |
| CapViolation | Isolate | — | — |
| PmpFail | Shutdown | — | — |
| WatchdogTimeout | Failover | After MAX_RESTART_WATCHDOG | Degrade |
| DeadlineMiss | DAL-A→Failover, DAL-D→Isolate | — | — |
| MultiModuleCrash | Shutdown | — | — |
| Unknown (>8) | Isolate | — | — (fail-safe) |

**Why Isolate as fail-safe default?** Unknown event → Shutdown is too aggressive (stops the system); Restart is too soft (can loop). Isolate quarantines the problem while the system continues running.

**Why PMP fail always Shutdown?** PMP integrity failure = memory protection is broken. The system is not trustworthy — the only safe decision is to halt.

### 7.3 Policy Lockstep (Dual Redundancy)

Inside `apply_policy()`, `decide_action()` is called twice and the results are compared. If the same input produces different output = cosmic ray, bit flip, or memory corruption → `FailureMode::Shutdown`. Without this, fault injection could manipulate the policy engine — returning Restart instead of Shutdown to keep a crashed task running.

**Why dual, not triple (TMR)?** In a single-hart system, the probability of memory corruption between two calls is astronomically low. TMR requires three calls + majority vote — adds ~10 cycles to WCET. Dual provides sufficient protection at ~5 cycle cost.

### 7.4 Graceful Degradation

When `degrade_system()` is triggered, DAL-C/D tasks are Suspended and their budgets are halved. The `DEGRADED` flag is set. Every scheduler tick, `try_recover_from_degrade()` is called: if all DAL-A/B tasks are healthy (none Isolated), DAL-C/D tasks are restarted with `original_budget`.

**Why budget halving?** After recovery, DAL-C/D tasks run in cautious mode — starting with half budget instead of immediately loading full budget. The original budget is stored in the `original_budget` field — in cyclic degradation, the budget never drops to zero; every recovery restores the original.

**Why automatic recovery?** Manual recovery requires an operator — in autonomous systems (drones, vehicles) there is no operator. If DAL-A/B are healthy, the system recovers itself. Critical for the AEGIS Safety Island — if Autoware crashes, MRM starts; if Autoware recovers, AEGIS steps back.

---

## 8. IPC — Lock-Free SPSC Ring Buffer

### 8.1 Design

8 static `SpscChannel`s, each with 16 slots × 64-byte messages. Lock-free — AtomicU16 head/tail, no mutex. Single producer (task A) single consumer (task B) model. Full → `Err(BufferFull)`, data is never overwritten. Empty → `None`.

**Why SPSC, not MPMC?** MPMC requires locks or CAS loops — WCET is uncertain. SPSC uses a single atomic read + single atomic write = O(1), guaranteed WCET.

**Why AtomicU16?** u16 → 65,536 head/tail space. With 16 slots, `% 16` modulo is used. When u16 wraps, modulo still works correctly (Kani proof: `ipc_ring_buffer_wrap_never_exceeds_slots`).

### 8.2 CRC32 Integrity Check (Opt-in)

IPC provides CRC32 helper methods: `set_crc()` computes payload CRC, `verify_crc()` validates it. **CRC usage is application-level — the kernel does not enforce CRC on send/recv.** `send()` and `recv()` do not auto-CRC; the sender computes if it calls `set_crc()`, the receiver validates if it calls `verify_crc()`.

CRC32 is computed bit-by-bit — no lookup table. Kernel-enforced auto-CRC planned for v2.0 (requires WCET budget revision: IPC_SEND 60c → ~1600c).

**Why no lookup table?** A 256-entry LUT = 1KB. If not in L1 cache, a cache miss → non-deterministic latency. Bit-by-bit: 8 iterations per byte, deterministic. 60-byte payload × 8 = 480 iterations — constant WCET.

---

## 9. Blackbox Flight Recorder

### 9.1 Design

128 records × 64 bytes = 8KB circular buffer. Each record: MAGIC (4B "SPHI"), version (2B), sequence (2B), timestamp (4B), task_id (1B), event (1B), data (46B), CRC32 (4B). Only the kernel writes — protected by PMP.

### 9.2 Power-Loss Protection

CRC32 detects partially written records. Power cut → record incomplete → CRC fail → `is_valid()` false → skipped. `volatile` writes prevent LTO reordering.

**Why not HMAC-BLAKE3?** CRC32 here is not for tamper protection but for power-loss detection. Only the kernel writes to the blackbox, protected by PMP. Physical access attacks (JTAG/probe) are addressed at the FPGA+production level, not in software.

### 9.3 Monotonic Tick

`BB_TICK` u32 increments every scheduler tick via `wrapping_add(1)`. Wrap detection: `next < t` → `BB_BOOT_EPOCH` u16 is incremented. `get_tick()` → `(epoch << 32) | tick` = effective u48 range. Assuming a 10ms tick, u32 alone wraps in ~497 days; combined with the u16 epoch, the counter stays monotonic for ~89,000 years — far enough for token expiry checks.

### 9.4 Event Types

14 event types: KernelBoot (0), TaskStart (1), TaskSuspend (2), TaskRestart (3), BudgetExhausted (4), PolicyIsolate (5), PolicyDegrade (6), PolicyFailover (7), PolicyShutdown (8), CapViolation (9), IopmpViolation (10), DeadlineMiss (11), WatchdogTimeout (12), PmpFail (13). Golden mine for post-mortem analysis.

---

## 10. WASM Sandbox

### 10.1 Wasmi 1.0.9 Runtime

Wasmi interpreter — register-based bytecode, deterministic execution. Interpreter over JIT runtime (Wasmtime) because JIT is non-deterministic (different platform = different native code).

**Why not Wasmi 2.0-beta?** Beta is not used in safety-critical systems. Wasmi 1.0.9 is stable, includes the register-based engine. `prefer-btree-collections` feature for `no_std` safety — no hash tables (random init issue).

### 10.2 Fuel Metering

Each WASM instruction consumes 1 fuel. When fuel is exhausted, execution stops. Infinite loops are impossible — proven by Kani liveness proof. Dual-layer protection together with budget enforcement.

### 10.3 Float Opcode Rejection

`validate_module()` scans all opcodes when loading a module. If a float opcode is found in the 0x43–0xBF range, `Err(FloatOpcodes)` → module rejected. `skip_instruction()` correctly skips LEB128 immediates and fixed-size operands (f32.const → 5B, f64.const → 9B) — buffer overread is impossible with bounds checking (found and fixed via Kani proof).

### 10.4 BumpAllocator

4MB arena, O(1) allocation, no free, zero fragmentation. `epoch_reset()` resets the entire arena (module reload). `checked_add` overflow protection + `aligned >= WASM_HEAP_SIZE` OOM check. Two allocations never overlap (Kani proof: `bump_allocator_offsets_never_overlap`).

### 10.5 Compute Services

4 fixed services: COPY (memory copy, ~80c — Sprint U-14: stub, returns NotImplemented), CRC (CRC32 bit-by-bit, ~1500c — Sprint U-15 estimate: 64B × 8 bits × ~3c), MAC (BLAKE3 keyed hash, ~350c), MATH (Q32.32 vector dot product, ~200c). WCET targets are estimated; FPGA measurement pending.

---

## 11. Secure Boot

### 11.1 Ed25519 Signature Verification

Boot chain: ROM boot (M-mode) → Ed25519 signature verification → load Sipahi kernel. Ed25519 was chosen because compared to RSA-2048, it has a 64-byte signature (RSA: 256 bytes) and 32-byte public key (RSA: 256 bytes), making it far more compact — it needs to fit in a bare-metal OTP fuse. Compared to ECDSA-P256, it offers constant-time verification (ECDSA has nonce-dependent timing side-channel risk) and simpler implementation. The `ed25519-dalek` crate is Rust-native, `no_std` compatible, RFC 8032 compliant — no heap allocation during verification (stack-only). On invalid public key or corrupted signature, it returns `false` instead of panicking.

### 11.2 Key Provisioning Model

Two-tier key hierarchy: Root key in OTP fuse (immutable, device lifetime), Module key in .rodata (signed by root key, updatable). In QEMU v1.0, there is no OTP — RFC 8032 Test Vector #1 is used as a compile-time constant via the `test-keys` feature. In production, factory provisioning: generate key pair in HSM → write public key to OTP → private key stays in HSM → burn JTAG fuse.

### 11.3 CNSA 2.0 Roadmap

`fast-sign` (Ed25519) and `cnsa-sign` (LMS post-quantum) are mutually exclusive features. `compile_error!` prevents both being active simultaneously or neither being active. LMS is not yet implemented — to be added in v2.0.

---

## 12. IOPMP (I/O Physical Memory Protection)

Stub implementation — requires real IOPMP hardware (DMA controller). 8 regions, enable/disable, `check_access(addr, size, write)` for read/write/size control. Will be activated on FPGA. When disabled, all access is permitted (fail-open); when enabled, only access to defined regions is allowed.

---

## 13. Trap Handler

### 13.1 Assembly (trap.S)

16 caller-saved register save/restore (ra, t0-t6, a0-a7). CSRs (mcause, mepc) are saved to the stack. ecall (mcause=8 U-mode, mcause=11 M-mode) → mepc+4 advance → `trap_handler()` call. On ecall return, the syscall result is written to the saved a0 slot.

### 13.2 Rust (trap.rs)

Timer interrupt (code=7) → increment tick, call scheduler. ecall → syscall dispatch. Illegal instruction → ISOLATE. LoadAccessFault (5) and StoreAccessFault (7) → PMP violation → ISOLATE + blackbox log. MPP verification after U-mode ecall.

### 13.3 Timer — Drift-Free

`schedule_next_tick()` reads the previous `mtimecmp` value and adds `+ ticks_per_period()`. Not based on `read_mtime()` because handler delay creates cumulative drift.

---

## 14. Boot Sequence

`_start` (boot.S) → hart 0 selection → BSS clear → stack setup → `rust_main`. `rust_main` (boot.rs) → PMP init → UART init → Timer init → task creation → test suite → scheduler start. Multi-hart: harts other than 0 park with `wfi`.

---

## 15. Formal Verification — 191 Kani Harnesses + 7/7 TLA+

### 15.1 Proof Distribution

| Module | Proofs | Coverage |
|--------|--------|----------|
| verify.rs (global) | 67 | DAL, PMP, memory, cross-module invariants |
| sandbox (mod+allocator) | 20+1 | LEB128, float scanning (U-14: load/store + comparisons), bounds safety, allocator overlap |
| dispatch | 18 | Syscall table, pointer rejection, dispatch fuzzing |
| scheduler | 17 | Selection correctness, Isolated/Dead never selected, watchdog, priority |
| ipc (mod+blackbox) | 15+14 | CRC roundtrip, channel bounds, ring buffer wrap, blackbox record/CRC/wrap |
| policy | 14 | Escalation chains, PMP→Shutdown, livelock freedom |
| capability (mod+broker+cache) | 15+2+2 | Token encoding, cache, invalidation by token/owner (U-14), nonce, ct_eq_16 |
| crypto | 2 | BLAKE3 API memory safety (Kani stub) — cryptographic correctness via external audit |
| hal (iopmp+key+boot) | 2+1+1 | IOPMP boundary, key size, secure boot |
| **Total** | **191** | 90 symbolic proofs (explore state space via kani::any) + 101 concrete/compile-time assertions |

### 15.2 High-Value Proofs

These proofs symbolically explore the entire input space using `kani::any()` — equivalent to infinite tests:

- **isolated_never_scheduled_any_config**: Isolated tasks are never selected across all state/priority combinations
- **selected_has_minimum_priority**: The selected task always has the lowest priority number (priority inversion impossible)
- **dispatch_rejects_invalid_syscall_id**: Invalid syscall ID → E_INVALID_SYSCALL via full dispatch call
- **policy_never_livelocks_on_repeated_failure**: Terminal state is reached after 10 consecutive crashes (infinite restart loop impossible)
- **wasm_skip_instruction_never_exceeds_bounds**: Buffer overread impossible with poisoned opcodes/LEB128 (this proof found a real bug and it was fixed)
- **bump_allocator_offsets_never_overlap**: Two allocations never overlap
- **invalidated_token_never_found_in_cache**: An invalidated token cannot be found in the cache with any resource/action

### 15.3 Const Asserts (Compile-Time)

Constant checks were moved from Kani to `const _: () = assert!(...)` at compile time — zero runtime cost; if the condition is not met, the code does not compile: Token == 32B, IpcMessage == 64B, IPC_CHANNEL_SLOTS > 0, BlackboxRecord == BLACKBOX_RECORD_SIZE, BLACKBOX_MAX_RECORDS <= 255, SYSCALL_COUNT == 5, OTP_KEY_SIZE == 32, SIGNATURE_SIZE == 2 × OTP_KEY_SIZE (8 const asserts total).

---

## 16. Modular Cryptography — Compile-Time Trait Selection

### 16.1 HashProvider Trait

`HashProvider::keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 16]` — for token MAC computation. Compile-time dispatch via Rust monomorphization — no runtime branching; unselected providers occupy no space in the binary. `fast-crypto` → BLAKE3 (~350 cycles), `cnsa-crypto` → SHA-384 + Zknh HW (~1500 cycles, v2.0).

### 16.2 SignatureVerifier Trait

`SignatureVerifier::verify(public_key, message, signature) -> bool` — for secure boot and WASM module verification. `fast-sign` → Ed25519, `cnsa-sign` → LMS post-quantum (v2.0). Thanks to the trait system, algorithm changes require only a single feature flag change — kernel code remains unchanged.

### 16.3 Feature Flag System

| Feature | Description | Conflict Protection |
|---------|-------------|---------------------|
| `fast-crypto` | BLAKE3 hash/MAC | Mutually exclusive with `cnsa-crypto` |
| `cnsa-crypto` | SHA-384 + Zknh HW (v2.0) | Mutually exclusive with `fast-crypto` |
| `fast-sign` | Ed25519 signatures | Mutually exclusive with `cnsa-sign` |
| `cnsa-sign` | LMS post-quantum (v2.0) | Mutually exclusive with `fast-sign` |
| `test-keys` | RFC 8032 test vectors | Disabled in production |
| `debug-boot` | Boot diagnostic output | Disabled in production |

Conflicting features are prevented by `compile_error!` — compile error, not runtime error. At least one sign feature must be active — if both are disabled, the code does not compile.

---

## 17. HAL — Hardware Abstraction Layer

### 17.1 DeviceAccess Trait

All hardware devices implement the `DeviceAccess` trait: `init()`, `read_byte()`, `write_byte()`, `is_ready()`. Static dispatch — `dyn Trait` forbidden, no vtable overhead. Every operation is bounded, non-blocking. On error, `SipahiError` is returned; no panic.

**Why static dispatch?** `dyn Trait` requires vtable pointer dereference — cache miss risk, WCET uncertainty. Static dispatch: the compiler inlines the function, zero overhead.

### 17.2 UartDevice

NS16550A UART implementation. `putc()` checks LSR (Line Status Register) bit 5 for transmit-ready — busy-wait but UART hardware always drains (~1μs/byte). `read_byte()` checks LSR bit 0 for data-ready — if no data, returns `Err(DeviceNotReady)`, non-blocking.

### 17.3 Diagnosable Trait

Health check and statistics reporting trait for each subsystem: `health_check() -> bool`, `stats() -> DiagStats`. DiagStats: name, ok, counter, error_count. API integration planned for v2.0 (scaffolding in place, implementation pending).

---

## 18. Synchronization — SingleHartCell

`UnsafeCell<T>` wrapper — zero-cost, no locks, no synchronization. SAFETY: only safe on single-hart systems. Will be replaced with `Mutex<T>` when multi-hart support is added. `Sync` trait is provided via `unsafe impl` — tells the compiler "this type can be shared across threads."

**Why not Mutex?** Mutex has lock/unlock cycles — added to WCET, priority inversion risk. Unnecessary overhead on single-hart. Hubris, Tock, and Embassy use the same pattern.

---

## 19. Error Handling

14 `SipahiError` variants — every error is explicit, no silent failures. `as_str()` provides a description string for each variant. `#[must_use]` on critical functions — the compiler enforces that results are checked. Panic handler enters a `wfi` loop — halts instead of crashing. OOM handler is the same — heap exhaustion does not crash the kernel. `shutdown_system()` logs to UART and enters an infinite `wfi` loop — hardware-level safe halt.

---

## 20. Boot-Time Integration Test Suite

Sipahi tests all subsystems during boot — before the scheduler starts. Test suite:

- **Policy Engine (6 tests)**: Budget→Restart, Budget→Degrade, CapViolation→Isolate, PmpFail→Shutdown, DeadlineMiss DAL-A→Failover, DeadlineMiss DAL-D→Isolate
- **Capability Broker (3 tests)**: validate_full MAC verification, cap_invoke cache hit, cap_invoke cache miss denial
- **IPC SPSC (9 tests)**: Empty recv, CRC set/verify, send OK, recv + CRC valid, double recv None, buffer full at 15, send when full Err, tampered CRC fail, invalid channel None
- **WCET Regression**: Each syscall's WCET limit is checked (informational only on QEMU TCG)
- **Secure Boot**: BLAKE3 determinism, key-binding, Ed25519 (with test-keys feature)
- **WASM Sandbox**: Module load, execute (result=42), fuel exhaustion trap, float rejection, epoch reset + reload
- **Blackbox**: Init record, log record, record validation

All tests print results via UART: `✓` passed, `✗` failed.

### 20.2 POST — Power-On Self Test

Runs before the test suite. If any single test fails, the scheduler does not start — halts with `wfi` loop. PBIT (Power-on Built-In Test) is mandatory for DO-178C DAL-A.

- **CRC32 engine**: Known vector "123456789" → `0xCBF43926` (IEEE 802.3). If mismatch, the CRC engine is corrupted — all integrity checks are unreliable. HALT.
- **PMP integrity**: The actual register is read via `read_pmpcfg0()` and compared with the shadow saved at boot. Mismatch = register corruption. HALT.
- **Policy engine sanity**: `decide_action(PmpFail, 0, 0)` → must return Shutdown. If not, the policy engine is corrupted — risk of incorrect safety decisions. HALT.
- **mstatus M-mode CSR access**: mstatus readable, MPP bits not in reserved value. HALT on fail.
- **mtvec set**: Trap handler installed (`mtvec != 0`). HALT on fail.
- **BLAKE3 self-test**: Determinism (same input → same output) + non-zero output check. HALT on fail.
- **Ed25519 RFC 8032 TV1** (with test-keys feature): Signature verification against known test vector. HALT on fail.
- **CLINT timer advance** (Sprint U-15): Does mtime register advance (hardware timer alive)? WARN level (QEMU TCG compatibility).
- **misa ISA identity** (Sprint U-15): MXL=2 (RV64) + bits I/M/A/C set. WARN level.

Cost: boot-time only, zero runtime overhead. Adds ~1ms to boot time.

---

## 21. Task Data Structure — Complete Field List

Each task contains a 128-byte TaskContext + metadata fields:

| Field | Type | Description |
|-------|------|-------------|
| id | u8 | Task identifier (0-7) |
| state | TaskState | Ready, Running, Suspended, Dead, Isolated |
| context | TaskContext | 16 registers (ra, sp, s0-s11, mepc, mstatus) = 128B |
| entry | usize | Entry point address (for restart) |
| stack_top | usize | Aligned stack top (for restart) |
| priority | u8 | 0-15 (0=highest, DAL-A group 0-3) |
| dal | u8 | Design Assurance Level (0=A, 1=B, 2=C, 3=D) |
| budget_cycles | u32 | CPU budget per period (cycles) |
| remaining_cycles | u32 | Remaining cycles in current period |
| period_ticks | u32 | Period length (ticks) |
| period_counter | u32 | Tick counter within current period |
| watchdog_counter | u32 | Tick counter — reset by yield/kick |
| watchdog_limit | u32 | Limit (0=disabled) — triggers policy when exceeded |
| watchdog_window_min | u32 | Windowed watchdog lower bound — error if kick too early |
| syscall_count | u32 | Anomaly detection — wrapping_add(1) on dispatch |
| ipc_send_count | u32 | Rate limiter — reset every tick |
| original_budget | u32 | Original budget before degrade (for recovery) |
| pmp_addr_napot | usize | NAPOT-encoded PMP address (entry 8, per-task stack) |

All fields are statically allocated — no heap. `Task::empty()` provides zeroed default values. `restart_task()` clears the context, reconfigures entry + stack + mepc + mstatus, and assigns `task_trampoline` to ra for U-mode transition.

---

## 22. Security Walls (7 Layers)

| # | Wall | Status | Description |
|---|------|--------|-------------|
| 1 | WASM Sandbox | ✅ Complete | Fuel metering + float rejection + isolated memory |
| 2 | Capability Token | ✅ Complete | BLAKE3 MAC + nonce + expiry + constant-time cache |
| 3 | PMP (kernel) | ✅ Complete | 4 TOR regions, L-bit locking + shadow register |
| 4 | PMP (per-task) | ✅ Complete | Task stacks outside Entry 5, NAPOT entry 8, WASM arena M-mode only |
| 5 | IOPMP | ⚠️ Stub | Requires real hardware (DMA controller) — FPGA |
| 6 | M/U-mode separation | ✅ Complete | Kernel in M-mode, tasks in U-mode, mret transition |
| 7 | Physical | ❌ None | JTAG/OTP/tamper — FPGA+production level |

5/7 walls completed at software level. Remaining: 1 hardware (IOPMP), 1 manufacturing (physical).

---

## 23. Hardening Features

| Feature | Cost | Attack Protected |
|---------|------|-----------------|
| PMP shadow register | ~5 cycles/tick | Fault injection (PMP register corruption) |
| mstatus.MPP verification | ~5 cycles/ecall | Privilege escalation |
| Syscall counter | ~1 cycle/dispatch | Anomaly detection / DoS |
| IPC rate limiter | ~2 cycles/send | IPC flood DoS |
| Kernel pointer sanitization | ~2 cycles/syscall | Info leak (kernel address leakage) |
| Argument truncation protection | ~3 cycles/cap_invoke | Silent truncation → wrong token ID |
| Timer drift-free | 0 extra cycles | Cumulative timing drift |
| BB_TICK epoch | ~3 cycles/wrap | 49-day u32 wrap → token expiry breakage |
| Windowed watchdog | ~1 cycle/kick | Control flow corruption (too-fast loops) |
| Policy lockstep | ~5 cycles/policy | Fault injection (policy decision manipulation) |
| Graceful degradation | 0 (O(N) when triggered) | DAL-C/D automatic recovery, budget protection |
| POST (boot-time) | 0 (no runtime cost) | Booting with corrupted RAM/CRC/PMP/policy |

Hardening items run at different points (per tick / per syscall / per ecall), so summarising them as a single "per-tick cost" is misleading. The items active during a single scheduler tick (PMP shadow ~5c + policy lockstep ~5c + watchdog ~1c + drift-free timer 0c ≈ ~11c) fit inside the `WCET_SCHEDULER_TICK = 350c` budget. Per-syscall costs (kernel pointer sanitization, MPP verification, syscall counter) are accounted for in the relevant syscall WCET targets.

---

## 24. Formatting and Diagnostic Helpers

Heap-free format functions for debug output over UART: `print_u32` (decimal), `print_u64` (decimal), `print_hex` (hex, no 0x prefix). All use stack-based buffers — `[u8; 10]` for u32, `[u8; 20]` for u64. `alloc::format!` and `core::fmt` are not used — they bloat binary size and can be non-deterministic.

---

## 25. Build System and Tools

- **Toolchain:** Rust nightly-2026-03-01, riscv64imac-unknown-none-elf target
- **Build:** `make build` (build-std flags), `cargo clippy -- -D warnings` (target in config.toml)
- **Run:** `make run` (QEMU 8.2.2 virt machine, -bios none, 512MB RAM)
- **Verify:** `cargo kani` (191 harnesses), const assert (8 compile-time checks), TLC (7 TLA+ specs)
- **Supply chain:** `cargo audit` (RustSec CVE scan, 0 CVE) + `cargo deny check` (license/bans/sources policy)
- **CI:** GitHub Actions 4 jobs — clippy+build, QEMU boot test (HALT criteria), supply chain audit, Kani (master push only)
- **WASM:** Wasmi 1.0.9, `default-features = false`, `prefer-btree-collections`
- **Crypto:** BLAKE3 (`fast-crypto` feature), Ed25519 (`fast-sign` feature, `ed25519-dalek`)

---

## 26. Performance Targets (100MHz CVA6 — estimated, FPGA pending)

Recalibrated in Sprint U-15. All values are kept in sync with the constants in `src/common/config.rs` — that is the single source of truth.

| Operation | Target | Equivalent @100MHz |
|-----------|--------|--------------------|
| trap_entry | ≤80 cycles | ≤0.80μs |
| trap_handler (Rust dispatch) | ≤80 cycles | ≤0.80μs |
| context_switch | ≤80 cycles | ≤0.80μs |
| scheduler_tick | ≤350 cycles | ≤3.50μs |
| sys_yield | ≤10 cycles | ≤0.10μs |
| ipc_recv | ≤40 cycles | ≤0.40μs |
| ipc_send | ≤60 cycles | ≤0.60μs |
| cap_invoke (cache-hit path) | ≤25 cycles | ≤0.25μs |
| token_cache_hit | ≤10 cycles | ≤0.10μs |
| token_validate (BLAKE3 full) | ≤400 cycles | ≤4.00μs |
| compute_mac (BLAKE3) | ≤350 cycles | ≤3.50μs |
| compute_crc (CRC32 bit-by-bit) | ≤1500 cycles | ≤15.00μs |

Precise measurements to be done on FPGA. In QEMU TCG, rdcycle returns instruction count, not real cycles — WCET regressions are used only for relative comparison.

---

---

## Appendix 1. Windowed Watchdog

### Design

Sipahi's watchdog operates bidirectionally — both upper and lower bound checking.

**Upper bound (`watchdog_limit`):** If a task doesn't send a kick within the limit, it's considered stuck. `watchdog_counter` increments every tick, resets on `sys_yield` or `watchdog_kick()`. `counter >= limit` → `WatchdogTimeout` policy event.

**Lower bound (`watchdog_window_min`):** If a kick arrives too early, control flow is considered corrupted. `watchdog_kick()` called when `counter < window_min` → `WatchdogTimeout` policy event.

### Why Bidirectional?

A simple watchdog only catches stuck tasks. But if a task enters an infinite loop calling kick every iteration, a simple watchdog sees it as "healthy." A windowed watchdog catches this: kicks arriving too fast means the task's normal control flow is corrupted.

### Parameters

`WATCHDOG_WINDOW_MIN = 3` ticks. Task can kick at earliest after 3 ticks. Kick at tick 1 or 2 → policy engine intervenes.

Cost: ~1 cycle/kick (single comparison).

---

## Appendix 2. Policy Lockstep (Software Dual Execution)

### Design

`decide_action()` is called twice per invocation. If the two results differ → `Shutdown`.

### Why?

`decide_action()` is a pure function — same input must always produce same output. Different results = hardware-level corruption (cosmic ray, fault injection, RAM error). The policy engine is the kernel's "brain" — decision correctness is the foundation of system safety.

Cost: ~5 cycles/policy decision.

---

## Appendix 3. Graceful Degradation + Auto-Recovery

### degrade_system()

DAL-C/D tasks are Suspended, budgets halved. DAL-A/B continue at full budget.

### try_recover_from_degrade()

Called every tick. If DAL-A/B are healthy, DAL-C/D are restored to Ready with `original_budget`.

### Why original_budget?

Prevents cumulative budget halving across degrade/recover cycles. Recovery always restores the original value.

Cost: 0 cycles during normal operation, O(N) when triggered (~20 cycles, N=8).

---

## Appendix 4. POST (Power-On Self-Test)

At boot: CRC32 known vector, PMP shadow integrity, policy engine sanity. Any failure → `wfi` halt, scheduler never starts.

Cost: 0 cycles at runtime (boot only, ~100 cycles added to boot time).

---

## Appendix 5. Illegal Instruction → ISOLATE

When a U-mode task executes an illegal instruction, the trap handler sends a `WasmTrap` event → policy → ISOLATE. Not Restart, because illegal instructions typically indicate memory corruption or attack — restart won't fix it.

---

## Appendix 6. IPC Head wrapping_add Safety

`head.wrapping_add(1)` instead of `head + 1` — prevents panic at u16::MAX with `overflow-checks = true`. Modulo still works correctly (Kani proof: `ipc_ring_buffer_wrap_never_exceeds_slots`).

---

## Appendix 7. ct_eq_16 + black_box LLVM Barrier

16-byte MAC comparison in constant-time: bitwise XOR + OR accumulate, no early exit. `core::hint::black_box()` prevents LLVM from optimizing the loop into `memcmp`. Without it, MAC could be cracked in 4096 attempts via timing side-channel.

---

## Appendix 8. TLA+ Formal Verification — System Level

7 TLA+ specs: SipahiIPC (✅), SipahiWatchdog (✅), SipahiCapability (✅), SipahiPolicy (✅), SipahiScheduler (✅), SipahiBudgetFairness (✅), SipahiDegradeRecover (✅).

Kani verifies at function level, TLA+ at system level. They answer different questions and complement each other. In Sprint U-12 all specs were brought to TLC 2026.04 compatibility (tick bound → StateConstraint, bounded message IDs, WF→SF fairness adjustments).

*Sipahi Microkernel v1.5 — 191 Kani Harnesses · 7/7 TLA+ Verified · 12 Hardening Features · 0 Clippy Warnings · 0 Runtime Panics · 0 Heap Allocations (kernel) · 5/7 Security Walls Active*
