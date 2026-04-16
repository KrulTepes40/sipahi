# Sipahi

A safety-critical hard real-time microkernel for RISC-V, written in Rust.

**Target:** CVA6 (RISC-V RV64IMAC), QEMU `virt` machine
**License:** Apache-2.0
**Toolchain:** Rust nightly (`riscv64imac-unknown-none-elf`, `build-std=core,alloc`)

---

## Overview

Sipahi is a bare-metal microkernel designed for DO-178C DAL-A avionics workloads. It provides:

- **U-mode task isolation** — tasks run in User mode, kernel in Machine mode (mret transition)
- **Zero heap in kernel** — bump allocator confined to WASM sandbox only
- **Formal verification** — 177 Kani model-checking proofs + 7 compile-time const asserts
- **PMP hardware protection** — 4 L-bit locked regions (text RX, rodata R, data RW, UART RW)
- **Capability-based access control** — BLAKE3-keyed tokens with per-task nonce, cache TTL, replay guard
- **Fixed-priority preemptive scheduler** — DAL budget enforcement, windowed watchdog, graceful degradation
- **WASM sandbox** — wasmi 1.0.9, float-opcode rejection (instruction-level v2), fuel metering
- **Secure boot** — Ed25519 signature verification (RFC 8032), key provisioning
- **6-mode failure policy engine** — RESTART/ISOLATE/DEGRADE/FAILOVER/ALERT/SHUTDOWN with lockstep verification
- **Blackbox flight recorder** — 8 KB CRC32-protected circular buffer, u64 monotonic tick (epoch + u32)
- **Power-On Self Test** — CRC32 engine, PMP integrity, policy engine verification at boot
- **IPC rate limiting** — per-task send quota, pointer validation, alignment check
- **Kernel pointer sanitization** — syscall return values scrubbed for kernel address leaks

---

## Architecture

```
sipahi/
├── src/
│   ├── main.rs        # Entry point, task definitions
│   ├── boot.rs        # Boot sequence (PMP, HAL, task creation, timer)
│   ├── tests/         # Integration tests + POST
│   ├── arch/          # RISC-V boot, UART, PMP, CLINT, CSR, trap, context switch
│   ├── hal/           # Device trait, IOPMP, Ed25519 secure boot, key store
│   ├── kernel/
│   │   ├── scheduler/ # Fixed-priority + budget + watchdog + U-mode trampoline
│   │   ├── capability/# Token broker, BLAKE3 MAC, 4-slot cache with TTL
│   │   ├── syscall/   # 5-handler dispatch, WCET tracking, pointer validation
│   │   ├── policy/    # 6-mode failure engine with lockstep
│   │   └── memory/    # PMP region setup + shadow integrity check
│   ├── ipc/           # SPSC lock-free channels (&self API), blackbox recorder
│   ├── sandbox/       # WASM sandbox (wasmi), bump allocator, compute services
│   └── common/        # Config, types, error, crypto, fmt, sync, diagnostic
├── sipahi.ld          # Linker script (8 MB RAM, 16 KB kernel stack)
├── ARCHITECTURE.md    # Layer structure and security model
└── .github/workflows/ # CI pipeline (clippy + build)
```

---

## Build & Run

**Prerequisites:** Rust nightly, `riscv64imac-unknown-none-elf` target, QEMU >= 7.0

```bash
rustup target add riscv64imac-unknown-none-elf
rustup component add rust-src

make build          # Release build
make run            # Run on QEMU virt (Ctrl+A then X to exit)
make debug          # Debug build + GDB attach
make check          # cargo clippy -D warnings
make kani           # Formal verification (requires Kani)
```

---

## Quality Gates

| Check | Status |
|---|---|
| `cargo clippy -- -D warnings` | 0 warnings |
| Kani proofs | 177 harnesses |
| Compile-time asserts | 7 const asserts |
| `no_std` + `no alloc` in kernel | enforced |
| Panic-free kernel | enforced (`overflow-checks = true`, no `unwrap`) |
| `static mut` | 0 (all via `SingleHartCell<T>`) |
| U-mode task isolation | active (MPP=U, mret transition) |
| TLA+ specs | 3/7 verified (IPC, Watchdog, Capability) |

---

## Sprint History

| Sprint | Description |
|---|---|
| 0 | Project setup: Rust nightly, `riscv64imac-unknown-none-elf`, bare-metal boot stub |
| 1 | UART driver, BSS clear loop, `_start` -> `rust_main`, memory map |
| 2 | PMP: 4 L-bit locked hardware regions |
| 3 | CLINT timer, `mtvec` trap vector, `trap_entry.S`, drift-free mtimecmp scheduling |
| 4 | Round-robin scheduler, callee-saved context switch (RISC-V ASM) |
| 5 | Syscall interface: ECALL handler, `cap_invoke` / `ipc_send` / `ipc_recv` / `yield` |
| 6 | HAL device trait (static dispatch), IOPMP stub |
| 7 | SPSC lock-free IPC channels (8 channels, 16 slots, 64 B, &self API) |
| 8 | Capability system: token broker, BLAKE3-keyed MAC, 4-slot TTL cache, `ct_eq` |
| 9 | Compute service: COPY / CRC32 / BLAKE3 MAC / Q32.32 vector dot-product |
| 10 | Fixed-priority preemptive scheduler: DAL budget, period, failure policy engine |
| 11 | Blackbox flight recorder: 8 KB circular buffer, CRC32, u64 monotonic tick |
| 12 | WASM sandbox: wasmi 1.0.9, float-opcode rejection v2, fuel limit, bump allocator |
| 13 | Secure boot: Ed25519 (RFC 8032), real BLAKE3 MAC (no-std), key provisioning |
| 14 | `TaskState::Isolated`, GitHub Actions CI, debug-boot feature |
| 1.5 | U-mode tasks, per-task PMP (NAPOT), windowed watchdog, policy lockstep, graceful degradation, POST, 177 Kani proofs |

---

## License

Apache-2.0 -- see [LICENSE](LICENSE)
