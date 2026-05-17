# Sipahi Microkernel — Architecture

## Layer Structure

```
Layer 0: arch/     — RISC-V hardware (PMP, CSR, CLINT, UART, trap, context switch)
Layer 1: hal/      — Abstraction (DeviceAccess, IOPMP, key store, secure boot)
Layer 2: kernel/   — Core (scheduler, syscall, capability, policy, memory, loader, pmp)
Layer 3: tasks/    — SNTM Native Task Model (task_hello, task_world; PMP-isolated U-mode)
Cross:   common/   — Shared (config, types, error, crypto, fmt, sync, diagnostic)
Cross:   ipc/      — Communication (SPSC lock-free ring buffer, blackbox recorder)
```

**Tarihsel:** Layer 3 v1.x'te `sandbox/` (WASM via Wasmi + bump allocator + compute services)
idi. U-29 v2.0'da WASM tamamen kaldırıldı; SNTM native task model ile değişti.

## Dependency Flow

```
arch <- hal <- kernel <- tasks
               ^         ^
            common     common
               ^
             ipc
```

**Exception:** `arch/trap.rs` calls `kernel::syscall::dispatch()` and
`kernel::scheduler::schedule()`. This is an intentional upward call from
the hardware trap entry point — trap dispatch requires kernel services.
No other circular dependencies exist.

## Privilege Model

- Kernel: M-mode (Machine mode) — full CSR/PMP/MMIO access
- Tasks: U-mode (User mode) — PMP-restricted, no CSR access
- Transition: `mret` instruction via assembly `task_trampoline`
- No S-mode (no MMU, no page tables, no TLB flush non-determinism)

## Security Layers

1. Rust type system (compile-time memory safety)
2. PMP hardware protection (L-bit locked + multi-region SNTM profiles via manifest)
3. Capability tokens (BLAKE3 MAC, per-task nonce, cache TTL, replay guard, expiry)
4. SNTM native task isolation (manifest-driven PMP profile + load-time bounded copy + cross-task statik kanıt + runtime ihlal observation)
5. CRC32 IPC integrity + rate limiting + pointer validation + sealed channel atomicity
6. Policy engine (6-mode failure escalation with lockstep verification)
7. Blackbox flight recorder (power-loss tolerant, u64 monotonic tick)
8. Hardening (PMP shadow, MPP verification, kernel pointer sanitization, windowed watchdog)

(Tarihsel: v1.x'te Layer 4 WASM sandbox vardı; U-29 v2.0'da kaldırıldı.)

## Safety Doctrine

- Zero panic in production code
- Zero heap allocation in kernel (U-29 v2.0: extern crate alloc + global_allocator
  tamamen kaldırıldı; pure `no_std + no_alloc`)
- Zero floating-point (Q32.32 fixed-point)
- Zero recursion (bounded stack)
- Zero `static mut` (all via `SingleHartCell<T>`)
- ~131 `unsafe` blocks, majority documented with `// SAFETY:` (CI informational check reports undocumented blocks via `continue-on-error: true` — advisory, not enforced)

## Formal Verification

- Kani: ~189 bounded model checking harnesses (U-29 sonrası, v1.x'te 213; WASM
  Kani proof'ları sandbox/ ile birlikte kaldırıldı)
- TLA+: 7 specifications, all verified with TLC v2.19 (35,770 distinct states total)
- Compile-time: 8+ `const assert!` (layout, size, config invariants)
- Clippy: zero warnings (`-D warnings`)
- Overflow checks: enabled in release (`overflow-checks = true`)
- Supply chain: `cargo audit` (RustSec CVE scan) + `cargo deny` (license/bans/sources policy)
- CI gates (GitHub Actions): 5 jobs — clippy+build, QEMU boot test (HALT criteria),
  supply chain audit, Kani full (master push), Kani critical subset (PR fast feedback)

## Formal Verification Scope & Limitations

What "verified" means for Sipahi — and what it does NOT mean.

### Kani (Bounded Model Checking)

~189 harnesses cover **Rust function-level invariants**: panic freedom, integer
overflow, array bounds, pure-function purity, policy decision tables, SNTM
multi-region PMP, sealed channel atomicity, cross-task isolation statik.

Kani **does NOT cover**:
- **Assembly** (`trap.S`, `context.S`, inline `asm!`) — Kani works at Rust IR level
- **Hardware register semantics** (CSR read/write, PMP, CLINT mtime/mtimecmp) — stubbed
- **Atomics / memory ordering** — single-hart assumption (`MIE=0` in trap context)
- **Interrupt timing / concurrency** — sequential execution model only
- **Crypto correctness** (BLAKE3, Ed25519/compact) — only API safety; correctness via upstream test vectors

Kani "proved" = "this Rust function over its bounded input space does not panic
or trigger UB." It does **not** mean "the system as a whole behaves correctly."

### TLA+ (Temporal Logic Model Checking)

7 specs verified with TLC: scheduler, capability cache, policy escalation,
watchdog, IPC, degrade/recovery, budget fairness.

TLA+ specs are **abstractions** of Rust code. **No formal refinement mapping**
(TLAPM) has been performed — i.e., we have not proved that the Rust code is a
refinement of the TLA+ spec. Specs and code are kept aligned by review and
shared constants, not by mechanically checked correspondence.

TLAPM-based refinement is a v2.0 roadmap item.

### WCET (Worst-Case Execution Time)

All `WCET_*` constants in `config.rs` are **estimated** based on instruction
counting under QEMU TCG. QEMU does **not** model cache, branch prediction, or
memory hierarchy — its `rdcycle` returns instruction count, not real cycles.

**No cycle-accurate WCET measurement has been performed on real hardware.**
Production WCET validation (FPGA + cycle counter, AbsInt aiT or similar) is a
v1.5 roadmap item. Treat WCET numbers as ordering constraints (which is
faster than which) rather than absolute cycle budgets.

### Bottom line

Sipahi applies **safety-critical design principles** (DO-178C DAL-A, ISO 26262
ASIL-D patterns) and uses **multiple verification layers** to catch entire
classes of bugs early. It is **not certified** and would require independent
DER review, requirements traceability matrix, and FPGA timing analysis before
any certification claim could be made.

## Scheduler Features

- Fixed-priority preemptive (0=highest, 15=lowest)
- Per-task CPU budget with `saturating_sub` (DAL-A 40%, DAL-B 30%, DAL-C 20%, DAL-D 10%)
- Windowed watchdog (upper + lower bound, ISO 26262 / DO-178C)
- Graceful degradation with automatic recovery (DAL-C/D budget halving + restore)
- POST (Power-On Self Test) at boot: CRC32, PMP, policy engine

## Memory Map

See `sipahi.ld` header comment for full layout.

## Known Limitations — v1.0

### Vanilla PMP (no Smepmp)

RISC-V vanilla PMP'de `L`-bit M-mode'un PMP entry'sini *değiştirmesini* kilitler;
ancak U-mode izinleri R/W/X bit'leriyle belirlenir. Entry 5 (kernel data,
`R+W+L`) U-mode task'larının kernel `.data` ve `.bss` bölgelerine read/write
erişimini engellemez — yalnızca W^X garantisi (bu bölgeler X=0) korunur.
Sonuç: `MAC_KEY`, `PMP_SHADOW`, `LAST_NONCE` gibi güvenlik-kritik veriler
U-mode'dan okunabilir (write da mümkündür ama Rust bu yolla erişim üretmez).

**Etki:**
- Capability MAC key okunup token forge edilebilir.
- PMP shadow okunup integrity check bypass tasarlanabilir.
- Kernel kodu (Entry 0-1, RX+L) yazılamaz — W^X korunur, code injection yok.

**Plan (v1.5):** Smepmp extension (RISC-V Trap and Memory Protection v1.0,
`mseccfg.MML=1`) ya da `.secure_data` PMP carve-out (kernel-only entry)
implementasyonu. Bu sprint *yalnızca dokümantasyondur*; kod değişikliği yok.

### Mevcut izolasyon kuvvetleri (vanilla PMP ile bile geçerli)

- W^X: kernel `.text` U-mode'dan write/execute ile yazılamaz.
- Per-task stack izolasyonu: NAPOT Entry 8 + L-bit dışı dinamik reprogramming;
  Task A, Task B'nin stack'ine yazamaz (sadece kendi stack range'i geçerli).
- WASM arena: `.wasm_arena` Entry 5 dışı, U-mode default-deny.
- Trap entry mscratch swap: cross-task corruption engellendi (U-9).
- Capability owner check (U-16): impersonation engellendi.
