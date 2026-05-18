# Sipahi — Technical Feature Inventory

This document summarizes the features that exist in the current Sipahi working
tree. It deliberately separates implemented behavior, partial scaffolding, and
roadmap items.

**Status:** kernel `v1.1.1` + SNTM Native Task Model v2.0 +
**SNTM-SAFE phased rollout complete (v1.9.0, sprint-u33)**
**Target:** RISC-V RV64IMAC, QEMU `virt`, single hart
**Language:** Rust `no_std + no_alloc`, bare metal
**Verification assets:** **204 Kani harnesses, 9 TLA+ models**, self-test +
SAFE gate (10/10 active), coverage map (14F + 20R)

Sipahi is not a certified RTOS. It applies several safety-critical design
practices, but certification would require additional evidence such as hardware
WCET reports, independent review, requirements traceability, a safety case, and
tool qualification.

---

## 1. Core Architecture

- The kernel runs in Machine mode.
- Tasks run in User mode.
- S-mode and virtual memory are not used; memory isolation is PMP-based.
- Boot, trap entry, timer interrupt, and context switching are supported by
  RISC-V assembly.
- The current runtime assumes a single hart. Multi-hart/AMCI work is tracked in
  separate design documents and is not part of the current runtime.

### Main modules

- `src/arch`: boot, trap, CSR, PMP, CLINT, UART, context switch
- `src/kernel/scheduler`: task table, priority selection, budget, watchdog
- `src/kernel/syscall`: syscall ABI, dispatch table, WCET tracking
- `src/kernel/capability`: token, broker, cache; SAFE-2 `cap_action` /
  `cap_generated` / `local_cap` modules
- `src/kernel/loader`: SNTM native task loader (bounded copy + zero fill + PMP)
- `src/kernel/pmp`: manifest-driven `PMP_PROFILES` codegen target
- `src/ipc`: SPSC IPC channels + blackbox recorder
- `src/kernel/policy`: failure policy engine with lockstep
- `sipahi_api`: task-side syscall ABI + typed IPC `channels.rs` codegen target
- `tasks/task_hello`: native task #2
- `tasks/task_world`: native task #3 (SAFE-2 typed IPC consumer)
- **Host tool ecosystem** (`tools/`): task-lint, sntm-validate, sntm-pack,
  riscv-bin-verify, sntm-cert-gen, sntm-image, sntm-stack — each sub-workspace,
  each invoked by `bash scripts/sntm_safe_gate.sh`.
- (v1.x `src/sandbox/` WASM prototype was completely removed in U-29 v2.0.)

---

## 2. Privilege And Trap Model

### Implemented

- Machine-mode kernel, User-mode tasks.
- Task entry through `mret` with `mstatus.MPP = U`.
- `mscratch`-based trap stack swap.
- `task_trampoline` resumes fresh tasks after context switch.
- The U-18 nested-fault fix restored the `mscratch` and user-stack invariant
  before returning to U-mode.
- U-19 clears caller-saved registers before task entry to reduce information
  leakage.
- `mcounteren = 0`; U-mode counter access is disabled.
- `medeleg/mideleg` are cleared for the M-only kernel model.

### Limits

- Trap latency and PMP behavior still require FPGA/silicon measurement.
- Multi-hart trap and interrupt behavior is not implemented in the runtime.

---

## 3. PMP And Memory Protection

### Implemented

- PMP protects kernel `.text`, `.rodata`, `.data+bss+kernel_stack`, and UART
  MMIO regions.
- Kernel regions use the L-bit where appropriate.
- Task stacks live in NAPOT-aligned regions per task (manifest-driven).
- Per-task PMP profiles loaded on context switch (SNTM multi-region §17 model).
- `sfence.vma zero, zero` is issued after per-task PMP writes.
- PMP shadow integrity is checked from the scheduler tick path.
- `is_valid_user_ptr(caller_task_id, ptr, size)` is task-specific: it only
  accepts addresses inside the caller task's own region range.
- Dead, isolated, or uninitialized tasks have no valid user pointer range.

### SNTM (Native Task Model, v2.0)

- `sipahi.toml` manifest scaffold (kernel, platform, tasks, regions, resources,
  channels, local_caps) — fully populated.
- `src/kernel/pmp/generated.rs` (`PMP_PROFILES`) emitted from manifest by
  `sntm-validate --output-rs` (CODEGEN; CI drift guard active).
- Runtime multi-region PMP reload working (SNTM-R6/R7/R8 Kani-verified).
- Cross-task PMP isolation runtime gate (4-gate: trap isolation, no BROKEN
  marker, post-trap tick continuity, no FATAL/NF/POLICY).

---

## 4. Scheduler

### Implemented

- Fixed-priority preemptive scheduler.
- `MAX_TASKS = 8`.
- Task states include Ready, Running, Suspended, Isolated, and Dead.
- Priority selection helpers are covered by Kani harnesses.
- `schedule_timer_tick()` and `schedule_yield()` are separate:
  - the timer path advances period, budget, watchdog, blackbox tick, and PMP
    integrity checks.
  - the yield path only performs priority selection and context switching.
- Budget accounting uses saturating arithmetic.
- Watchdog counters increase only for Running tasks.
- Degrade/recovery has a cooldown to reduce flapping.
- `isolate_task()` marks a task isolated and invalidates its capabilities.
- `SYS_EXIT` isolates the current task and yields to the scheduler.

### Limits

- WCET values are estimates until measured on real target hardware.
- Task migration and multi-hart scheduling are not implemented.

---

## 5. Syscall ABI

### Implemented syscalls

| ID | Syscall | Purpose |
|---:|---|---|
| 0 | `cap_invoke` | capability check (BLAKE3 MAC + nonce + owner) |
| 1 | `ipc_send` | non-blocking IPC send |
| 2 | `ipc_recv` | non-blocking IPC receive |
| 3 | `yield` | yield to scheduler |
| 4 | `task_info` | query task state/priority/DAL |
| 5 | `exit` | voluntary task termination |
| (n/a) | `local_cap_invoke` | SAFE-2 typed cap action (uses cap_invoke ID with extra check) |

`SYSCALL_COUNT = 6`. The SAFE-2 `local_cap_invoke` is dispatched through
`sys_cap_invoke` with `LOCAL_CAP_TABLE[task][resource]` lookup; argv reserved
bits are checked for forward compatibility.

### Implemented guards

- O(1) function-pointer dispatch table.
- Invalid syscall ID returns `E_INVALID_SYSCALL`.
- Kernel-looking return values are sanitized to `E_INTERNAL`.
- IPC pointers are validated with task-specific pointer checks and alignment
  checks before use.
- `sys_cap_invoke` rejects truncation-prone arguments + SAFE-2 reserved bits.
- `rdcycle` records last/max syscall cycles.
- `check_wcet_limits()` covers all six syscalls.
- `Error::from_kernel` (sipahi_api) maps 8 raw values (SAFE-3 CR-1 alignment).

---

## 6. Capability System

### Implemented

- 32-byte `Token` structure.
- BLAKE3 keyed MAC validation path.
- `ct_eq_16` constant-time MAC comparison.
- 4-slot validation cache.
- Per-task nonce replay guard.
- Token expiry check.
- Token owner enforcement.
- Cache invalidation by token/owner and capability revocation on task isolation.
- **SAFE-2 static `LOCAL_CAP_TABLE`** — per-task `[task][resource] → CapAction`
  enforcement, emitted from `sipahi.toml [[task.local_cap]]` by sntm-validate
  CODEGEN. Manifest dışı action (None/Read/Write/ReadWrite/Execute/All)
  syscall'da reject; runtime drift'i CI drift gate'i yakalar.
- `production-otp` is a production provisioning path that expects an external
  deployment-side symbol; accidental production builds fail at link time.
- `test-keys` is the default development/CI path.

### Limits

- BLAKE3 is the current fast prototype MAC. SHA-2/Zknh or other CNSA-aligned
  paths are roadmap work.
- Kani does not prove cryptographic strength; it checks bounds, ordering, and
  API-use invariants around the crypto calls.

---

## 7. IPC

### Implemented

- 8 static SPSC channels (`MAX_IPC_CHANNELS = 8`).
- 16 slots per channel.
- 64-byte messages (`IPC_MSG_SIZE = 64`).
- `AtomicU16` head/tail with Release/Acquire ordering.
- O(1), non-blocking `send` and `recv`.
- Channel producer/consumer ownership is assigned at boot and then sealed.
- Unassigned channels are default-deny.
- `can_send` / `can_recv` enforce ownership.
- CRC32 helper methods exist (`set_crc`, `verify_crc`).
- IPC send rate limiting is applied per tick.
- **SAFE-2 typed IPC**: `sipahi_api::channels::send_<msg_name>` /
  `recv_<msg_name>` wrappers generated from manifest `[[channel]]` entries by
  sntm-validate. Wrong struct = compile error at task crate level.
- `BOOT_CHANNELS` table emits producer/consumer pair per channel id (drift
  guard via CI regen).

### Limits

- CRC is not enforced by the kernel on every send/receive; it is helper-driven.
- The model is SPSC, not MPMC.

---

## 8. Policy Engine And Degrade

### Implemented

- Pure `decide_action(event, restart_count, dal)` function.
- Failure modes: Restart, Isolate, Degrade, Failover, Alert, Shutdown.
- `PolicyEvent` variants: `BudgetExhausted=0`, `StackOverflow=1`, `TaskFault=2`,
  `CapViolation=3`, `IopmpViolation=4`, `PmpIntegrityFail=5`, `WatchdogTimeout=6`,
  `DeadlineMiss=7`, `MultiModuleCrash=8`.
- In v1.x, Failover is not a real hot-standby switch; it falls back to Degrade
  while preserving forensic distinction.
- Policy lockstep calls the decision function twice and shuts down on mismatch.
- `black_box` fences reduce compiler CSE risk in the lockstep path.
- Restart counters use saturating behavior.
- Degrade suspends lower-criticality work while keeping high-criticality tasks
  prioritized.
- Recovery has a cooldown to reduce oscillation.
- **SAFE-4 Kani-proved**: `stack_overflow_policy_event_mapping` —
  `PolicyEvent::StackOverflow` → decide_action returns only Restart or Isolate
  across all DAL×restart combinations (K7 no dead arms; DAL-D 3-restart
  policy preserved).

### Limits

- A real standby-task failover runtime is not implemented.

---

## 9. Blackbox Flight Recorder

### Implemented

- 8 KB static circular buffer.
- 128 records, 64 bytes each.
- CRC32-protected record format.
- Monotonic tick field (u64).
- Events cover policy, PMP, watchdog, lockstep, POST warning, and similar paths.
- Write-position bounds guard prevents out-of-bounds writes.

### Limits

- There is no automatic persistent-storage flush.
- Multi-hart log aggregation is not implemented.

---

## 10. WASM Sandbox Status (historical — removed in v2.0)

**U-29 v2.0: WASM completely removed.** v1.x had a Wasmi 1.0.9 + 4MB bump
allocator + float opcode rejection + LEB128 parser + fuel metering prototype
path, gated behind `wasm-sandbox`. U-29 deletions:

- `wasmi` dep removed
- `wasm-sandbox` feature removed
- `src/sandbox/` folder removed (~700 LOC)
- `.wasm_arena` linker section removed
- ~13-15 WASM-tied Kani proofs removed (213 → 189)
- `extern crate alloc` + `#[global_allocator]` + `#[alloc_error_handler]` removed
- `ed25519-dalek` (alloc dep) → `ed25519-compact` (pure no_alloc) migration

The kernel is now pure `no_std + no_alloc`. SNTM Native Task Model v2.0 final.

---

## 11. SNTM Native Task Model (post-Phase-5)

### Implemented

- `sipahi_api` crate (no_std + no_alloc, ed25519-compact, blake3 path-dep):
  - `Error` enum (8 variants: `InvalidSyscall=0`..`Internal=7`) +
    `from_kernel` mapping (SAFE-3 CR-1 ABI alignment).
  - 64-byte `ipc::Message` + typed `send_<msg>`/`recv_<msg>` codegen wrappers.
  - All 6 syscall wrappers + SAFE-2 `local_cap_invoke`.
  - Per-task feature flag `task_<name>` gates channel access (manifest
    [[channel]] consumer/producer match).
- `SYS_EXIT = 5` kernel syscall handler.
- `tasks/task_hello` (id=2) standalone native task — yield + IPC + exit loop.
- `tasks/task_world` (id=3, SAFE-2) IPC consumer.
- `sipahi.toml` manifest fully populated: kernel + platform + 2 tasks + 4
  regions per task (NAPOT) + resources + 1 channel + local_caps.
- `sntm` + `sntm-safe` feature flags; default-off; SAFE-1..4 work is
  build-time gate driven, not runtime feature flag.
- Kernel crate does not depend on `sipahi_api`; task crates use path dep.
  This boundary is intentional.

### Cross-task isolation

- **Statik kanıt** (Kani SNTM-R12): manifest-driven per-task PMP profiles +
  symmetric region rejection (task_hello rejects task_world region, and vice
  versa) — both branches Kani-verified.
- **Runtime kanıtı** (`scripts/check_cross_isolation.sh`): 4-gate — trap
  isolation enforced, no BROKEN marker, post-trap tick continuity, no
  FATAL/NF/POLICY (DAL-D 3-restart policy validated).

---

## 12. SNTM-SAFE phased rollout (sprint-u30..u33)

### SAFE-1 (v1.6.1) — task-lint Safe Native Profile

- `tools/task-lint/` host tool (~700 LOC, syn 2.0 AST visitor, cfg-aware).
- **11 forbidden rules** on task source:
  1. `unsafe` block
  2. `extern "C"` FFI
  3. `alloc::*` import (heap-free task discipline)
  4. inline `asm!`
  5. recursion (call graph cycle detect)
  6. `dyn` trait + function pointer
  7. `panic_unwind`
  8. `#[link_section = ".init_array"]`
  9. `f32`/`f64` floating-point
  10. `core::sync::atomic`
  11. MMIO raw pointer cast (volatile arithmetic)
- **DAL-aware `trust_tier` enforcement**:
  - `safe` (default) → 11 rules HARD-FAIL
  - `trusted_unsafe` (manifest opt-in):
    - DAL-A/B → HARD-FAIL (doctrine)
    - DAL-C/D → `waiver_reason` required + `demo_feature_waivers` list of
      Cargo features that gate unsafe code (must be default-OFF; CI drift guard)
- Safe gate [2/10] aktif; CI `task-lint` job + production binary unsafe leak
  guard (objdump-based, cfg compile-out check).
- 18 integration tests.

### SAFE-2 (v1.7.0) — Static cap table + typed IPC

- `src/kernel/capability/cap_action.rs` — `CapAction` 6-variant enum +
  `from_u8` (None, Read, Write, ReadWrite, Execute, All).
- `src/kernel/capability/cap_generated.rs` — CODEGEN: `LOCAL_CAP_TABLE` (per
  task × resource action grant) + `BOOT_CHANNELS` (id, producer, consumer
  tuples).
- `src/kernel/capability/local_cap.rs` — `local_cap_invoke` syscall wrapper.
- `sipahi_api/src/channels.rs` — CODEGEN: per-channel typed `send_<msg>` /
  `recv_<msg>` wrapper.
- Manifest schema extensions: `[[resource]]`, `[[channel]]`, `[[task.local_cap]]`.
- TLA+ `ChannelOwnershipInvariant` + `StrongChannelOwnership` (sealed
  atomicity birleşik).
- Safe gate [3/10] cargo +nightly build (typed IPC compile guard) +
  [7/10] cap_generated drift + [8/10] channels drift.
- Kani +7 harness (typed_ipc cross-crate K8, CapAction roundtrip, BOOT_CHANNELS
  well-formed, sys_cap_invoke reserved bits).

### SAFE-3 (v1.8.0) — Binary verifier + TaskCertificate + signed image

- `tools/riscv-bin-verify/` — RV64IMAC instruction whitelist (~1700 LOC):
  - ALLOW: base RV64I + M + A + RVC (c.ld/c.sd/c.ldsp/c.sdsp = integer)
  - ALLOW: `ecall` (kernel syscall — CR-10)
  - REJECT: F/D floating-point (c.fld/c.fsd reject; integer RVC OK)
  - REJECT: CSR instructions, `mret`, `ebreak`
  - Symbol filter: STT_FILE / STT_SECTION / SHN_ABS / SHN_UNDEF SKIP (CR-11)
  - Region check: task code outside kernel range
  - 18 unit + 21 integration test (synthetic ELF builder)
- `tools/sntm-cert-gen/` — TaskCertificate `repr(C)` 424B ABI v1:
  - BLAKE3 hash chain: manifest, toolchain (`rust-toolchain.toml`),
    `source_commit` (git HEAD or zero sentinel), text/rodata/data
  - ed25519-compact RFC 8032 sign + verify
  - 14 integration tests (RFC 8032 + tamper + SAFE-4 cert flow).
- `tools/sntm-image/` — Signed image:
  - `SIPI1` 5-byte magic + 64-byte header (kernel/body/tail_sig offsets)
  - Kernel ELF + task certs + task `.bin` payloads
  - 64-byte tail ed25519 signature
  - 11 integration tests (roundtrip + tamper magic/body/sig).
- `Tla+/SipahiSecureBoot.tla` — 6-state image verify spec, invariants:
  `StartedImpliesValid`, `NoFalseAccept`, `AtomicVerify`, `SigValidImpliesHeader`.
- `keys/dev-image.{priv,pub}` ed25519 keypair (`gen_dev_key.sh` bootstrap;
  `.priv` gitignored).
- Safe gate [4/10] riscv-bin-verify + [9/10] cert sign+verify + [10/10] image
  assemble+verify.
- Kani +6 harness (cert_field_layout_pin, image_magic_invariant,
  image_header_size_invariant, verify_cert_signature_bounded,
  syscall_error_abi_alignment).

### SAFE-4 (v1.9.0) — Stack analyzer + 10/10 gate (Plan B)

- **Plan B decision**: `cargo-call-stack 0.1.16` incompatible with current
  nightly (`error: unsupported rust toolchain`); LLVM `-Z emit-stack-sizes`
  ELF section direct parse.
- `tools/sntm-stack/` host tool (~800 LOC):
  - `object 0.36.5` ELF parser
  - ULEB128 `.stack_sizes` section decode (8-byte LE addr + ULEB128 size per fn)
  - **AUIPC+JALR pair detect** (linker-resolved direct call/tail)
  - **Indirect REJECT**: bare JALR (rd!=x0), c.jalr, c.jr (rs1!=x1)
  - **JAL / c.j direct edge**
  - **DFS recursion cycle detect**
  - **Sum-of-frames over-approximation** (caveat in report)
  - 23 unit + 9 integration tests
  - Golden fixture `task_hello.stack.golden.txt` committed
- `src/common/config.rs`:
  - `STACK_ANALYSIS_MARGIN_BYTES = 256` (CR-5 doctrine)
  - `STACK_ANALYSIS_UNKNOWN_SENTINEL = 0xFFFF_FFFF` (CR-4)
- `tools/sntm-validate/src/stackreport.rs` + `validate.rs::check_stack_bounds`:
  - `stack_size ≥ observed_max + margin` formula (exact equality REJECT)
  - UNKNOWN sentinel always REJECT
  - 12 unit + 5 integration tests
- `tools/sntm-cert-gen/src/stackreport.rs` (FIX-G shared crate deferred):
  - `--call-stack-report` parses sntm-stack output
  - Absent or FAIL → `max_stack_bytes = UNKNOWN_SENTINEL`
  - **Manifest `stack_size` NEVER fallback** (CR-4: allocation vs observation)
  - 4 new integration tests.
- `Tla+/SipahiSNTM.tla` `StackRegionBound` invariant (state count 138
  baseline preserved).
- `src/verify.rs` +3 Kani: `stack_analysis_margin_pin` (K2 const literal),
  `stack_bounds_invariant` (K3+K5 symbolic formula + exact equality reject),
  `stack_overflow_policy_event_mapping` (K7 PolicyEvent::StackOverflow=1).
- **Safe gate [5/10] active — 10/10 active, DEFER yok, SAFE faz kapanışı.**
- `scripts/stack_analysis.sh` runner with `env -u RUSTFLAGS` (SAFE-3 lesson).
- CI `sntm-stack` job: build + integration + stack bound validation.
- `docs/safe/cert_abi_v2_migration.md` — ABI v2 plan (doc only, post-CFI).
- `coverage.toml` `SNTM-SAFE-R6` requirement with `required_tool_tests`
  schema (8 tool tests + 3 Kani proofs + 2 scripts).

### Carry-forward (post-SAFE faz)

- **CFI hardware faz** (Zicfilp landing pad + Zicfiss shadow stack — CVA6-CFI
  ready)
- **Stack scribble debug-boot redesign** (low-watermark region-bottom scan —
  SAFE-4 CR-6 doctrine; "stack top -8 sentinel" pattern is incorrect for
  RISC-V downward stack growth)
- **HSM/OTP production key sprint** (`keys/dev-image.priv` → HSM-provisioned)
- **TaskCertificate ABI v2** (CFI landing pad list, post-quantum sig migration)
- **Shared `sntm-manifest` lib crate** (SAFE-2 FIX-G — sntm-validate +
  riscv-bin-verify + sntm-cert-gen + sntm-stack manifest struct unification)
- **`sipahi_api` task-lint scope** (SAFE-2 CR-4 — `[[support_crate]]` design
  for shared task-API helper crates)

---

## 13. Verification Infrastructure

### Implemented

- **204 Kani harnesses** (kernel-side; host tool fixtures are in cargo test).
- **9 TLA+ specifications** (Scheduler, Capability, Policy, Watchdog,
  DegradeRecover, BudgetFairness, IPC, **SNTM** [138 states, post-SAFE-4 with
  StackRegionBound], **SecureBoot** [6 states, SAFE-3]).
- `make check` — Clippy `-D warnings`.
- `make run-self-test` — POST + integration/self-test path.
- `scripts/sipahi_sprint_gate.sh` — legacy kernel umbrella.
- `scripts/sntm_sprint_gate.sh` — SNTM v1.x umbrella.
- **`scripts/sntm_safe_gate.sh` — SAFE umbrella (10/10 active, DEFER yok).**
- `scripts/stack_analysis.sh` — SAFE-4 sntm-stack runner.
- `scripts/check_coverage.sh` — coverage.toml ↔ source traceability
  (14 feature + 20 requirement, with `required_tool_tests` SAFE-4 schema).
- `scripts/check_proof_quality.sh` — Kani harness adequacy heuristic.
- `scripts/feature_matrix.sh` — 10 feature combination build.
- `scripts/check_cross_isolation.sh` — SNTM-R12 4-gate runtime.
- GitHub Actions: 15 jobs (build, qemu, audit, Kani full + PR subset,
  task-lint, sntm-validate, sntm-pack, sntm-stack, mutation, ct-eq, ...).

### Verification limits

- Kani is bounded model checking; does not prove all hardware or concurrency
  behavior.
- TLA+ models check abstract protocols; there is no full Rust refinement
  proof.
- Coverage checks are name-based mechanical guards; semantic test/proof
  quality still requires review (`// VERIFIES: ID` + `// CALLS: ...` +
  `// FAILS-IF: ...` triple required for non-grandfathered items).
- QEMU does not model real cache, bus contention, PMP timing, or FPGA
  platform interference.
- SAFE-4 stack analyzer uses sum-of-frames over-approximation; call-graph
  transitive analysis is post-SAFE roadmap.

---

## 14. Explicit Non-Features / Roadmap Items

The following items appear in design documents or planning notes but are not
current runtime guarantees:

- AMCI multi-hart runtime
- SPMP / WorldGuard / production IOPMP enforcement
- CLIC integration
- Scratchpad/TCM optimization
- CHERIoT research branch
- Hardware CFI (Zicfilp + Zicfiss)
- TaskCertificate ABI v2 (post-CFI)
- HSM-provisioned production key chain (post-SAFE)
- Smepmp adoption (`mseccfg.MML=1`)
- Real FPGA WCET database
- TLAPM-based Rust refinement proof
- Call-graph-aware transitive stack analysis (replace SAFE-4 sum-of-frames)
- Runtime stack-overflow watermark (SAFE-4 CR-6 redesign)

The distinction matters: Sipahi has an ambitious design direction, but not
every design idea is an implemented kernel feature today.
