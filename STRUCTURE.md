# Sipahi v1.5 — File Structure

```
sipahi/
├── .cargo/config.toml           # riscv64imac-unknown-none-elf target + QEMU runner
├── .github/workflows/ci.yml     # GitHub Actions: clippy + build
├── src/
│   ├── main.rs                  # Entry point, task_a/task_b, panic handler (~110 lines)
│   ├── boot.rs                  # Boot sequence: PMP, HAL, task creation, timer (~55 lines)
│   ├── verify.rs                # Kani formal verification harnesses (~600 lines)
│   ├── tests/
│   │   └── mod.rs               # POST + integration tests (~470 lines)
│   ├── arch/                    # Layer 0: RISC-V hardware
│   │   ├── boot.S               # _start → BSS clear → stack → rust_main
│   │   ├── trap.S               # Trap frame save/restore (34 registers)
│   │   ├── trap.rs              # M-mode trap handler (timer, ecall, faults, MPP check)
│   │   ├── context.S            # switch_context (16 regs) + task_trampoline (mret)
│   │   ├── csr.rs               # CSR read/write + mstatus MPP/MPIE constants
│   │   ├── pmp.rs               # PMP register access + pack_pmpcfg
│   │   ├── clint.rs             # CLINT timer (drift-free mtimecmp scheduling)
│   │   ├── uart.rs              # NS16550A UART (transmit-ready check)
│   │   └── mod.rs
│   ├── hal/                     # Layer 1: Hardware abstraction
│   │   ├── device.rs            # DeviceAccess trait (static dispatch)
│   │   ├── iopmp.rs             # IOPMP stub (software emulation)
│   │   ├── key.rs               # Ed25519 key provisioning (test-keys feature)
│   │   ├── secure_boot.rs       # Ed25519 signature verification (RFC 8032)
│   │   └── mod.rs
│   ├── kernel/                  # Layer 2: Core kernel
│   │   ├── scheduler/
│   │   │   └── mod.rs           # Priority scheduler, budget, watchdog, U-mode trampoline
│   │   ├── capability/
│   │   │   ├── token.rs         # 32B capability token
│   │   │   ├── cache.rs         # 4-slot constant-time cache with TTL
│   │   │   ├── broker.rs        # BLAKE3 MAC, per-task nonce, token expiry
│   │   │   └── mod.rs
│   │   ├── syscall/
│   │   │   ├── dispatch.rs      # 5-handler table, WCET tracking, pointer validation
│   │   │   └── mod.rs           # Userspace ecall wrappers
│   │   ├── policy/
│   │   │   └── mod.rs           # 6-mode failure engine with lockstep verification
│   │   ├── memory/
│   │   │   └── mod.rs           # PMP region setup + shadow integrity check
│   │   └── mod.rs
│   ├── ipc/                     # Layer 2: Communication
│   │   ├── mod.rs               # SPSC lock-free ring buffer (&self API, CRC32)
│   │   └── blackbox.rs          # 128-record flight recorder (u64 monotonic tick)
│   ├── sandbox/                 # Layer 3: WASM isolation
│   │   ├── mod.rs               # Wasmi runtime, float scanner v2, compute services
│   │   └── allocator.rs         # Bump allocator (4MB arena, epoch reset)
│   └── common/                  # Cross-cutting
│       ├── config.rs            # Compile-time constants
│       ├── types.rs             # TaskState, TaskConfig, Q32, newtypes
│       ├── error.rs             # SipahiError enum + as_str()
│       ├── sync.rs              # SingleHartCell<T> (zero static mut)
│       ├── fmt.rs               # print_u32, print_u64, print_hex
│       ├── diagnostic.rs        # DiagStats + Diagnosable trait (v2.0)
│       ├── crypto/
│       │   ├── provider.rs      # HashProvider + SignatureVerifier traits
│       │   ├── blake3_impl.rs   # BLAKE3 keyed hash (no_std)
│       │   └── mod.rs           # Feature-gated provider selection
│       └── mod.rs
├── sipahi.ld                    # Linker script (8MB RAM, memory map in header)
├── Makefile                     # build, run, debug, check, kani, clean
├── Cargo.toml                   # Dependencies + features + overflow-checks
├── README.md                    # Project overview
├── ARCHITECTURE.md              # Layer structure + security model
├── LICENSE                      # Apache-2.0
└── docs/
    └── sipahi_v10_0.txt         # Architecture document
```

## Stats

| Metric | Value |
|---|---|
| Source lines (Rust + ASM) | ~7,350 |
| `.rs` files | 41 |
| `.S` files | 3 |
| Kani proofs | 173 |
| Compile-time asserts | 7 |
| `static mut` count | 0 |
| `unsafe` blocks documented | all |
