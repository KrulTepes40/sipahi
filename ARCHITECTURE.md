# Sipahi Microkernel — Architecture

## Layer Structure

```
Layer 0: arch/     — RISC-V hardware (PMP, CSR, CLINT, UART, trap, context switch)
Layer 1: hal/      — Abstraction (DeviceAccess, IOPMP, key store, secure boot)
Layer 2: kernel/   — Core (scheduler, syscall, capability, policy, memory)
Layer 3: sandbox/  — Isolation (WASM via Wasmi, bump allocator, compute services)
Cross:   common/   — Shared (config, types, error, crypto, fmt, sync, diagnostic)
Cross:   ipc/      — Communication (SPSC lock-free ring buffer, blackbox recorder)
```

## Dependency Flow

```
arch <- hal <- kernel <- sandbox
               ^         ^
            common     common
               ^         ^
             ipc --------+
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
2. PMP hardware protection (L-bit locked, 4 TOR regions + shadow integrity check)
3. Capability tokens (BLAKE3 MAC, per-task nonce, cache TTL, replay guard, expiry)
4. WASM sandbox (fuel metering, float rejection v2, bump allocator)
5. CRC32 IPC integrity + rate limiting + pointer validation
6. Policy engine (6-mode failure escalation with lockstep verification)
7. Blackbox flight recorder (power-loss tolerant, u64 monotonic tick)
8. Hardening (PMP shadow, MPP verification, kernel pointer sanitization, windowed watchdog)

## Safety Doctrine

- Zero panic in production code
- Zero heap allocation in kernel (alloc confined to WASM sandbox)
- Zero floating-point (Q32.32 fixed-point, WASM float opcodes rejected)
- Zero recursion (bounded stack)
- Zero `static mut` (all via `SingleHartCell<T>`)
- 123 `unsafe` blocks, 95 documented with `// SAFETY:` (CI informational check enforces growth)

## Formal Verification

- Kani: 191 bounded model checking harnesses
  (90 symbolic proofs, 101 concrete/compile-time assertions)
- TLA+: 7 specifications, all verified (Sprint U-12: TLC 2026.04 compatibility)
- Compile-time: 8 `const assert!` (layout, size, config invariants)
- Clippy: zero warnings (`-D warnings`)
- Overflow checks: enabled in release (`overflow-checks = true`)
- Supply chain: `cargo audit` (RustSec CVE scan) + `cargo deny` (license/bans/sources policy)
- CI gates (GitHub Actions): 4 jobs — clippy+build, QEMU boot test (HALT criteria),
  supply chain audit, Kani formal verification (master push only)

## Scheduler Features

- Fixed-priority preemptive (0=highest, 15=lowest)
- Per-task CPU budget with `saturating_sub` (DAL-A 40%, DAL-B 30%, DAL-C 20%, DAL-D 10%)
- Windowed watchdog (upper + lower bound, ISO 26262 / DO-178C)
- Graceful degradation with automatic recovery (DAL-C/D budget halving + restore)
- POST (Power-On Self Test) at boot: CRC32, PMP, policy engine

## Memory Map

See `sipahi.ld` header comment for full layout.
