# Sipahi

A safety-critical hard real-time microkernel for RISC-V, written in Rust.

**Target:** CVA6 (RISC-V RV64IMAC), QEMU `virt` machine  
**License:** Apache-2.0  
**Toolchain:** Rust nightly (`riscv64imac-unknown-none-elf`, `build-std=core,alloc`)

---

## Overview

Sipahi is a bare-metal microkernel designed for DO-178C DAL-A avionics workloads. It provides:

- **Zero heap in kernel** — bump allocator confined to WASM sandbox only
- **Formal verification** — 72 Kani model-checking proofs
- **PMP memory isolation** — 6 hardware protection regions
- **Capability-based access control** — BLAKE3-keyed tokens with cache
- **Fixed-priority preemptive scheduler** — DAL budget enforcement, failure policy engine
- **WASM sandbox** — wasmi 1.0.9, float-opcode rejection, fuel limit
- **Secure boot** — Ed25519 signature verification (RFC 8032), key provisioning

---

## Architecture

```
sipahi/
├── src/
│   ├── arch/          # RISC-V boot, UART, PMP, CLINT, CSR, trap entry
│   ├── hal/           # Device trait, IOPMP stub, Ed25519 secure boot, key store
│   ├── kernel/
│   │   ├── memory/    # PMP region setup
│   │   ├── scheduler/ # Fixed-priority + budget + context switch (ASM)
│   │   ├── capability/# Token broker, BLAKE3 MAC, cache
│   │   └── policy/    # Failure policy engine (RESTART/ISOLATE/DEGRADE/...)
│   ├── ipc/           # SPSC lock-free channels, blackbox flight recorder
│   ├── sandbox/       # WASM sandbox (wasmi), bump allocator, epoch reset
│   └── common/        # Config constants, types (Q32, TaskState, DAL), crypto traits
├── src/verify.rs      # Kani harnesses (central + inline in each module)
├── sipahi.ld          # Linker script (4 MB RAM, 16 KB kernel stack)
└── .cargo/config.toml # riscv64imac-unknown-none-elf target + QEMU runner
```

---

## Build & Run

**Prerequisites:** Rust nightly, `riscv64imac-unknown-none-elf` target, QEMU ≥ 7.0

```bash
# Install target
rustup target add riscv64imac-unknown-none-elf
rustup component add rust-src

# Build (release)
make build

# Run on QEMU virt (Ctrl+A then X to exit)
make run

# Debug build + GDB attach
make debug

# Lint
make check

# Formal verification (requires Kani)
make kani
```

**Debug output** (arena offsets, task counts):
```bash
cargo build --release --features debug-boot \
  -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
```

**WASM runtime tests** (disabled by default, pending PMP/linker fix in Sprint 14):
```bash
cargo build --release --features wasm-sandbox-test \
  -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
```

---

## Quality Gates

| Check | Status |
|---|---|
| `cargo clippy -- -D warnings` | 0 warnings |
| Kani proofs | 72 harnesses |
| `no_std` + `no alloc` in kernel | enforced |
| Panic-free kernel | enforced (`panic = "abort"`, no `unwrap` in kernel) |

---

## Sprint History

| Sprint | Description |
|---|---|
| 0 | Project setup: Rust nightly, `riscv64imac-unknown-none-elf`, bare-metal boot stub |
| 1 | UART driver, BSS clear loop, `_start` → `rust_main`, memory map |
| 2 | PMP: 6 hardware regions (text R/X, rodata R, data R/W, BSS R/W, stack guard, WASM heap) |
| 3 | CLINT timer, `mtvec` trap vector, `trap_entry.S`, `mtime`/`mtimecmp` |
| 4 | Round-robin scheduler, callee-saved context switch (RISC-V ASM) |
| 5 | Syscall interface: ECALL handler, `cap_invoke` / `ipc_send` / `ipc_recv` / `yield` |
| 6 | HAL device trait (static dispatch), IOPMP stub, 25 Kani proofs |
| 7 | SPSC lock-free IPC channels (8 channels × 16 slots × 64 B), host-call limit |
| 8 | Capability system: token broker, BLAKE3-keyed MAC, 4-slot cache, `ct_eq` |
| 9 | Compute service: COPY / CRC32 / MAC / Q32.32 vector dot-product |
| 10 | Fixed-priority preemptive scheduler: DAL budget, period, failure policy engine |
| 11 | Blackbox flight recorder: 8 KB circular buffer, CRC32, power-loss recovery |
| 12 | WASM sandbox: wasmi 1.0.9, float-opcode rejection, fuel limit, bump allocator |
| 13 | Secure boot: Ed25519 (RFC 8032), real BLAKE3 MAC (no-std), key provisioning |
| 14 | `TaskState::Isolated`, GitHub Actions CI, debug-boot feature, 72 Kani proofs |

---

## License

Apache-2.0 — see [LICENSE](LICENSE)
