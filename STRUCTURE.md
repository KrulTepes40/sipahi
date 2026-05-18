# Sipahi (post-SAFE-4) — File Structure

> Status: kernel crate `v1.1.1` + SNTM Native Task Model v2.0 +
> **SNTM-SAFE faz tamamlandı (v1.9.0, sprint-u33)**.

```
sipahi/
├── .cargo/config.toml             # riscv64imac-unknown-none-elf target + QEMU runner
├── rust-toolchain.toml            # nightly-2026-03-01 + rust-src + clippy + llvm-tools-preview
├── .github/workflows/ci.yml       # 15 jobs: build, QEMU, audit, Kani full/PR, task-lint,
│                                  #         sntm-validate, sntm-pack, sntm-stack (SAFE-4), ...
├── deny.toml                      # cargo-deny policy (licenses, bans, sources)
├── sipahi.ld                      # Kernel linker script (kernel @ 0x80000000, _end ≤ 0x80600000)
├── sipahi.toml                    # SNTM manifest (tasks, regions, resources, channels, caps)
├── Makefile                       # build/run/check/kani/run-self-test/run-cross-isolation
├── Cargo.toml                     # workspace + features + overflow-checks + ed25519-compact pin
├── coverage.toml                  # feature/requirement → test/proof traceability (14F + 20R)
├── README.md                      # project overview
├── ARCHITECTURE.md                # layer/security/verification scope
├── CHANGELOG.md                   # sprint history (SAFE-1..4 + earlier)
├── LICENSE                        # Apache-2.0
│
├── src/                           # Kernel crate (no_std + no_alloc)
│   ├── main.rs                    # Entry point, panic handler
│   ├── boot.rs                    # Boot sequence (mtvec, PMP, blackbox, cap, tasks, IPC, timer)
│   ├── verify.rs                  # Cross-module Kani harnesses (204 toplam)
│   ├── tests/
│   │   └── mod.rs                 # POST + integration + FI + negative regression tests
│   ├── arch/                      # Layer 0: RISC-V hardware
│   │   ├── boot.S                 # _start → BSS clear → stack → rust_main
│   │   ├── trap.S                 # Trap frame save/restore (34 regs)
│   │   ├── trap.rs                # M-mode trap handler (timer, ecall, faults, MPP check)
│   │   ├── context.S              # switch_context (16 regs) + task_trampoline (mret)
│   │   ├── csr.rs                 # CSR read/write + mstatus MPP/MPIE constants
│   │   ├── pmp.rs                 # PMP register access + pack_pmpcfg
│   │   ├── clint.rs               # CLINT timer (drift-free mtimecmp scheduling)
│   │   ├── uart.rs                # NS16550A UART (transmit-ready check)
│   │   └── mod.rs
│   ├── hal/                       # Layer 1: Hardware abstraction
│   │   ├── device.rs              # DeviceAccess trait (static dispatch)
│   │   ├── iopmp.rs               # IOPMP stub (software emulation)
│   │   ├── key.rs                 # Ed25519 key provisioning (test-keys feature)
│   │   ├── secure_boot.rs         # Ed25519-compact RFC 8032 signature verify
│   │   └── mod.rs
│   ├── kernel/                    # Layer 2: Core kernel
│   │   ├── scheduler/
│   │   │   └── mod.rs             # Priority scheduler, budget, watchdog, U-mode trampoline
│   │   ├── capability/
│   │   │   ├── token.rs           # 32B capability token
│   │   │   ├── cache.rs           # 4-slot constant-time cache with TTL
│   │   │   ├── broker.rs          # BLAKE3 MAC, per-task nonce, token expiry
│   │   │   ├── cap_action.rs      # SAFE-2: CapAction 6-variant enum + from_u8
│   │   │   ├── cap_generated.rs   # SAFE-2 CODEGEN: LOCAL_CAP_TABLE + BOOT_CHANNELS
│   │   │   ├── local_cap.rs       # SAFE-2: local_cap_invoke syscall wrapper
│   │   │   └── mod.rs
│   │   ├── syscall/
│   │   │   ├── dispatch.rs        # 6-handler table, WCET tracking, pointer validation,
│   │   │   │                      # sys_cap_invoke reserved bits check (SAFE-2)
│   │   │   └── mod.rs             # Userspace ecall wrappers
│   │   ├── policy/
│   │   │   └── mod.rs             # PolicyEvent (StackOverflow=1), decide_action lockstep
│   │   ├── memory/
│   │   │   └── mod.rs             # PMP region setup + shadow integrity check
│   │   ├── loader/
│   │   │   └── mod.rs             # Native task loader (bounded_copy + zero_fill + PMP region)
│   │   ├── pmp/
│   │   │   ├── mod.rs             # PMP profile types
│   │   │   └── generated.rs       # CODEGEN: PMP_PROFILES from manifest (sntm-validate)
│   │   └── mod.rs
│   ├── ipc/                       # Layer 2: Communication
│   │   ├── mod.rs                 # SPSC lock-free ring buffer (&self API, CRC32)
│   │   └── blackbox.rs            # 128-record flight recorder (u64 monotonic tick)
│   └── common/                    # Cross-cutting
│       ├── config.rs              # Compile-time constants (STACK_ANALYSIS_MARGIN_BYTES=256,
│       │                          # STACK_ANALYSIS_UNKNOWN_SENTINEL=0xFFFF_FFFF — SAFE-4)
│       ├── types.rs               # TaskState, TaskConfig, Q32, newtypes
│       ├── error.rs               # SipahiError enum + as_str()
│       ├── sync.rs                # SingleHartCell<T> (zero static mut)
│       ├── fmt.rs                 # print_u32, print_u64, print_hex
│       ├── diagnostic.rs          # DiagStats + Diagnosable trait (v2.0)
│       ├── crypto/
│       │   ├── provider.rs        # HashProvider + SignatureVerifier traits
│       │   ├── blake3_impl.rs     # BLAKE3 keyed hash (no_std)
│       │   └── mod.rs             # Feature-gated provider selection
│       └── mod.rs
│
├── sipahi_api/                    # Task-side syscall + typed IPC API crate
│   ├── Cargo.toml                 # ed25519-compact, blake3 (no_std + no_alloc)
│   └── src/
│       ├── lib.rs                 # Error enum (8 variants) + from_kernel mapping;
│       │                          # syscall wrappers (cap_invoke, ipc_send/recv, yield,
│       │                          # task_info, exit, local_cap_invoke)
│       └── channels.rs            # SAFE-2 CODEGEN: typed send_<msg>/recv_<msg> per channel
│
├── tasks/
│   ├── task_hello/                # Native task #2 (task_id=2)
│   │   ├── Cargo.toml             # path dep sipahi_api with features = ["task_task_hello"]
│   │   ├── .cargo/config.toml     # task-scoped rustflags (-Tlinker, -relax, build-std=core)
│   │   ├── build.rs               # CARGO_MANIFEST_DIR linker script resolve
│   │   ├── task_hello.ld          # task linker @ 0x80600000
│   │   └── src/main.rs            # yield loop + ipc_send + ipc_recv + exit
│   └── task_world/                # Native task #3 (task_id=3) — SAFE-2 IPC consumer
│       ├── Cargo.toml
│       ├── build.rs
│       ├── task_world.ld          # task linker @ 0x80700000
│       └── src/main.rs            # yield loop + recv_GreetingPing (SAFE-2 typed)
│
├── tools/                         # Host-side toolchain (each sub-workspace)
│   ├── task-lint/                 # SAFE-1: syn 2.0 AST static analyzer
│   │   ├── Cargo.toml             # workspace=[]; cargo +stable; syn 2.0, walkdir
│   │   ├── .cargo/config.toml     # host target placeholder (no riscv inherit)
│   │   ├── src/
│   │   │   ├── lib.rs             # 11 forbidden rules export
│   │   │   ├── lint.rs            # AST visitor (~630 LOC, cfg-aware)
│   │   │   └── main.rs            # CLI (--manifest, --tasks-dir)
│   │   └── tests/integration.rs   # 18 tests (her kural ±çift + DAL × trust_tier matrix)
│   ├── sntm-validate/             # Manifest validator + codegen
│   │   ├── Cargo.toml             # toml 1.1.2, serde 1.0.228
│   │   ├── .cargo/config.toml
│   │   ├── src/
│   │   │   ├── main.rs            # CLI: --manifest, --output-rs/cap-table/channels,
│   │   │   │                      #      --call-stack-report + --task-name (SAFE-4)
│   │   │   ├── manifest.rs        # Manifest + KernelEntry + TaskEntry + ResourceEntry +
│   │   │   │                      # ChannelEntry + LocalCapGrant (stack_margin_override SAFE-4)
│   │   │   ├── validate.rs        # 10+ invariant check (id unique, NAPOT, overlap,
│   │   │   │                      # kernel-overlap, budget, SAFE-1 trust_tier × DAL,
│   │   │   │                      # SAFE-2 channel topology, SAFE-4 check_stack_bounds)
│   │   │   ├── codegen.rs         # PMP_PROFILES + LOCAL_CAP_TABLE + BOOT_CHANNELS + channels.rs
│   │   │   ├── napot.rs           # NAPOT encoding helpers
│   │   │   └── stackreport.rs     # SAFE-4: sntm-stack rapor parser
│   │   └── tests/integration.rs   # 32+ scenario test (SAFE-4 dahil)
│   ├── sntm-pack/                 # ELF → per-section .bin
│   │   ├── Cargo.toml             # object 0.36.5
│   │   ├── .cargo/config.toml
│   │   ├── src/...                # ELF section extract
│   │   └── tests/...
│   ├── riscv-bin-verify/          # SAFE-3: RV64IMAC opcode + region + symbol verify
│   │   ├── Cargo.toml             # object 0.36.5
│   │   └── src/
│   │       ├── decoder.rs         # 32-bit base + M + A + RVC decoder (~325 LOC)
│   │       ├── opcodes.rs         # whitelist tables (ALLOW: ecall; REJECT: F/D/CSR/mret/ebreak)
│   │       ├── parser.rs          # ELF section/symbol parser
│   │       ├── regions.rs         # kernel range check
│   │       ├── sections.rs        # symbol filter (STT_FILE/SECTION/SHN_ABS/UNDEF SKIP)
│   │       └── ...
│   │   └── tests/                 # 18 unit + 21 integration (synthetic ELF builder)
│   ├── sntm-cert-gen/             # SAFE-3: TaskCertificate generator
│   │   ├── Cargo.toml             # blake3, ed25519-compact, serde, toml
│   │   └── src/
│   │       ├── cert.rs            # TaskCertificate repr(C) 424B ABI v1
│   │       ├── chain.rs           # BLAKE3 hash chain + ed25519 sign/verify
│   │       ├── stackreport.rs     # SAFE-4: parser duplicate (FIX-G deferred)
│   │       ├── main.rs            # CLI: --manifest, --task-name/id, --signing-key, --out-*,
│   │       │                      #      --call-stack-report (SAFE-4)
│   │       └── lib.rs
│   │   └── tests/integration.rs   # 14 tests (RFC 8032 + tamper + SAFE-4 cert flow)
│   ├── sntm-image/                # SAFE-3: signed image assemble + verify
│   │   ├── Cargo.toml             # blake3, ed25519-compact, serde, toml
│   │   └── src/
│   │       ├── format.rs          # SIPI1 magic + 64B header + body + 64B tail sig
│   │       ├── sign.rs            # ed25519-compact sign + verify
│   │       ├── main.rs            # CLI: --manifest, --kernel, --task, --signing-key,
│   │       │                      #      --output | --verify + --pubkey
│   │       └── lib.rs
│   │   └── tests/integration.rs   # 11 tests (roundtrip + tamper + missing args ExitCode 2)
│   └── sntm-stack/                # SAFE-4 Plan B: stack analyzer
│       ├── Cargo.toml             # object 0.36.5 (riscv-bin-verify pin ile aynı)
│       └── src/
│           ├── lib.rs             # UNKNOWN_SENTINEL + REPORT_VERSION export
│           ├── elf.rs             # object crate ELF parse + ULEB128 .stack_sizes decode
│           ├── decode.rs          # AUIPC+JALR pair detect + JAL + c.j + indirect classifier
│           ├── analysis.rs        # frame map + call graph + DFS cycle + sum-of-frames
│           ├── report.rs          # text rapor format (SNTM-STACK v1.0 banner)
│           └── main.rs            # CLI: --bin, --output
│       ├── tests/integration.rs   # 9 tests (synthetic ELF + CLI ExitCode 2 + golden fixture)
│       └── tests/elf_builder.rs   # Minimal RV64 ELF builder (.text + .stack_sizes + .symtab)
│       └── tests/fixtures/        # 3 golden fixture (.golden.txt — committed)
│
├── Tla+/                          # TLA+ specs (9 specs, all TLC verified)
│   ├── SipahiScheduler.tla + .cfg
│   ├── SipahiCapability.tla + .cfg
│   ├── SipahiPolicy.tla + .cfg
│   ├── SipahiWatchdog.tla + .cfg
│   ├── SipahiDegradeRecover.tla + .cfg
│   ├── SipahiBudgetFairness.tla + .cfg
│   ├── SipahiIPC.tla + .cfg
│   ├── SipahiSNTM.tla + .cfg      # SAFE-4 StackRegionBound invariant ek (138 states)
│   ├── SipahiSecureBoot.tla + .cfg # SAFE-3 image verify state machine (6 states)
│   ├── run_tlc.sh                 # tüm 9 spec'i ardı sıra koşturucu
│   ├── tla2tools.jar
│   └── results/                   # TLC çalıştırma çıktıları + README
│
├── scripts/
│   ├── sntm_safe_gate.sh          # SAFE umbrella gate (10/10 active — DEFER yok)
│   ├── stack_analysis.sh          # SAFE-4 sntm-stack runner (env -u RUSTFLAGS)
│   ├── check_coverage.sh          # coverage.toml ↔ source name traceability
│   ├── check_proof_quality.sh     # Kani harness adequacy heuristic
│   ├── feature_matrix.sh          # 10 feature kombinasyonu build
│   ├── sipahi_sprint_gate.sh      # legacy kernel sprint umbrella
│   ├── sntm_sprint_gate.sh        # SNTM v1.x umbrella
│   ├── build_native_tasks.sh      # task ELF → .bin pipeline (sntm-pack call)
│   ├── regen_pmp_profiles.sh      # manifest → pmp/generated.rs codegen
│   ├── regen_safe_codegen.sh      # manifest → cap_generated + channels codegen
│   ├── gen_dev_key.sh             # openssl ed25519 development keypair bootstrap
│   ├── check_cross_isolation.sh   # SNTM-R12 4-gate runtime PMP isolation
│   ├── u19_find_missing_safety.sh # // SAFETY: undocumented-block scanner
│   └── verify-ct-eq.sh            # constant-time helper objdump inspection
│
├── docs/
│   ├── sipahi_context.md          # New-chat context doc (local, not tracked)
│   ├── sipahi_features_tr.md      # Technical features (TR)
│   ├── sipahi_features_en.md      # Technical features (EN)
│   └── safe/
│       └── cert_abi_v2_migration.md  # SAFE-4 G8: ABI v2 plan (post-CFI, doc only)
│
└── keys/                          # ed25519 dev keypair (private gitignored)
    ├── .gitignore                 # *.priv deny + !.gitignore + !*.pub allow
    ├── dev-image.priv             # GITIGNORED (gen_dev_key.sh ile bootstrap)
    └── dev-image.pub              # COMMITTED (CI roundtrip verify için)
```

## Stats (post-SAFE-4)

| Metric | Value |
|---|---|
| Kernel Rust LOC (`src/` + `sipahi_api/` + `tasks/`) | ~12,500 |
| Tool Rust LOC (`tools/`) | ~9,200 |
| ASM LOC (`src/arch/*.S`) | ~332 |
| Kernel `.rs` files | 49 |
| Tool `.rs` files | 61 |
| ASM `.S` files | 3 |
| **Kani harnesses** | **204** (SAFE-1..4 hepsi PASS) |
| **TLA+ specs** | **9/9 PASS** (SipahiSNTM 138 states + SecureBoot 6 + 7 legacy) |
| Compile-time `const _: () = assert!` | 10+ |
| `static mut` count | 0 (all via `SingleHartCell<T>`) |
| `unsafe` blocks (kernel) | ~162 (majority `// SAFETY:` documented) |
| Sprints completed | Core 0–14 + U-3..U-22 + U-23..U-33 (SNTM Phase 1..5 + SAFE-1..4) |
| **SAFE gate** | **10/10 active** (DEFER yok — SAFE faz kapanışı) |
| **coverage.toml** | **14 feature + 20 requirement** (SNTM-R1..R14 + SNTM-SAFE-R1..R6) |
| CI jobs | 15 (build, qemu, audit, Kani full/PR, task-lint, sntm-*, ...) |
| Supply chain | `cargo audit` + `cargo deny` (license/bans/sources policy) |

## Post-Sprint Checklist

After each sprint, run the below and update the metrics in this file:

- [ ] `find src/ -name '*.rs' \| xargs wc -l \| tail -1` → kernel Rust LOC
- [ ] `find tools/ -name '*.rs' \| xargs wc -l \| tail -1` → tool Rust LOC
- [ ] `grep -rc 'kani::proof' src/ \| awk -F: '{s+=$2} END {print s}'` → Kani count
- [ ] `grep -rc 'unsafe {' src/ \| awk -F: '{s+=$2} END {print s}'` → unsafe count
- [ ] `bash Tla+/run_tlc.sh` → TLA+ 9/9 PASS
- [ ] `cargo clippy -- -D warnings` → 0 warnings
- [ ] `cargo kani` → 204 PASS (SAFE faz sonrası)
- [ ] `cargo audit && cargo deny check` → clean
- [ ] `bash scripts/sntm_safe_gate.sh` → 10/10 active PASS
- [ ] `bash scripts/check_coverage.sh` → 14F + 20R traceable
- [ ] `bash scripts/check_cross_isolation.sh` → 4-gate PASS
- [ ] `make run-self-test` → ALL TESTS PASSED
- [ ] Increment sprint count + update CHANGELOG.md
- [ ] (After explicit user approval) `git tag -a v<X.Y.Z> -m "..."`
