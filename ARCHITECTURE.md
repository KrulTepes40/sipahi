# Sipahi Microkernel — Architecture

## Layer Structure

```
Layer 0: arch/     — RISC-V hardware (PMP, CSR, CLINT, UART, trap)
Layer 1: hal/      — Abstraction (DeviceAccess, IOPMP, key store, secure boot)
Layer 2: kernel/   — Core (scheduler, syscall, capability, policy, memory)
Layer 3: sandbox/  — Isolation (WASM via Wasmi, bump allocator)
Cross:   common/   — Shared (config, types, error, crypto, fmt, sync)
Cross:   ipc/      — Communication (SPSC ring buffer, blackbox recorder)
```

## Dependency Flow

```
arch <- hal <- kernel <- sandbox
               ^         ^
            common     common
               ^         ^
             ipc --------+
```

No circular dependencies. Upper layers depend on lower layers only.

## Security Layers

1. Rust type system (compile-time memory safety)
2. PMP hardware protection (L-bit locked, 4 regions)
3. Capability tokens (BLAKE3 MAC, nonce replay guard)
4. WASM sandbox (fuel metering, float rejection)
5. CRC32 IPC integrity
6. Policy engine (6-mode failure escalation)
7. Blackbox flight recorder (power-loss tolerant)

## Safety Doctrine

- Zero panic in production code
- Zero heap allocation in kernel (alloc confined to WASM sandbox)
- Zero floating-point (determinism)
- Zero recursion (bounded stack)
- All mutable state via SingleHartCell<T> (zero static mut)

## Formal Verification

- Kani: 83+ bounded model checking proofs
- Clippy: zero warnings (`-D warnings`)
- All unsafe blocks documented with `// SAFETY:`

## Memory Map

See `sipahi.ld` header comment for full layout.
