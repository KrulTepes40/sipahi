# Sipahi

Sipahi is an experimental bare-metal microkernel for RISC-V, written in Rust.
It is built around deterministic scheduling, hardware memory isolation, small
trusted components, and explicit verification gates.

The project is currently developed against QEMU `virt` and a CVA6-style
RV64IMAC target. It is **not certified** and should be read as a research and
engineering prototype, not as a deployable safety-critical product.

## Current Status

- Target ISA: `riscv64imac-unknown-none-elf`
- Execution model: Machine-mode kernel, User-mode tasks
- Primary platform today: QEMU `virt`, single hart
- Kernel version: `1.1.1` (kernel crate); SNTM-SAFE phase **v1.9.0** (sprint-u33)
- License: Apache-2.0
- Toolchain: pinned `nightly-2026-03-01` with `build-std=core,alloc`
- Memory model: pure `no_std + no_alloc` (heap kaldırıldı U-29 v2.0)

Implemented in the current tree:

- Boot, trap entry, timer interrupt, context switch, and U-mode entry path
- Fixed-priority scheduler with budget accounting, watchdog logic, and task
  state transitions
- PMP-based kernel and task memory isolation (multi-region SNTM profiles
  manifest-driven)
- Six syscall IDs: capability invoke, IPC send, IPC receive, yield, task info,
  task exit
- Capability broker with keyed MAC tokens, nonce checks, owner checks, and a
  small validation cache
- SPSC IPC channels with ownership assignment, sealing, CRC support, rate
  limiting; **typed IPC API codegen** from manifest (SAFE-2)
- Static `LOCAL_CAP_TABLE` capability column (SAFE-2 §17.5)
- Policy engine for restart/isolate/degrade/alert/shutdown style actions
- Blackbox flight recorder with CRC-protected records
- Secure-boot verification path using Ed25519 (`ed25519-compact`, no_alloc)
- **SNTM Native Task Model (v2.0)**: `sipahi_api`, `task_hello` + `task_world`
  native tasks, `sipahi.toml` manifest-driven PMP profiles, generic
  `load_native_task` loader, sealed channels, cross-task PMP isolation
  (statik kanıt + runtime gate)
- **SNTM-SAFE phased rollout complete**:
  - **SAFE-1** (v1.6.1) task-lint Safe Native Profile (11 yasak kural,
    DAL-aware `trust_tier` enforcement)
  - **SAFE-2** (v1.7.0) typed IPC channels + static cap table codegen
  - **SAFE-3** (v1.8.0) RISC-V binary verifier + TaskCertificate + signed
    image (ed25519 RFC 8032)
  - **SAFE-4** (v1.9.0) build-time stack analyzer (Plan B: cargo-call-stack
    0.1.16 toolchain drift → LLVM `-Z emit-stack-sizes` ELF section + sntm-stack
    direkt parse); safe gate 10/10 aktif
- Historical: WASM sandbox path (`wasm-sandbox` feature + `wasmi`) was in v1.x
  but was completely removed in U-29 v2.0 (no_alloc kernel doctrine)

## What Sipahi Is Not

Sipahi is not a certified RTOS, not a seL4 replacement, and not a production
DAL-A kernel. The code applies several safety-critical design practices, but
certification requires artifacts that are outside this repository today:

- hardware WCET measurements
- independent review
- requirements traceability to a formal safety case
- tool qualification
- hardware fault-injection campaigns
- target-board driver maturity
- certification evidence and process documents

The repository is intentionally explicit about these limits.

## Repository Layout

```text
src/
  arch/                 RISC-V boot, trap, CSR, PMP, CLINT, UART, context switch
  common/               Configuration, formatting, sync primitives, crypto helpers
  hal/                  Device abstraction, IOPMP/key/secure-boot stubs
  ipc/                  SPSC channels and blackbox recorder
  kernel/
    capability/         Token, broker, validation cache + SAFE-2 cap_action /
                        cap_generated / local_cap modules
    loader/             Native task loader (boot-time PMP region setup)
    memory/             PMP setup and shadow checks
    pmp/                PMP profile types + manifest-driven generated.rs
    policy/             Failure-policy engine
    scheduler/          Task table, scheduling, budget, watchdog
    syscall/            Syscall ABI, dispatch table, WCET tracking
  tests/                Self-test and regression harness
  verify.rs             Cross-module Kani harnesses (204 total, SAFE-4 incl.)

sipahi_api/             Task-side SNTM API crate (syscalls + typed IPC channels)
tasks/task_hello/       Native task (id=2)
tasks/task_world/       Native task (id=3, SAFE-2 typed IPC consumer)

tools/                  Host-side toolchain (each tool sub-workspace; build with
                        cd tools/<crate> && cargo +stable build)
  task-lint/            SAFE-1 syn 2.0 AST static analyzer (11 yasak kural)
  sntm-validate/        Manifest validator + codegen (PMP_PROFILES,
                        LOCAL_CAP_TABLE, BOOT_CHANNELS, typed channels.rs) +
                        SAFE-4 stack bound check
  sntm-pack/            ELF → per-section .bin (text/rodata/data)
  riscv-bin-verify/     SAFE-3 RV64IMAC opcode whitelist + region + symbol filter
  sntm-cert-gen/        SAFE-3 TaskCertificate (424B ABI v1) + ed25519 sign
  sntm-image/           SAFE-3 signed image assemble + verify (SIPI1 magic)
  sntm-stack/           SAFE-4 Plan B: .stack_sizes ELF parser + AUIPC+JALR
                        pair detect + recursion cycle detect + sum-of-frames

Tla+/                   TLA+ specs and TLC run artifacts (9 specs)
scripts/                Sprint, coverage, proof-quality, feature, SAFE gates
  sntm_safe_gate.sh     10/10 active SAFE umbrella gate (SAFE-1..4 birleşik)
  stack_analysis.sh     SAFE-4 sntm-stack runner (env -u RUSTFLAGS)
  check_coverage.sh     coverage.toml ↔ source name-based mechanical guard
.github/workflows/      CI: build, QEMU, audit, Kani, task-lint, sntm-*, ...
docs/safe/              SAFE doctrine notları (cert_abi_v2_migration plan v.s.)
keys/                   ed25519 dev keypair (private gitignored)
```

Key top-level files:

- `Cargo.toml`: workspace, features, dependency policy
- `Makefile`: kernel build/run/check commands
- `sipahi.ld`: kernel linker script
- `sipahi.toml`: SNTM manifest (tasks, regions, resources, channels, caps)
- `coverage.toml`: feature/requirement to test/proof traceability map
- `CHANGELOG.md`: sprint history and release notes
- `ARCHITECTURE.md`: deeper design notes and limitations

## Build And Run

The pinned toolchain (`rust-toolchain.toml`) installs nightly-2026-03-01 with
`rust-src`, `clippy`, `llvm-tools-preview` and the `riscv64imac-unknown-none-elf`
target automatically:

```bash
rustup toolchain install nightly-2026-03-01   # auto via rust-toolchain.toml
```

Common commands:

```bash
make build          # release kernel build
make run            # run production build in QEMU
make run-self-test  # run POST + integration/self-test build
make check          # clippy with warnings denied
make kani           # run all 204 Kani harnesses
make run-cross-isolation  # SNTM-R12 runtime PMP isolation gate (4-gate)

# SAFE gate (10/10 aktif)
bash scripts/sntm_safe_gate.sh

# SAFE-4 stack analiz runner (sntm-stack)
bash scripts/stack_analysis.sh

# Coverage map check (feature/requirement → test/proof traceability)
bash scripts/check_coverage.sh
```

The kernel linker script is passed through `Makefile` via `KERNEL_RUSTFLAGS`.
This keeps the root Cargo config from leaking the kernel linker script into
host tool builds (SAFE-2 lesson; `env -u RUSTFLAGS` doctrine for tools).

## Verification And Development Gates

Sipahi uses several layers of checking. None of them alone proves the kernel
correct; the point is to catch different classes of mistakes early.

Current verification assets:

- **204 Kani harnesses** in the current tree (post-SAFE-4; SAFE-1..4 hepsi PASS)
- **9 TLA+ specifications** under `Tla+/` (SipahiScheduler, Capability, Policy,
  Watchdog, DegradeRecover, BudgetFairness, IPC, SNTM, SecureBoot)
- self-test and regression suite under `src/tests/`
- feature-matrix builds for supported feature combinations
- coverage map checks for feature/requirement/test/proof drift (`SNTM-R*` +
  `SNTM-SAFE-R1..R6` traceability)
- light proof-quality scan for trivial or stale Kani harnesses
- **SAFE gate 10/10 aktif** (DEFER yok — phased rollout kapandı)
- CI checks: build, QEMU smoke tests, Kani, audit/deny, binary guards,
  constant-time helper inspection, task-lint, sntm-validate, sntm-pack,
  sntm-stack (SAFE-4)

Useful gate commands:

```bash
bash scripts/sipahi_sprint_gate.sh   # legacy kernel sprint umbrella
bash scripts/sntm_sprint_gate.sh     # SNTM v1.x umbrella
bash scripts/sntm_safe_gate.sh       # SAFE-1..4 umbrella (10/10 active)
bash scripts/stack_analysis.sh       # SAFE-4 sntm-stack run
bash scripts/check_coverage.sh       # feature/requirement traceability
bash scripts/check_proof_quality.sh  # Kani harness adequacy heuristic
bash scripts/feature_matrix.sh       # supported feature combinations
```

Important caveat: name-based coverage checks are mechanical guards. They do not
prove that a test or proof is semantically strong. New verification items should
state the requirement they verify, the production functions they call, and the
fault model that would make them fail (`// VERIFIES: ...` / `// CALLS: ...` /
`// FAILS-IF: ...` triple).

## SNTM Status (post-SAFE-4)

SNTM is the **native-task** model: hardware-isolated tasks, build-time
validation, sealed channels, signed image. The phased rollout is complete:

| Phase | Sprint | Status | Highlights |
|-------|--------|--------|-----------|
| Native task model | U-23..U-27.5 | ✅ | sipahi_api + task_hello + task_world, manifest, multi-region PMP, sealed channels, cross-task isolation runtime |
| SAFE-1 | U-30 | ✅ | task-lint (11 rules, DAL-aware trust_tier), [2/10] gate |
| SAFE-2 | U-31 | ✅ | typed IPC channels.rs, LOCAL_CAP_TABLE, BOOT_CHANNELS, [3/10] + [7/10] + [8/10] gates |
| SAFE-3 | U-32 | ✅ | riscv-bin-verify (RV64IMAC whitelist), TaskCertificate (424B ABI v1), signed image (SIPI1), [4/10] + [9/10] + [10/10] gates |
| SAFE-4 | U-33 | ✅ | sntm-stack Plan B (`.stack_sizes` parser), check_stack_bounds (margin 256), cert max_stack_bytes refinement (UNKNOWN sentinel), [5/10] gate; **safe gate 10/10 aktif** |

Carry-forward (post-SAFE faz):

- CFI hardware faz (Zicfilp landing pad + Zicfiss shadow stack — CVA6-CFI
  olgunluğu beklenir)
- Stack scribble debug-boot redesign (low-watermark region-bottom scan;
  SAFE-4 CR-6 doctrine ile post-SAFE'e taşındı)
- Production HSM/OTP key sprint (`keys/dev-image.priv` → HSM-provisioned)
- TaskCertificate ABI v2 (CFI landing pad list, post-quantum sig migration —
  `docs/safe/cert_abi_v2_migration.md` plan)
- Shared `sntm-manifest` crate (SAFE-2 FIX-G; sntm-validate + riscv-bin-verify
  + sntm-cert-gen + sntm-stack manifest struct unification)

## Security Model Summary

Sipahi relies on a small set of explicit mechanisms:

- kernel code runs in Machine mode
- task code runs in User mode
- PMP protects kernel memory and task memory regions (SNTM multi-region
  manifest-driven)
- syscalls are routed through a fixed dispatch table
- task pointers are validated before kernel use
- capabilities bind authority to a task owner (Token MAC + nonce + cache TTL)
- static `LOCAL_CAP_TABLE` per-task action enforcement (SAFE-2)
- IPC channels have assigned producer/consumer ownership; sealed atomicity
  invariant
- typed IPC API (codegen from manifest) — wrong message struct = compile error
- Safe Native Profile (SAFE-1) — task source-level safety lints (11 rules,
  DAL-aware trust_tier; production binary unsafe leak guard)
- RV64IMAC opcode whitelist (SAFE-3) — F/D/CSR/mret/ebreak reject; ecall ALLOW
- TaskCertificate (SAFE-3) — BLAKE3 chain (manifest, toolchain, source_commit,
  text/rodata/data) + ed25519 sig; forensics metadata bundle
- Signed image (SAFE-3) — `SIPI1` magic + 64B header + body + 64B tail
  ed25519 sig
- Build-time stack analysis (SAFE-4) — `.stack_sizes` ELF parser + indirect
  call/recursion reject + margin enforce (`STACK_ANALYSIS_MARGIN_BYTES=256`)
- scheduler state and watchdog behavior are checked by tests and Kani harnesses

The project currently assumes a single-hart runtime. Multi-hart work is tracked
separately in AMCI/SNTM design documents and is not part of the current kernel
runtime.

## Known Limitations

- Hardware WCET numbers are estimates until measured on target silicon.
- QEMU does not model all cache, bus, PMP, and platform-interference behavior.
- `test-keys` is enabled in the default development build; production key
  provisioning is a separate target path (post-SAFE HSM sprint).
- SAFE-4 stack analyzer uses **sum-of-frames over-approximation**
  (call-graph-aware transitive analysis is post-SAFE work). task_hello
  observed 128B + 256B margin = 384B << 8KB stack region — comfortable.
- `cargo-call-stack 0.1.16` is incompatible with current nightly (rustc
  intercept 2023-11 hard-coded); SAFE-4 uses **Plan B** (`-Z emit-stack-sizes`).
- Runtime stack-overflow detection (watermark) is **not** in SAFE-4 — it was
  deferred (CR-6: RISC-V downward stack growth, "stack top -8 sentinel"
  pattern is incorrect; post-SAFE redesign with low-watermark / region-bottom
  scan).
- IOPMP, SPMP, WorldGuard, CLIC, hardware CFI, and CHERI-style work are roadmap
  topics, not current runtime guarantees.

## Feature Flags

Common flags:

- `self-test`: enables POST, integration tests, trace, debug boot output
- `trace`: verbose runtime tracing
- `debug-boot`: boot-time diagnostic output
- `cross-isolation-demo`: U-27.5 cross-task PMP runtime ihlal observation
  (opt-in; production default-OFF compile-out + CI unsafe leak guard)
- `v2-hal`: HAL/device abstraction work (v2.0)
- `sntm`: SNTM base umbrella feature (default-off)
- `sntm-safe`: SAFE-1..4 hardening umbrella (default-off)
- `test-keys`: QEMU/dev ed25519 key (default-on)
- `production-otp`: production key provisioning path; requires deployment-side
  symbol (link-time guard against accidental production builds)

Removed in U-29 v2.0:

- `wasm-sandbox`: Wasmi 1.0.9 + bump allocator path. WASM completely removed;
  kernel is now pure `no_std + no_alloc`.

## Documentation

- `ARCHITECTURE.md`: architecture, isolation model, verification scope
- `CHANGELOG.md`: sprint history and release notes (SAFE-1..4 + earlier)
- `STRUCTURE.md`: repository structure (post-SAFE-4 layout)
- `docs/safe/cert_abi_v2_migration.md`: ABI v2 migration plan (post-CFI)
- `Tla+/results/README.md`: TLA+ run notes
- `docs/sipahi_features_tr.md` / `docs/sipahi_features_en.md`: technical
  feature inventory (TR + EN)

## License

Apache-2.0. See [LICENSE](LICENSE).
