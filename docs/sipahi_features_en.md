# Sipahi — Technical Feature Inventory

This document summarizes the features that exist in the current Sipahi working
tree. It deliberately separates implemented behavior, partial scaffolding, and
roadmap items.

**Status:** v1.1.1 + U-23 SNTM Phase 1 working tree  
**Target:** RISC-V RV64IMAC, QEMU `virt`, single hart  
**Language:** Rust `no_std`, bare metal  
**Verification assets:** 198 Kani harnesses, 7 TLA+ models, self-test and sprint gates

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
- `src/kernel/capability`: token, broker, cache
- `src/ipc`: SPSC IPC channels and blackbox recorder
- `src/kernel/policy`: failure policy engine
- `src/sandbox`: feature-gated WASM prototype path
- `sipahi_api`: SNTM task-side API crate
- `tasks/task_hello`: standalone native task scaffold

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
- Task stacks live in the `.task_stacks` area.
- A per-task NAPOT stack entry is programmed on context switch.
- `sfence.vma zero, zero` is issued after per-task PMP writes.
- PMP shadow integrity is checked from the scheduler tick path.
- `is_valid_user_ptr(caller_task_id, ptr, size)` is task-specific:
  it only accepts addresses inside the caller task's own stack range.
- Dead, isolated, or uninitialized tasks have no valid user pointer range.

### SNTM status

- The SNTM manifest scaffold (`sipahi.toml`) exists.
- Generated multi-region PMP profile tables do not exist yet.
- Runtime multi-region PMP reload is a future U-25+ target.

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
- Watchdog counters increase only for Running tasks; Ready tasks are not
  punished for not receiving CPU time.
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
| 0 | `cap_invoke` | capability check |
| 1 | `ipc_send` | non-blocking IPC send |
| 2 | `ipc_recv` | non-blocking IPC receive |
| 3 | `yield` | yield to scheduler |
| 4 | `task_info` | query task state/priority/DAL |
| 5 | `exit` | voluntary task termination |

`SYSCALL_COUNT = 6`.

### Implemented guards

- O(1) function-pointer dispatch table.
- Invalid syscall ID returns `E_INVALID_SYSCALL`.
- Kernel-looking return values are sanitized to `E_INTERNAL`.
- IPC pointers are validated with task-specific pointer checks and alignment
  checks before use.
- `sys_cap_invoke` rejects truncation-prone arguments.
- `rdcycle` records last/max syscall cycles.
- `check_wcet_limits()` covers all six syscalls.
- `print_wcet_stats()` uses a compile-time `SYSCALL_COUNT`-sized name table.

---

## 6. Capability System

### Implemented

- 32-byte `Token` structure.
- BLAKE3 keyed MAC validation path.
- `ct_eq_16` constant-time MAC comparison.
- 4-slot validation cache.
- Per-task nonce replay guard.
- Token expiry check.
- Token owner enforcement: a valid MAC is not enough if the caller is not the
  token owner.
- Cache invalidation by token/owner and capability revocation on task isolation.
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

- 8 static SPSC channels.
- 16 slots per channel.
- 64-byte messages.
- `AtomicU16` head/tail with Release/Acquire ordering.
- O(1), non-blocking `send` and `recv`.
- Channel producer/consumer ownership is assigned at boot and then sealed.
- Unassigned channels are default-deny.
- `can_send` / `can_recv` enforce ownership.
- CRC32 helper methods exist (`set_crc`, `verify_crc`).
- IPC send rate limiting is applied per tick.

### Limits

- CRC is not enforced by the kernel on every send/receive; it is helper-driven.
- The model is SPSC, not MPMC.
- Typed IPC generation is planned for later SNTM phases.

---

## 8. Policy Engine And Degrade

### Implemented

- Pure `decide_action(event, restart_count, dal)` function.
- Failure modes: Restart, Isolate, Degrade, Failover, Alert, Shutdown.
- In v1.x, Failover is not a real hot-standby switch; it falls back to Degrade
  while preserving forensic distinction.
- Policy lockstep calls the decision function twice and shuts down on mismatch.
- `black_box` fences reduce compiler CSE risk in the lockstep path.
- Restart counters use saturating behavior.
- Degrade suspends lower-criticality work while keeping high-criticality tasks
  prioritized.
- Recovery has a cooldown to reduce oscillation.

### Limits

- A real standby-task failover runtime is not implemented.
- System-level behavior still depends on the application architecture using the
  policy engine.

---

## 9. Blackbox Flight Recorder

### Implemented

- 8 KB static circular buffer.
- 128 records, 64 bytes each.
- CRC32-protected record format.
- Monotonic tick field.
- Events cover policy, PMP, watchdog, lockstep, POST warning, and similar paths.
- Write-position bounds guard prevents out-of-bounds writes even if the position
  is corrupted.

### Limits

- There is no automatic persistent-storage flush.
- Multi-hart log aggregation is not implemented.

---

## 10. WASM Sandbox Status

WASM is no longer the main forward path. It remains as a prototype/test path.

### Implemented

- Gated behind the `wasm-sandbox` feature.
- Enabled by `self-test` for test builds.
- Uses Wasmi 1.0.9.
- 4 MB arena/bump allocator.
- WASM magic/version/code-section checks.
- Float-opcode rejection heuristic.
- Additional checks for `0xFC` saturating truncation and `br_table` skipping.

### Limits

- It is not a full WASM grammar parser.
- It is off in the production default build.
- It is expected to shrink or disappear as SNTM becomes the primary path.

---

## 11. SNTM Phase 1

### Implemented

- `sipahi_api` crate:
  - `Error` enum and kernel return mapping.
  - 64-byte `ipc::Message`.
  - `cap_invoke`, `ipc_send`, `ipc_recv`, `yield_cpu`, `task_info`, and `exit`
    syscall wrappers.
- `SYS_EXIT = 5` kernel syscall handler.
- `tasks/task_hello` standalone native task scaffold:
  - `_start`
  - yield loop
  - panic -> `syscall::exit(255)`
  - task-scoped linker configuration
- `sipahi.toml` manifest scaffold.
- `sntm` and `sntm-safe` feature flags are default-off.
- The kernel crate does not depend on `sipahi_api`; task crates depend on it
  directly. This boundary is intentional.

### Not implemented yet

- `sntm-validate` host tool.
- Generated PMP profile tables.
- Native task image packer/loader.
- Runtime native task boot.
- Multi-region PMP reload from manifest.
- Typed IPC generator.
- Binary verifier / task certificate flow.
- SNTM runtime behavior tests with booted native tasks.

---

## 12. Verification Infrastructure

### Implemented

- 198 Kani harnesses.
- 7 TLA+ models.
- `make check` Clippy gate.
- `make run-self-test` POST + integration/self-test path.
- `scripts/sipahi_sprint_gate.sh`.
- `scripts/sntm_sprint_gate.sh`.
- `scripts/check_coverage.sh`.
- `scripts/check_proof_quality.sh`.
- `scripts/feature_matrix.sh` with 10 feature combinations.
- GitHub Actions for build, QEMU smoke/self-test, audit/deny, Kani, binary
  guards, and constant-time helper inspection.

### Verification limits

- Kani is bounded model checking; it does not prove all hardware or concurrency
  behavior.
- TLA+ models check abstract protocols; there is no full Rust refinement proof.
- Coverage checks are name-based mechanical guards; semantic test/proof quality
  still requires review.
- QEMU does not model real cache, bus contention, PMP timing, or FPGA platform
  interference.

---

## 13. Explicit Non-Features / Roadmap Items

The following items appear in design documents or planning notes but are not
current runtime guarantees:

- AMCI multi-hart runtime.
- SPMP / WorldGuard / production IOPMP enforcement.
- CLIC integration.
- Scratchpad/TCM optimization.
- CHERI research branch.
- Hardware CFI.
- SNTM-SAFE binary verifier.
- Task certificate flow.
- Real FPGA WCET database.

The distinction matters: Sipahi has an ambitious design direction, but not every
design idea is an implemented kernel feature today.
