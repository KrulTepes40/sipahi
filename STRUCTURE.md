# Sipahi v1.5 — File Structure

```
sipahi/
├── .cargo/config.toml           # riscv64imac-unknown-none-elf target + QEMU runner
├── .github/workflows/ci.yml     # GitHub Actions: build + qemu-test + audit + kani
├── deny.toml                    # cargo-deny policy (licenses, bans, sources)
├── src/
│   ├── main.rs                  # Entry point, task_a/task_b, panic handler (~113 lines)
│   ├── boot.rs                  # Boot sequence: PMP, HAL, task creation, timer (~85 lines)
│   ├── verify.rs                # Kani formal verification harnesses (~925 lines)
│   ├── tests/
│   │   └── mod.rs               # POST + integration + FI tests (~785 lines)
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
├── Tla+/                        # TLA+ formal specs (7 specs, 14 files)
│   ├── SipahiPolicy.tla + .cfg
│   ├── SipahiWatchdog.tla + .cfg
│   ├── SipahiDegradeRecover.tla + .cfg
│   ├── SipahiBudgetFairness.tla + .cfg
│   ├── SipahiCapability.tla + .cfg
│   ├── SipahiIPC.tla + .cfg
│   └── SipahiScheduler.tla + .cfg
└── docs/
    ├── sipahi_context.md        # New-chat context doc (design rationale, build commands)
    ├── sipahi_features_tr.md    # Technical features (Turkish, ~650 lines)
    └── sipahi_features_en.md    # Technical features (English, ~650 lines)
```

## Stats

| Metric | Value |
|---|---|
| Source lines (Rust) | ~8,315 |
| Source lines (ASM) | ~265 |
| `.rs` files | 39 |
| `.S` files | 3 |
| Kani harnesses | 191 (90 symbolic, 101 concrete/compile-time) |
| Compile-time asserts | 8 |
| `static mut` count | 0 |
| `unsafe` blocks | 123 (95 documented with `// SAFETY:`) |
| TLA+ specs | 7 (all verified — Sprint U-12: TLC 2026.04 compatibility fixes) |
| TLA+ lines | ~1,030 |
| Sprints completed | 16 core (0–14 + 1.5) + 12 security (U-3 … U-15, U-7 skipped) = 28 |
| CI jobs | 4 (clippy+build, qemu-test, audit, kani) |
| Supply chain | `cargo audit` (0 CVE) + `cargo deny` (license/bans/sources) |

## Post-Sprint Checklist

After each sprint, run the below and update the metrics in this file:

- [ ] `find src/ -name '*.rs' \| xargs wc -l \| tail -1` → update Rust LOC
- [ ] `grep -rc 'kani::proof' src/ \| awk -F: '{s+=$2} END {print s}'` → update Kani count
- [ ] `grep -rc 'unsafe {' src/ \| awk -F: '{s+=$2} END {print s}'` → update unsafe count
- [ ] `grep -rB3 'unsafe {' src/ \| grep -c 'SAFETY:'` → update SAFETY-documented count
- [ ] `cd Tla+ && for s in *.tla; do java -jar tla2tools.jar -config ${s%.tla}.cfg $s; done` → update TLA+ status
- [ ] `cargo clippy -- -D warnings` → 0 warnings
- [ ] `cargo kani` → all harnesses PASS
- [ ] `cargo audit && cargo deny check` → clean
- [ ] Increment sprint count in this file
- [ ] `git tag -a sprint-<N> -m "..."` → tag the release
