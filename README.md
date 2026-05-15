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
- Kernel version: `1.1.1`
- License: Apache-2.0
- Toolchain: pinned Rust nightly with `build-std=core,alloc`

Implemented in the current tree:

- Boot, trap entry, timer interrupt, context switch, and U-mode entry path
- Fixed-priority scheduler with budget accounting, watchdog logic, and task
  state transitions
- PMP-based kernel and task memory isolation
- Six syscall IDs: capability invoke, IPC send, IPC receive, yield, task info,
  and task exit
- Capability broker with keyed MAC tokens, nonce checks, owner checks, and a
  small validation cache
- SPSC IPC channels with ownership assignment, sealing, CRC support, and rate
  limiting
- Policy engine for restart/isolate/degrade/alert/shutdown style actions
- Blackbox flight recorder with CRC-protected records
- Secure-boot verification path using Ed25519 in the current prototype
- Optional WASM sandbox path behind `wasm-sandbox`; this is being phased out
  in favor of SNTM
- SNTM Phase 1 scaffolding: `sipahi_api`, `task_hello`, `sipahi.toml`, and
  task-side syscall wrappers

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
    capability/         Token, broker, and validation cache
    memory/             PMP setup and shadow checks
    policy/             Failure-policy engine
    scheduler/          Task table, scheduling, budget, watchdog
    syscall/            Syscall ABI, dispatch table, WCET tracking
  sandbox/              WASM prototype path, gated by `wasm-sandbox`
  tests/                Self-test and regression harness
  verify.rs             Cross-module Kani harnesses

sipahi_api/             Task-side SNTM API crate
tasks/task_hello/       Standalone native task scaffold
Tla+/                   TLA+ specs and TLC run artifacts
scripts/                Sprint, coverage, proof-quality, and feature gates
.github/workflows/     CI jobs
```

Key top-level files:

- `Cargo.toml`: workspace, features, dependency policy
- `Makefile`: kernel build/run/check commands
- `sipahi.ld`: kernel linker script
- `sipahi.toml`: SNTM manifest scaffold
- `coverage.toml`: feature to test/proof traceability map
- `CHANGELOG.md`: sprint history and release notes
- `ARCHITECTURE.md`: deeper design notes and limitations

## Build And Run

Install the Rust target and `rust-src`:

```bash
rustup target add riscv64imac-unknown-none-elf
rustup component add rust-src
```

Common commands:

```bash
make build          # release kernel build
make run            # run production build in QEMU
make run-self-test  # run POST + integration/self-test build
make check          # clippy with warnings denied
make kani           # run Kani harnesses
```

The kernel linker script is passed through `Makefile` via `KERNEL_RUSTFLAGS`.
This keeps the root Cargo config from leaking the kernel linker script into
native SNTM task crates.

## Verification And Development Gates

Sipahi uses several layers of checking. None of them alone proves the kernel
correct; the point is to catch different classes of mistakes early.

Current verification assets:

- 198 Kani harnesses in the current tree
- 7 TLA+ models under `Tla+/`
- self-test and regression suite under `src/tests/`
- feature-matrix builds for supported feature combinations
- coverage map checks for feature/test/proof drift
- light proof-quality scan for trivial or stale Kani harnesses
- CI checks for build, QEMU smoke tests, Kani, audit/deny, binary guards, and
  constant-time helper inspection

Useful gate commands:

```bash
bash scripts/sipahi_sprint_gate.sh
bash scripts/sntm_sprint_gate.sh
bash scripts/check_coverage.sh
bash scripts/check_proof_quality.sh
bash scripts/feature_matrix.sh
```

Important caveat: name-based coverage checks are mechanical guards. They do not
prove that a test or proof is semantically strong. New verification items should
state the requirement they verify, the production functions they call, and the
fault model that would make them fail.

## SNTM Transition

Sipahi is moving away from the WASM prototype path toward SNTM: a native-task
model that keeps isolation in hardware and pushes as much validation as possible
to build time.

Current SNTM pieces:

- `sipahi_api`: `no_std` task-side syscall API
- `tasks/task_hello`: standalone native task scaffold
- `sipahi.toml`: manifest scaffold for task memory layout and metadata
- `SYS_EXIT`: sixth syscall for voluntary task termination
- `sntm` and `sntm-safe`: default-off feature flags

Not implemented yet:

- manifest validator (`sntm-validate`)
- generated PMP profile tables
- native task image packing and loading
- runtime multi-region PMP reload from SNTM profiles
- typed IPC generation
- binary verifier and task certificate flow
- full SNTM runtime tests with booted native tasks

The current rule is simple: partial SNTM work must remain default-off and must
not silently change the production kernel path.

## Security Model Summary

Sipahi relies on a small set of explicit mechanisms:

- kernel code runs in Machine mode
- task code runs in User mode
- PMP protects kernel memory and task memory regions
- syscalls are routed through a fixed dispatch table
- task pointers are validated before kernel use
- capabilities bind authority to a task owner
- IPC channels have assigned producer/consumer ownership
- scheduler state and watchdog behavior are checked by tests and Kani harnesses

The project currently assumes a single-hart runtime. Multi-hart work is tracked
separately in AMCI/SNTM design documents and is not part of the current kernel
runtime.

## Known Limitations

- Hardware WCET numbers are estimates until measured on target silicon.
- QEMU does not model all cache, bus, PMP, and platform-interference behavior.
- `test-keys` is enabled in the default development build; production key
  provisioning is a separate target path.
- WASM support is prototype-only and gated by `wasm-sandbox`.
- SNTM is incomplete; the current code contains Phase 1 scaffolding, not a full
  native-task deployment pipeline.
- IOPMP, SPMP, WorldGuard, CLIC, hardware CFI, and CHERI-style work are roadmap
  topics, not current runtime guarantees.

## Feature Flags

Common flags:

- `self-test`: enables POST, integration tests, trace, debug boot output, and
  WASM sandbox support for tests
- `trace`: verbose runtime tracing
- `debug-boot`: boot-time diagnostic output
- `wasm-sandbox`: optional WASM prototype path
- `v2-hal`: HAL/device abstraction work
- `sntm`: SNTM base work, default-off
- `sntm-safe`: future SNTM hardening layers, default-off
- `production-otp`: production key provisioning path; requires deployment-side
  integration

## Documentation

- `ARCHITECTURE.md`: architecture, isolation model, verification scope
- `CHANGELOG.md`: sprint history and release notes
- `STRUCTURE.md`: repository structure
- `SIPAHI_V1_TO_V2_TRANSITION.md`: migration notes toward SNTM and v2 work
- `Tla+/results/README.md`: TLA+ run notes

## License

Apache-2.0. See [LICENSE](LICENSE).
