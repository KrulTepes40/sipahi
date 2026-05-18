# Sipahi Microkernel — Architecture

> Status: kernel crate `v1.1.1` + SNTM Native Task Model v2.0 +
> **SNTM-SAFE faz tamamlandı (v1.9.0, sprint-u33)**.
> Hedef: RV64IMAC, QEMU `virt`, single-hart; pure `no_std + no_alloc`.

## Layer Structure

```
Layer 0: arch/     — RISC-V hardware (PMP, CSR, CLINT, UART, trap, context switch)
Layer 1: hal/      — Abstraction (DeviceAccess, IOPMP, key store, secure boot)
Layer 2: kernel/   — Core (scheduler, syscall, capability, policy, memory,
                            loader, pmp, cap_generated/cap_action/local_cap)
Layer 3: tasks/    — SNTM Native Task Model (task_hello, task_world; PMP-isolated
                            U-mode tasks via sipahi_api)
Cross:   common/   — Shared (config, types, error, crypto, fmt, sync, diagnostic;
                            SAFE-4 STACK_ANALYSIS_MARGIN_BYTES / SENTINEL const)
Cross:   ipc/      — Communication (SPSC lock-free ring buffer, blackbox recorder)
Cross:   sipahi_api/ — Task-side syscall ABI + typed IPC channels (codegen target)

Host:    tools/    — task-lint, sntm-validate, sntm-pack, riscv-bin-verify,
                            sntm-cert-gen, sntm-image, sntm-stack
                            (each is a sub-workspace; `env -u RUSTFLAGS` doctrine)
```

**Tarihsel:** Layer 3 v1.x'te `sandbox/` (WASM via Wasmi + bump allocator +
compute services) idi. U-29 v2.0'da WASM tamamen kaldırıldı; SNTM native task
model ile değişti. Kernel artık `extern crate alloc` taşımaz.

## Dependency Flow

```
arch <- hal <- kernel <- tasks (via sipahi_api)
               ^         ^
            common     common
               ^
             ipc
```

**Exception:** `arch/trap.rs` calls `kernel::syscall::dispatch()` and
`kernel::scheduler::schedule()`. This is an intentional upward call from
the hardware trap entry point — trap dispatch requires kernel services.
No other circular dependencies exist.

**Host tool side:** `tools/*/` her biri kendi sub-workspace'i (`[workspace]`
Cargo.toml'da). Kernel `cargo build` / `cargo kani` host tool'ları görmez;
SAFE gate (`scripts/sntm_safe_gate.sh`) her tool için ayrı invoke eder
(`env -u RUSTFLAGS`).

## Privilege Model

- Kernel: M-mode (Machine mode) — full CSR/PMP/MMIO access
- Tasks: U-mode (User mode) — PMP-restricted, no CSR access
- Transition: `mret` instruction via assembly `task_trampoline`
- No S-mode (no MMU, no page tables, no TLB flush non-determinism)

## Security Layers (post-SAFE-4)

1. **Rust type system** (compile-time memory safety; SAFE-1 `task-lint`
   AST-level 11 yasak kural; `unsafe`, `alloc::*`, `asm!`, recursion,
   `dyn`/fn-pointer, `panic_unwind`, `f32/f64`, atomic, MMIO cast yasak)
2. **PMP hardware protection** (L-bit locked kernel + UART entries +
   multi-region SNTM profiles via manifest)
3. **Capability tokens** (BLAKE3 MAC, per-task nonce, cache TTL, replay guard,
   expiry)
4. **SAFE-2 static cap table** — `LOCAL_CAP_TABLE[task][resource] → CapAction`
   codegen'den drift guard ile gelir; manifest dışı eylem syscall'da
   reject
5. **SAFE-2 typed IPC** — `sipahi_api::channels::send_<msg>` / `recv_<msg>`
   wrapper'lar codegen; manifest [[channel]] dışı mesaj compile error
6. **SNTM native task isolation** (manifest-driven PMP profile + load-time
   bounded copy + cross-task statik kanıt + runtime ihlal observation)
7. **SAFE-3 riscv-bin-verify** (RV64IMAC opcode whitelist; F/D/CSR/mret/ebreak
   REJECT; ecall ALLOW; symbol filter STT_FILE/SECTION/SHN_ABS/UNDEF SKIP)
8. **SAFE-3 TaskCertificate + signed image** (BLAKE3 chain: manifest,
   toolchain, source_commit, text/rodata/data + ed25519-compact RFC 8032
   sign/verify; SIPI1 magic + 64B header + tail sig)
9. **SAFE-4 stack analyzer (sntm-stack Plan B)** — `-Z emit-stack-sizes` ELF
   section direkt parse; AUIPC+JALR pair detect (linker-resolved direct call) +
   bare JALR rd!=x0/c.jalr indirect REJECT + DFS recursion cycle REJECT;
   `stack_size ≥ observed_max + STACK_ANALYSIS_MARGIN_BYTES` (256 default)
10. **CRC32 IPC integrity** + rate limiting + pointer validation + sealed
    channel atomicity
11. **Policy engine** (6-mode failure escalation with lockstep verification;
    `PolicyEvent::StackOverflow` → Restart/Isolate only — K7 SAFE-4 Kani
    kanıtlı)
12. **Blackbox flight recorder** (power-loss tolerant, u64 monotonic tick)
13. **Hardening** (PMP shadow, MPP verification, kernel pointer sanitization,
    windowed watchdog, production binary unsafe leak guard)

(Tarihsel: v1.x'te bir katman WASM sandbox vardı; U-29 v2.0'da kaldırıldı.)

## Safety Doctrine

- Zero panic in production code
- Zero heap allocation in kernel (U-29 v2.0: `extern crate alloc` +
  `global_allocator` tamamen kaldırıldı; pure `no_std + no_alloc`)
- Zero floating-point (Q32.32 fixed-point)
- Zero recursion (bounded stack; SAFE-1 task-lint Rule 5 + SAFE-4 sntm-stack
  DFS cycle detect)
- Zero `static mut` (all via `SingleHartCell<T>`)
- ~131 `unsafe` blocks, majority documented with `// SAFETY:` (CI
  informational check reports undocumented blocks via `continue-on-error:
  true` — advisory, not enforced)
- DAL-aware policy: DAL-A/B `trusted_unsafe` HARD-FAIL (SAFE-1 doctrine);
  DAL-C/D allow with explicit `waiver_reason` + `demo_feature_waivers`
  (cfg-gated, default-OFF, production guard)

## Formal Verification (post-SAFE-4)

- **Kani**: **204 bounded model checking harnesses** (SAFE-1..4 hepsi PASS;
  v1.x'te 213 idi, U-29'da -24 WASM proof, sonrasında SAFE faz +15)
- **TLA+**: **9 specifications**, all verified with TLC v2.19
  (SipahiScheduler, Capability, Policy, Watchdog, DegradeRecover,
  BudgetFairness, IPC, SNTM, SecureBoot; total ~36,000 distinct states)
- Compile-time: 10+ `const _: () = assert!(...)` (layout, size, config
  invariants; SAFE-3 cert ABI size; SAFE-4 stack margin range)
- Clippy: zero warnings (`-D warnings`)
- Overflow checks: enabled in release (`overflow-checks = true`)
- Supply chain: `cargo audit` (RustSec CVE scan) + `cargo deny`
  (license/bans/sources policy)
- **SAFE gate 10/10 aktif** (DEFER yok; SAFE faz kapanışı sprint-u33)
- CI gates (GitHub Actions): build, QEMU boot test, audit, Kani full (master
  push), Kani PR subset, task-lint, sntm-validate, sntm-pack, sntm-stack
  (SAFE-4 Plan B build + integration + stack bound check)

### Kani harness sayım dökümü

| Sprint | Kani | Açıklama |
|--------|------|----------|
| U-22 baseline | 200 | Pre-SNTM kernel proof'ları |
| U-23 SNTM Phase 1 | +1 | syscall_id_set_complete (R1/R2) |
| U-24 SNTM Phase 2 | +2 | region_overlap_symmetric, napot_alignment (R3/R5) |
| U-25 SNTM Phase 3 | +5 | SNTM-R6/R7/R8 multi-region PMP |
| U-26 SNTM Phase 4 | +4 | SNTM-R9 native task loader |
| U-27 SNTM Phase 5 | +4 | SNTM-R10/R11/R12 sealed channels, cross-task |
| U-29 v2.0 | -24 | WASM proof'lar sandbox/ ile kaldırıldı |
| SAFE-1 (U-30) | +0 | task-lint static analiz cargo test'te (Kani yok) |
| SAFE-2 (U-31) | +7 | typed IPC cross-crate K8, BOOT_CHANNELS, CapAction |
| SAFE-3 (U-32) | +6 | cert ABI pin, image magic, verify bounded, syscall ABI |
| SAFE-4 (U-33) | +3 | stack_analysis_margin_pin, stack_bounds_invariant, stack_overflow_policy_event_mapping |
| **Toplam** | **204** | SAFE-1..4 hepsi PASS |

## Formal Verification Scope & Limitations

What "verified" means for Sipahi — and what it does NOT mean.

### Kani (Bounded Model Checking)

204 harnesses cover **Rust function-level invariants**: panic freedom, integer
overflow, array bounds, pure-function purity, policy decision tables, SNTM
multi-region PMP, sealed channel atomicity, cross-task isolation statik,
SAFE-2 typed IPC cross-crate K8, SAFE-3 cert ABI pin + image magic + verify
bounded, SAFE-4 stack bound formula + margin pin + policy mapping.

Kani **does NOT cover**:

- **Assembly** (`trap.S`, `context.S`, inline `asm!`) — Kani works at Rust IR
  level
- **Hardware register semantics** (CSR read/write, PMP, CLINT mtime/mtimecmp) —
  stubbed
- **Atomics / memory ordering** — single-hart assumption (`MIE=0` in trap
  context)
- **Interrupt timing / concurrency** — sequential execution model only
- **Crypto correctness** (BLAKE3, Ed25519-compact) — only API safety;
  correctness via upstream RFC 8032 test vectors (SAFE-3 cargo test fixture)
- **Host tool internals** (SAFE-2 CR-8 + SAFE-3 CR-8 + SAFE-4 CR-7 doctrine):
  `tools/*` her biri sub-workspace; kernel `cargo kani` host tools'u koşmaz.
  Tool tarafı **cargo test fixture'ları** ile doğrulanır (sntm-stack 32,
  sntm-validate 52, sntm-cert-gen 17, riscv-bin-verify 39, sntm-image 11,
  task-lint 18)

Kani "proved" = "this Rust function over its bounded input space does not panic
or trigger UB." It does **not** mean "the system as a whole behaves correctly."

### TLA+ (Temporal Logic Model Checking)

9 specs verified with TLC: scheduler, capability cache, policy escalation,
watchdog, IPC, degrade/recovery, budget fairness, **SNTM (138 states with
SAFE-2 ChannelOwnership + SAFE-3 StrongChannelOwnership + SAFE-4
StackRegionBound)**, **SecureBoot (6 states, SAFE-3)**.

TLA+ specs are **abstractions** of Rust code. **No formal refinement mapping**
(TLAPM) has been performed — i.e., we have not proved that the Rust code is a
refinement of the TLA+ spec. Specs and code are kept aligned by review and
shared constants, not by mechanically checked correspondence.

TLAPM-based refinement is a v2.0 roadmap item.

### WCET (Worst-Case Execution Time)

All `WCET_*` constants in `config.rs` are **estimated** based on instruction
counting under QEMU TCG. QEMU does **not** model cache, branch prediction, or
memory hierarchy — its `rdcycle` returns instruction count, not real cycles.

**No cycle-accurate WCET measurement has been performed on real hardware.**
Production WCET validation (FPGA + cycle counter, AbsInt aiT or similar) is a
v2.0 roadmap item. Treat WCET numbers as ordering constraints (which is
faster than which) rather than absolute cycle budgets.

### Stack analysis (SAFE-4 Plan B)

`tools/sntm-stack/` parses LLVM `.stack_sizes` ELF section directly
(`-Z emit-stack-sizes` flag). It:

- detects AUIPC+JALR pairs → resolved direct calls (linker-relaxed)
- rejects bare JALR (rd != x0), c.jalr, c.jr (non-x1) → indirect call FAIL
- DFS over call graph → cycle (recursion) FAIL
- sum-of-frames (over-approximation) → max_stack_bytes

**Over-approximation caveat:** sum-of-frames worst-case = "all of program's
functions are simultaneously on stack." Call-graph-aware transitive analysis
(more precise) is post-SAFE roadmap. task_hello observed 128B + 256B margin =
384B << 8KB stack region — comfortable headroom.

**Why Plan B:** `cargo-call-stack 0.1.16` is the latest upstream version but
hard-codes nightly-2023-11-13 in its rustc wrapper; current project nightly
(2026-03-01) triggers `error: unsupported rust toolchain` and rustc intercept
failure. Plan B sidesteps the dependency entirely.

### Bottom line

Sipahi applies **safety-critical design principles** (DO-178C DAL-A, ISO 26262
ASIL-D patterns) and uses **multiple verification layers** to catch entire
classes of bugs early. It is **not certified** and would require independent
DER review, requirements traceability matrix, FPGA timing analysis, and HSM
production key provisioning before any certification claim could be made.

## Scheduler Features

- Fixed-priority preemptive (0=highest, 15=lowest)
- Per-task CPU budget with `saturating_sub` (DAL-A 40%, DAL-B 30%, DAL-C 20%,
  DAL-D 10%)
- Windowed watchdog (upper + lower bound, ISO 26262 / DO-178C)
- Graceful degradation with automatic recovery (DAL-C/D budget halving +
  restore)
- POST (Power-On Self Test) at boot: CRC32, PMP, policy engine
- SAFE-3 secure-boot path: `SipahiSecureBoot` TLA+ spec 9/9
  (`StartedImpliesValid`, `NoFalseAccept`, `AtomicVerify`)

## Memory Map

See `sipahi.ld` header comment for full layout.

Kernel reserved: `0x80000000..0x80600000` (6 MiB; manifest `[kernel]
reserved_size = 0x600000` default; SAFE-3 CR-2 dynamic cross-check).

Native task base: `0x80600000` onwards; her task'in stack/text/rodata/data
NAPOT region'ları `sipahi.toml` `[[task.region]]` ile manifest-driven.

## Image Format (SAFE-3 SIPI1)

```
+-----------+--------------+--------------+--------------+----------+----------+
| HEADER 64 | KERNEL ELF   | TASK CERT(N) | TASK BIN(N)  | ...      | TAIL SIG |
| SIPI1+off | (sealed)     | 424 byte    | text/rodata/  |          | 64 byte  |
|           |              | ABI v1      | data sections |          | ed25519  |
+-----------+--------------+--------------+--------------+----------+----------+
```

- `SIPI1` magic 5 byte
- Header offsets: kernel_offset, body_offset, tail_sig_offset (4 byte LE each)
- Tail ed25519-compact RFC 8032 wire format (64 byte raw signature)
- TaskCertificate per task (424 byte ABI v1; `abi_version = 1` cross-crate K8
  pin; SAFE-4 CR-4: `max_stack_bytes` parsed sntm-stack value or
  `UNKNOWN_SENTINEL = 0xFFFFFFFF`)

## Known Limitations — v1.x → v2.0 (post-SAFE-4)

### Vanilla PMP (no Smepmp)

RISC-V vanilla PMP'de `L`-bit M-mode'un PMP entry'sini *değiştirmesini* kilitler;
ancak U-mode izinleri R/W/X bit'leriyle belirlenir. Entry 5 (kernel data,
`R+W+L`) U-mode task'larının kernel `.data` ve `.bss` bölgelerine read/write
erişimini engellemez — yalnızca W^X garantisi (bu bölgeler X=0) korunur.
Sonuç: `MAC_KEY`, `PMP_SHADOW`, `LAST_NONCE` gibi güvenlik-kritik veriler
U-mode'dan okunabilir (write da mümkündür ama Rust bu yolla erişim üretmez).

**Etki:**

- Capability MAC key okunup token forge edilebilir
- PMP shadow okunup integrity check bypass tasarlanabilir
- Kernel kodu (Entry 0-1, RX+L) yazılamaz — W^X korunur, code injection yok

**Plan (post-SAFE):** Smepmp extension (RISC-V Trap and Memory Protection v1.0,
`mseccfg.MML=1`) ya da `.secure_data` PMP carve-out (kernel-only entry)
implementasyonu.

### Mevcut izolasyon kuvvetleri (vanilla PMP ile bile geçerli)

- W^X: kernel `.text` U-mode'dan write/execute ile yazılamaz
- Per-task stack izolasyonu: NAPOT Entry + manifest-driven; cross-task PMP
  isolation runtime gate (4-gate verify) + statik Kani SNTM-R12
- Trap entry mscratch swap: cross-task corruption engellendi (U-9)
- Capability owner check (U-16): impersonation engellendi
- SAFE-2 static `LOCAL_CAP_TABLE`: per-task action enforcement
- SAFE-3 binary verifier: opcode-level surface attack reduction (no
  CSR/mret/ebreak/F-D in task binary)
- SAFE-4 build-time stack overflow rejection (margin enforce + indirect/recursion
  reject)

### Carry-forward (post-SAFE faz)

- **CFI hardware faz**: Zicfilp landing pad + Zicfiss shadow stack (CVA6-CFI
  olgunluğu beklenir; TaskCertificate ABI v2 plan
  `docs/safe/cert_abi_v2_migration.md`)
- **Stack scribble debug-boot redesign**: low-watermark region-bottom scan
  (SAFE-4 CR-6: RISC-V downward stack growth, "stack top -8 sentinel"
  yanlış; post-SAFE runtime/debug observation sprint)
- **HSM/OTP production key sprint**: `keys/dev-image.priv` → HSM-provisioned;
  image sig boot-time M-mode HSM API çağrısı
- **Smepmp adoption**: vanilla PMP yerine `mseccfg.MML=1` ile kernel data
  U-mode'a opak hale gelir
- **Shared `sntm-manifest` lib crate** (SAFE-2 FIX-G): sntm-validate +
  riscv-bin-verify + sntm-cert-gen + sntm-stack manifest struct duplicate
  birleştirilecek
- **Real FPGA WCET database**: AbsInt aiT veya cycle-accurate sim ile
  hardware-validated cycle budgets
