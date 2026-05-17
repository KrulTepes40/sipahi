# Changelog

All notable changes to Sipahi microkernel.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - U-27.5 SNTM-R12 Runtime Observation

### Added (cross-task PMP runtime ihlal observe + script gate)
- **`cross-isolation-demo` feature** (root Cargo.toml + tasks/task_hello/Cargo.toml,
  default OFF, `check-cfg` listesine eklendi — clippy `-D warnings` guard):
  opt-in runtime gözlem; production unaffected (compile-out), self-test
  unaffected (feature DAHIL EDİLMEZ).
- **`tasks/task_hello/src/main.rs`**: `cfg(cross-isolation-demo)` altında
  deliberate cross-region write — `*0x80705000 = 0xAA` (task_world.data,
  task_hello PMP profile dışı). Disassembly: 6 instruction (lui+addi+slli+li+sb).
- **`src/arch/trap.rs` mcause 5|7 IN-HANDLER state check** (Codex hardening):
  `handle_task_fault()` SONRASI `task_state_for_test` ile attacker (task=2)
  Isolated mı + victim (task=3) Ready/Running mı doğrulanır. Marker:
  - `[OK] Cross-task PMP isolation enforced: task=2 attempted=0x80705000 REJECTED`
    SADECE task=2 attacker + Isolated + victim Ready/Running ise
  - `[FAIL] Cross-task PMP isolation BROKEN: ...` collateral damage (victim
    Dead/Isolated) ya da unexpected attacker durumunda
  - **SILENT** restart pattern sırasında (`PolicyEvent::WasmTrap` event=2
    `restart_count < MAX_RESTART_FAULT=3` → Restart×3 → Isolate; DAL-D 3-şans
    politikası, doğru davranış)
- **`scripts/check_cross_isolation.sh`** (yeni, 4-gate QEMU log verification):
  - Gate 1: `[OK] Cross-task PMP isolation enforced: task=2` marker var
  - Gate 2: `[FAIL] ... BROKEN` marker YOK
  - Gate 3: marker sonrası ≥3 `[TICK]` (task_world unaffected + scheduler ilerliyor)
  - Gate 4: NO FATAL / [NF] / [POLICY] SHUTDOWN
  Fail durumunda non-zero exit; make target hata propagate eder.
- **`make run-cross-isolation`** Makefile target: `SIPAHI_CROSS_ISOLATION=1`
  env propagation (build_native_tasks.sh task_hello'yu feature ile build eder),
  `--features cross-isolation-demo,debug-boot` kernel build (Codex fix: `self-test`
  feature DAHIL EDİLMEZ — `tests::run_all` scheduler START öncesi çalışır,
  runtime observation için uygun değil; timing bug guard), QEMU 30s,
  `scripts/check_cross_isolation.sh /tmp/u275_xi.log` invocation.
- **`scripts/build_native_tasks.sh`**: `SIPAHI_CROSS_ISOLATION` env → task_hello
  cargo build'a `--features cross-isolation-demo` propagate eder.
- **`scripts/check_coverage.sh` schema extension**: `required_scripts` field
  (~12 satır, `os.access(X_OK)` guard ile executable kontrolü). Codex fix:
  kernel self-test ismi DEĞİL, script gate adı doğrulanır.
- **`src/kernel/scheduler/mod.rs`**: `task_state_for_test` cfg gate genişletildi
  (`any(feature = "self-test", feature = "cross-isolation-demo")`) — trap.rs
  IN-HANDLER state check için minimum erişim.

### Changed
- **`coverage.toml` SNTM-R12 entry**: `deferred = "runtime_observe"` KALDIRILDI
  (statik kanıt + runtime observation tam); `required_scripts =
  ["check_cross_isolation.sh"]` eklendi (Codex fix: kernel self-test ismi DEĞİL);
  `description` + `fault_model` genişletildi (4-gate senaryoları).
- **`coverage.toml`** yeni `[feature.cross-isolation-demo]` entry
  (`non_safety = true` — runtime test gate feature, safety mekanizması değil).
- **`src/tests/mod.rs`**: yorum sadeleştirildi (`cross-isolation-demo` referansı
  kaldırıldı). Kullanıcı dikkat 5 birebir uyum: kernel self-test'te
  `cross-isolation-demo` cfg gate YOK; runtime gate sadece script/Makefile.

### Codex 5-Madde Dikkat Noktası (hepsi karşılandı)
1. **check_cross_isolation.sh fail'de non-zero exit** ✓ — her 4 gate `exit 1`
2. **[FAIL] marker varsa kesin fail** ✓ — Gate 2 `grep -qF` + exit 1
3. **Marker SADECE task=2 + Isolated + victim Ready/Running ise [OK] bassın**
   ✓ — trap.rs `if task_id == 2 && attacker_isolated && victim_runnable` guard;
   restart pattern sırasında SILENT (sahte [FAIL] yazmaz)
4. **Production marker leak YOK** ✓ — `grep -c Cross-task /tmp/u275_prod.log = 0`
5. **src/tests/mod.rs cross-isolation-demo eklenmedi** ✓ —
   `grep -c cross-isolation-demo src/tests/mod.rs = 0`

### Verification (§18.7 + §18.4)
- Kani proof count: **213/213 PASS** (no delta — U-27 statik kanıtlar korunur)
- TLA+: **8/8 specs PASS** (SipahiSNTM 138 distinct states korunur)
- Self-test (normal `make run-self-test`): **ALL TESTS PASSED**
  (cross-isolation feature OFF, U-27'nin 14 test korunur)
- Cross-isolation gate (`make run-cross-isolation`): **4/4 PASS**
  (marker var, BROKEN yok, post-marker [TICK]=3, no FATAL)
- Production smoke (`make run` 30s): **NF/FATAL/POLICY-free**,
  `Cross-task` marker count = 0 (compile-out kanıtı)
- Coverage: **15 feature** (cross-isolation-demo eklendi, non_safety=true)
  + **14 requirement** (R12 deferred kalktı, required_scripts schema)
- Clippy: -D warnings PASS (check-cfg list `cross-isolation-demo` dahil)

### Runtime Bulgu (kernel policy davranışı)
`PolicyEvent::WasmTrap` (event=2) için `decide_action`
([policy/mod.rs:107-110](src/kernel/policy/mod.rs#L107-L110)):
`restart_count < MAX_RESTART_FAULT=3` → `Restart`, aksi → `Isolate`.
Cross-task ihlal yapan task DAL-D için 3 trap'te Restart edilir, 4. trap'te
Isolated. Trap marker bu pattern'a uyumlu — Restart sırasında SILENT (beklenen
3-şans davranışı), 4. trap'te `[OK]` (full pipeline kanıtı).

### Carry-forward (U-28+ FPGA bring-up bekliyor)
- U-28: CVA6 FPGA silicon test (hardware bekliyor)
- U-29: WASM tamamen sil + ed25519-compact migration (~1 gün)
- v1.7 SAFE-2: Typed IPC codegen + Static cap table
- v1.8 SAFE-3: Binary verifier + Task certificate
- v1.9 SAFE-4: Stack analyzer

## [Unreleased] - U-27 SNTM Phase 5

### Added (two-task demo + v1.5 closure)
- **`tasks/task_world/`** (yeni dizin) — ikinci native SNTM task (task_id=3):
  - `Cargo.toml`, `build.rs`, `task_world.ld` (MEMORY origin 0x80700000),
    `src/main.rs` (forever yield_cpu loop, fail-closed `panic = syscall::exit(255)`)
  - workspace member ([Cargo.toml:25](Cargo.toml#L25))
  - 16K text + 4K rodata + 4K data + 8K stack NAPOT regions
- **`sipahi.toml`**: `[[task]]` task_world entry (task_id=3, priority=7,
  budget=500_000, period=50, DAL-D) + 4 region (0x80700000+, 1MB margin from
  task_hello). FIX-C tuned values.
- **`src/kernel/pmp/generated.rs`**: PMP_PROFILES[3] regen (sntm-validate
  codegen) — NAPOT encoding bit-exact, region count=4.
- **`src/kernel/loader/`** generic helper refactor:
  - `NativeTaskSegments` struct (text + Option<rodata> + Option<data>)
  - `load_native_task(task_id, &segments)` — task_hello + task_world ortak path
  - `load_task_hello()` + `load_task_world()` thin wrapper
  - `load_region_zero_only` — segment'siz region için FIX-D zero-fill only
- **`src/kernel/loader/embed.rs`**: `TASK_WORLD_TEXT/RODATA/DATA` include_bytes!()
- **`scripts/build_native_tasks.sh`**: task_world build + sntm-pack invocation
- **`src/boot.rs`**: task_hello + task_world production live boot (U-26
  `cfg(self-test)` gate KALDIRILDI), her ikisi `budget=500_000 period=50`
  (FIX-C tune).
- **`src/ipc/mod.rs`**: `is_sealed()` accessor (cfg `any(self-test, kani)`) —
  SNTM-R13 test ve Kani proof için.
- **TLA+ SipahiSNTM extension**: Multi-task `LoadNative(t)` + `AssignChannel(c,p)` +
  `SealChannels` transitions + `channels` + `channelsAtSeal` ghost variables +
  `SealedAtomicityInvariant`. 23 → **138 distinct states**, 8/8 PASS.
- **Kani proofs** (4 yeni, R12+R13):
  - `check_ptr_in_profile_rejects_other_task_region` (SNTM-R12 statik)
  - `check_ptr_in_profile_symmetric_isolation` (SNTM-R12 symmetric)
  - `post_seal_assign_returns_false` (SNTM-R13 atomicity)
  - `seal_channels_idempotent` (SNTM-R13 idempotent)
- **Self-test entegrasyon (4 yeni)**: `test_pmp_profiles_disjoint`
  (R12 disjoint), `test_two_native_tasks_runnable` (R14 flags+state),
  `test_native_create_task_idempotent` (R14 FIX-G idempotent),
  `test_sealed_channel_assign_rejected` (R13 runtime).
- **sntm-validate negative tests**: `cross_task_overlap_rejected` +
  `two_tasks_disjoint_accepted` (tools/sntm-validate/tests/integration.rs).
- **coverage.toml** R12/R13/R14 entries (description + tests + proofs + fault_model).
- **`scripts/sntm_sprint_gate.sh` E5 fix** (Codex pre-review): `command -v
  sntm-pack` PATH lookup yanlıştı (false-green SKIP); repo-local
  `tools/sntm-pack/target/release/sntm-pack` veya `cargo build` fallback.

### Fixed (5 kritik kernel bug — U-27 production live boot ile keşfedildi)
- **`src/arch/context.S` trap frame mepc slot sync** (KRITIK, U-26'da masked):
  `switch_context` CSR mepc'ye yazıyor ama `trap.S .restore_regs` mepc'i trap
  frame slot'undan (`__stack_top - 136`) geri yüklüyor → switch_context'in
  CSR yazısı ezilir → mret yanlış (önceki task'ın) mepc'e gider → instruction
  access fault. U-26'da task_hello early-exit ile gizliydi; U-27 iki native
  task + tam preemption ile patladı. Fix: switch_context trap frame mepc
  slot'unu da güncellesin (`sd t0, -136(t1)`).
- **`src/arch/pmp.rs` legacy reload pmpaddr9..15 zero**: `write_per_task_napot`
  pmpcfg2'yi sıfırlıyor ama pmpaddr9..15 HW register'larında önceki native
  task'ın region adresleri kalıyordu. Functional impact yoktu (cfg OFF) ama
  `verify_pmp_integrity` shadow=0 bekliyor → HW eski değerlerle mismatch →
  PMP integrity FAIL → POLICY SHUTDOWN. Fix: 7× `csrw pmpaddrN, zero`.
- **`src/arch/pmp.rs` multi-region reload kullanılmayan pmpaddr zero**:
  `reload_pmp_profile` aktif region sayısından sonra kalan entries 12..15'i
  yazmıyordu (önceki task'ın value'su kalıyordu). Aynı shadow drift problemi.
  Fix: while loop sonu kullanılmayan entries için `write_pmpaddr_dyn(entry, 0)`.
- **`src/kernel/scheduler/mod.rs` native task `watchdog_window_min = 0`**:
  Yield-loop tight (yield_cpu/ecall/jump 3 instruction) ile
  WATCHDOG_WINDOW_MIN=3 ile çakışıyordu (kick-too-early → policy degrade
  cycle → instruction access fault). SNTM native task'lar için window check
  WASM-style sandboxed control flow için anlamlı; native RISC-V code için
  PMP + budget yeterli izolasyon. Fix: `t.watchdog_window_min = 0` (window
  check no-op, `watchdog_kick`'te `if window > 0` koşulu false → güvenli).
- **`src/kernel/scheduler/mod.rs` `native_create_task` idempotency (FIX-G)**:
  Aynı task_id ile ikinci çağrı state'i sessizce overwrite ediyordu (state
  corruption riski). Fix: `TaskState::Ready | Running` ise `None` döner
  (DENY), state preserved. SNTM-R14 test gate.

### Verification (§18.7 + §18.4)
- Kani proof count: 209 → **213** (+4 SNTM-R12/R13)
- TLA+: **8/8 specs PASS** (SipahiSNTM 23 → 138 distinct states)
- Self-test: **ALL TESTS PASSED** + 4 yeni `[OK]` marker (R12 disjoint,
  R14 runnable + idempotent, R13 sealed)
- Production smoke: **G5.a CI 30s** clean + **G5.b local 120s** clean
  (NF/FATAL/POLICY-free, 2 native task heartbeat)
- Coverage: **14 feature, 14 requirement** (R1..R14)
- SNTM sprint gate: **5 PASS, 0 FAIL, 2 SKIP** (E2 task_world dahil)
- Clippy: -D warnings PASS

### 14 SNTM Invariant Audit (8 carry + 6 yeni — hepsi GREEN)
1-8. U-25/U-26'dan korundu (PMP dyn 8..15, kernel 0..7 sabit, legacy
     fallback, multi-region path gating, shadow sync, native region kernel
     disjoint, zero-fill ÖNCE)
9. task_hello production live boot stable (120s clean)
10. task_world region 0x80700000+ disjoint (sntm-validate + runtime)
11. seal_channels atomic (Kani + TLA+ SealedAtomicityInvariant)
12. Cross-task PMP isolation statik (Kani + sntm-validate + runtime disjoint)
13. native_create_task idempotent (FIX-G)
14. PMP_PROFILES[2]+[3] disjoint (test_pmp_profiles_disjoint)

### Carry-forward (U-27.5)
- SNTM-R12 runtime ihlal observation (trap → restart×3 → isolate path)
- `cross-isolation-demo` feature (opt-in)
- `scripts/check_cross_isolation.sh` 4-gate verification

## [Unreleased] - U-26 SNTM Phase 4

### Added (SNTM Phase 4 — native task loader + sntm-pack tool)
- **`tools/sntm-pack`** (host tool sub-workspace, FIX-5 pattern): task ELF →
  per-section .bin packer via `object` crate. CLI: `--elf` + `--out-text` +
  `--out-rodata` + `--out-data`. 3 integration test (real task_hello ELF
  pipeline, FIX-E arg bounds-check ×2). FIX-C: SKIP YASAK — self-build veya
  hard FAIL.
- **`src/kernel/loader/`** module (yeni dizin): bounded_copy + zero_fill +
  is_safe_load_dst pure helpers (Kani-proven SNTM-R9) + `load_task_hello()`
  boot-time entry (U-26 scope: tek native task).
- **`src/kernel/loader/embed.rs`** include_bytes!() task_hello 3 segment
  (text/rodata/data) — kernel image embed. Path: `target/native/`.
- **FIX-A NATIVE_TASK_BASE = 0x80600000** (eski 0x80100000 wasm_arena 4MB
  içinde silent overwrite riski):
  - `sipahi.toml`: task_hello task_id 0→2 + 4 region 0x80600000+
  - `sipahi.ld`: `__native_task_base = 0x80600000` + `ASSERT(_end <= __native_task_base)`
  - `tasks/task_hello/task_hello.ld`: TEXT/RODATA/DATA origin 0x80600000+
  - `src/common/config.rs`: `KERNEL_BASE/KERNEL_SIZE=0x600000/NATIVE_TASK_BASE`
  - `src/kernel/pmp/generated.rs`: PMP_PROFILES[2] task_hello (regen)
- **FIX-B build pipeline integration**: Makefile build/check/run-self-test/
  debug → `build-native` depend; CI build + qemu-test job'larında native bin
  cargo step ÖNCESİ; `scripts/build_native_tasks.sh` (cargo build task_hello
  + sntm-pack ELF→.bin).
- **FIX-D info-leak guard**: `load_region` zero_fill ÖNCE, sonra bounded_copy;
  stack region da explicit zero_fill (CWE-457 uninitialized read defense).
- **FIX-F production smoke kanıtı**: sys_exit handler UART marker
  `#[cfg(any(feature = "trace", feature = "self-test"))]` — gerçek native
  task SYS_EXIT yolu trace+self-test build'lerde `[SYS] exit(task=X, code=Y)`.
- **`native_create_task(&NativeTaskConfig)`** API (scheduler/mod.rs):
  is_sntm_native=true, manifest-driven (PMP_PROFILES[task_id] sağlar), entry
  = text.base.
- **task_hello boot integration** (`#[cfg(feature = "self-test")]` gated):
  task_id=2, priority=6, dal=3, budget_cycles=100_000. **U-26 SCOPE LIMITATION**:
  production'da watchdog/budget tuning gerekli, full live boot U-27 demo
  hedef (typed IPC ile birlikte).
- **`task_state_for_test` + `set_current_for_test`** scheduler self-test
  helpers (#[cfg(feature = "self-test")]).
- **TLA+ SipahiSNTM extension**: `LoadNative(t)` transition + `LoaderInvariant`
  (kernel range overwrite YOK). 8/8 specs TLC PASS.

### Verification (§18.7 + §18.4)
- Kani proof count: 205 → **209** (+4: `loader_bounded_copy_atomic`,
  `loader_zero_fill_complete`, `loader_no_kernel_overwrite`,
  `loader_data_bss_composition_zero` — hepsi SNTM-R9)
- Kernel self-test: **5 yeni** [OK] marker:
  - `test_native_task_image_embedded` (R10: include_bytes! size + no ELF magic)
  - `test_native_task_loaded_to_region` (R10: byte-by-byte volatile + tail-zero FIX-D)
  - `test_native_task_bss_zero` (R10: data region tail zero)
  - `test_native_task_stack_zero` (R9 FIX-D: 8K stack volatile zero)
  - `test_sys_exit_runtime_isolates_task` (R11: isolate_task state + idempotent)
- sntm-pack integration: **3 test** (real ELF pipeline + 2 arg bounds-check)
- TLA+ SipahiSNTM: 23 distinct states, 0 invariant violation
- TLA+ tüm suite: **8/8 specs PASS**
- coverage.toml: **14 feature, 11 requirement** (R1..R11)
- Proof quality: 0 tautoloji
- Clippy: -D warnings PASS

### 6 FIX runtime-validated (Codex audit v2)
- **FIX-A** memory map relocate: sipahi.ld ASSERT geçti, generated.rs regen ✓
- **FIX-B** build pipeline: Makefile + CI integrate, clean clone'da otomatik ✓
- **FIX-C** sntm-pack no-SKIP: 3 integration test, ELF self-build veya FAIL ✓
- **FIX-D** zero-fill first: load_region 2-stage + stack zero, tests GREEN ✓
- **FIX-E** arg parsing bounds: 2 negative test (missing-value, no-args) ✓
- **FIX-F** sys_exit UART marker: trace+self-test build'de `[SYS] exit(task=2)` ✓

### 7 SNTM Invariant Audit (U-25'ten devam)
1-7. Hepsi korundu (PMP dynamic 8..15, kernel 0..7 unchanged, legacy task'lar
   fallback, is_valid_user_ptr dual-path, native task self-test scope, shadow
   senkron, native boot U-26'da self-test feature ile).

### Carry-forward (U-27 SNTM Phase 5 — two-task IPC demo)
- task_hello production boot (budget/period tuning + watchdog tune)
- task_world ikinci native task (task_id=3) — ilk IPC consumer
- Typed IPC channel sealing (`[[channel]]` manifest block)
- TLA+ SipahiSNTM channel sealing + IPC atomicity extension
- SNTM-R12 (typed IPC msg integrity), SNTM-R13 (channel seal atomicity)

## [Unreleased] - U-25 SNTM Phase 3

### Added (SNTM Phase 3 — multi-region PMP runtime + manifest-driven codegen)
- **`src/kernel/pmp/profile.rs`**: `Access` enum (Read/Write/Execute) +
  `Access::matches(perm)` pure helper (Kani-friendly, const fn).
- **`src/common/types.rs`**: `TaskConfig.is_sntm_native: bool` (FIX-3,
  default false — task_a/task_b legacy path preservation).
- **`src/kernel/scheduler/mod.rs`**:
  - `TaskControlBlock.is_sntm_native: bool` (Task struct field)
  - `is_task_sntm_native(task_id) -> bool` helper (FIX-3 dispatch routing)
  - Scheduler context switch (`do_priority_select_and_switch` + `start_first_task`):
    conditional reload — `is_sntm_native=true` → `reload_pmp_profile`,
    `false` → legacy `write_per_task_napot`. Mevcut task_a/task_b unaffected.
- **`src/kernel/syscall/dispatch.rs`**:
  - `is_valid_user_ptr(task_id, ptr, size, access: Access) -> bool` — signature
    refactor (FIX-4 dual-path: legacy stack-only ya da SNTM multi-region)
  - `check_ptr_in_profile(&PmpProfile, ptr, size, access) -> bool` pure helper
    (Kani-friendly, no CSR/global state)
  - `test_check_ptr_in_profile_for_task` self-test wrapper (is_sntm_native bypass)
  - sys_ipc_send Access::Read, sys_ipc_recv Access::Write callsite migration
- **`src/arch/pmp.rs`**:
  - `reload_pmp_profile(&PmpProfile)` — SNTM design v0.8 §4.5.3 sequence:
    Stage 1 DENY (pmpcfg2=0), Stage 2 sequential write entry 8..15,
    Stage 3 pmpcfg2 atomic write, Stage 4 sfence.vma, Stage 5 shadow update
  - `reload_indices_touched` Kani-friendly plan model ([u8;12], usize tuple — FIX-5)
  - `write_pmpaddr_dyn` helper (entry 8..15 only — FIX-1 lock guard)
  - `accumulate_cfg2` + `perm_to_cfg` helpers
  - `read_pmpaddr` indices 8..15 açıldı (multi-region verify için)
  - `write_pmpaddr` indices 8..15 açıldı (boot-time zero init için)
  - `PmpEncoding` import compat
- **`src/common/config.rs`**: `PMP_DYNAMIC_START_ENTRY = 8` (FIX-1: kernel 0..5 +
  UART 6..7 lock'lu), `MAX_PMP_ENTRIES = 16`, `MAX_DYNAMIC_PMP_ENTRIES = 8`.
- **`src/kernel/memory/mod.rs`** (FIX-2 multi-region shadow):
  - `PMP_SHADOW_DYN_ADDRS: [usize; 8]` static — pmpaddr8..15 shadow
  - `update_task_pmp_shadow` legacy path da `PMP_SHADOW_DYN_ADDRS[0]`'ı mirror'lar
  - `update_dynamic_pmp_shadow(&addrs, cfg2)` — reload_pmp_profile çağırır
  - `verify_pmp_integrity`: pmpaddr8..15 + pmpcfg2 multi-region check
  - `init_pmp` boot'ta pmpaddr9..15 explicitly 0 (defansif initial state)
- **`src/kernel/pmp/generated.rs`** (sntm-validate codegen output):
  manifest-driven PMP_PROFILES (task 0 = task_hello 4 region NAPOT, task 1..7 EMPTY).
- **`src/kernel/pmp/profile.rs::get_pmp_profile`**: `generated::PMP_PROFILES`'tan okur.
- **`tools/sntm-validate`** (host tool, FIX-6):
  - `napot.rs`: `napot_size_log2` + `napot_pmpaddr` pure helpers (5 unit test)
  - `codegen.rs`: `generate_pmp_profiles_rs` — manifest → generated.rs writer
  - `main.rs`: `--output-rs <path>` flag
  - `validate.rs`: `RESERVED_LOW_PMP_ENTRIES = 8` (FIX-6, eski KERNEL_PMP_ENTRIES=6 yanıltıcıydı)
  - Integration tests: `output_rs_codegen_round_trip` eklendi (7 total)
- **`scripts/regen_pmp_profiles.sh`** + Makefile `regen-pmp` target.
- **`Tla+/SipahiSNTM.tla` + `.cfg`**: task lifecycle (Loaded/Ready/Running/
  Isolated/Dead) + PMP reload atomicity. 6 invariant (TypeOK, KernelPmpInvariant,
  UModeRequiresDispatch, NoIsolatedRunning, AtMostOneRunning, RunningIsCurrent).
- **CI**: `generated-rs-drift` yeni job (`scripts/regen_pmp_profiles.sh` +
  `git diff src/kernel/pmp/generated.rs` exit-code check).

### Verification (§18.7 + §18.4 quality gates)
- Kani proof count: 200 → **205** (+5: `multi_region_user_ptr_in_region` R7,
  `multi_region_user_ptr_overflow_safe` R7, `multi_region_dead_task_deny` R7,
  `napot_encoding_size_consistent` R8, `reload_pmp_kernel_indices_untouched` R6)
- Kernel self-test: **4 yeni** [OK] marker:
  - `test_pmp_profile_loaded_from_manifest` (SNTM-R8 4-region content match)
  - `test_is_valid_user_ptr_multi_region_table` (SNTM-R7 15-case table)
  - `test_is_valid_user_ptr_access_perm_table` (SNTM-R7 9-case RX/R/RW×R/W/X)
  - `test_reload_pmp_profile_kernel_invariant` (SNTM-R6 CSR pmpcfg0+addr0..7
    preserved + verify_pmp_integrity GREEN post-reload — FIX-1 + FIX-2)
- TLA+ SipahiSNTM: 23 distinct states, 0 invariant violation (8/8 specs total).
- sntm-validate integration: 7 test PASS (+output_rs_codegen_round_trip).
- coverage.toml: **14 feature, 8 requirement (R1, R2-id, R3, R4, R5, R6, R7, R8)**.
- Test-first discipline: G3+G4 (kernel test + Kani proof) ÖNCE yazıldı, RED
  gözlemlendi (`cargo check` E0432 unresolved import `test_check_ptr_in_profile_for_task`
  + `reload_indices_touched`), sonra G5 + G8 GREEN yaptı.
- Tautology scan: 205 proof, 0 tautoloji.
- Clippy: -D warnings PASS.

### 7 SNTM Invariant Audit (Sprint başı user kontrolü)
1. ✅ PMP dynamic writes sadece entry 8..15 — `reload_pmp_profile` sadece
   pmpcfg2 + pmpaddr8..15 yazar (FIX-1).
2. ✅ Entry 0..7 hiçbir reload path'inde yazılmaz — pmpcfg0 read-modify-write
   tamamen kaldırıldı, UART entry 6/7 LOCK korunur.
3. ✅ Legacy task'lar (task_a/task_b) `is_sntm_native=false` — `boot.rs`
   create_task çağrıları explicit false, scheduler conditional legacy path.
4. ✅ `is_valid_user_ptr` legacy task'larda stack-only davranışı korur —
   `is_task_sntm_native(task_id) == false` ise `task_stack_range` path
   (cross_task_pointer_rejected self-test GREEN).
5. ✅ SNTM multi-region path sadece is_sntm_native=true veya test helper
   üzerinden — production `is_valid_user_ptr` is_sntm_native check, test
   wrapper `test_check_ptr_in_profile_for_task` flag bypass.
6. ✅ PMP shadow reload sonrası pmpcfg2 + pmpaddr8..15 ile senkron —
   `update_dynamic_pmp_shadow` reload_pmp_profile Stage 5 zorunlu;
   `verify_pmp_integrity` post-reload GREEN (G6 test invariant 3).
7. ✅ Native task boot U-26'ya kaldı — task_hello kernel'a embed edilmedi,
   `is_sntm_native=true` task şu an yok; `native_create_task()` API'si U-26.

### Carry-forward (U-26 SNTM Phase 4)
- `native_create_task(&NativeTaskConfig)` API (is_sntm_native=true setup)
- sntm-pack tool (task ELF → kernel image)
- Multi-task boot (task_hello native loader)
- Runtime SNTM-R2-full (exit isolate behavior, real task)
- Typed IPC channel sealing
- TLA+ SipahiSNTM extension (channel sealing + IPC atomicity)

## [Unreleased] - U-24 SNTM Phase 2

### Added (SNTM Phase 2 — manifest validator + PmpProfile types)
- **`src/arch/pmp.rs`**: `PmpEncoding` enum (NAPOT/TOR variants),
  HW-level encoding type per SNTM design v0.8 §4.5.4. `#[allow(dead_code)]`
  while runtime consumers (U-25) pending.
- **`src/kernel/pmp/`** module (yeni dizin): `mod.rs` (re-export profile+overlap),
  `profile.rs` (`Permission` RX/R/RW/NONE + `Region` + `PmpProfile`
  region_count + 6-region array, `EMPTY` const, `active_regions()`),
  `static PMP_PROFILES: [PmpProfile; MAX_TASKS]` placeholder + `get_pmp_profile(task_id) -> Option<&'static>`.
- **`src/kernel/pmp/overlap.rs`**: `regions_overlap()` + `valid_napot_alignment()`
  pure helpers (no_std, const fn, saturating_add). Kani-proven (R3, R5).
- **`tools/sntm-validate`** HOST tool (kendi sub-workspace — root workspace
  `-Z build-std` ile serde'yi RISC-V'e derliyordu, ayrı tutuldu):
  - `manifest.rs`: serde Deserialize structs (Manifest, KernelEntry,
    PlatformEntry, TaskEntry, RegionEntry).
  - `main.rs`: TOML parser + CLI (`--manifest <path>`), exit 0=PASS, 1=FAIL.
  - `validate.rs`: 6 invariant check — task ID uniqueness, NAPOT alignment,
    intra-task overlap, cross-task overlap, **kernel-task overlap** (SNTM
    design §4.5.2 shadow-attack koruma), PMP budget.
  - `tests/integration.rs`: 6 integration test (1 valid + 5 fault injection).
- **CI**: `sntm-validate` yeni job (build + integration tests + `sipahi.toml`
  validation, all on explicit HOST target).
- **`sntm_sprint_gate.sh` E4**: aktive edildi, `cd tools/sntm-validate &&
  cargo run --target $HOST` invocation pattern.

### Verification (§18.7 + §18.4 quality gates)
- Kani proof count: 198 → 200 (+`region_overlap_symmetric` SNTM-R3,
  +`napot_alignment_correct` SNTM-R5)
- Kernel self-test: 3 yeni (`test_regions_overlap_table` 12-case + symmetry,
  `test_napot_alignment_table` 14-case, `test_pmp_profile_struct_smoke`
  bounds + EMPTY). Hepsi table-driven semantics, scope-honest.
- Tool integration: 6 test (negative tests TOOL-SIDE — gerçek "manifest
  reject" senaryoları). Kernel self-test scope: pure helper semantics.
- Test-first discipline: G3 (kernel tests) + G4 (Kani proofs) HELPER'LARDAN
  ÖNCE yazıldı. `cargo check` `unresolved import crate::kernel::pmp::overlap`
  RED gözlemlendi → G5 helper implement → GREEN.
- §18.7 3-yorum: 5 yeni isim için VERIFIES/CALLS/FAILS-IF zorunlu, hepsi compliant.
- `coverage.toml`: 14 feature, 5 requirement (SNTM-R1, R2-id, R3, R4, R5).
- Tautology scan: 200 proof, 0 tautoloji.
- Production binary: unchanged (sntm-validate HOST tool, kernel binary
  PMP_PROFILES placeholder EMPTY — U-25 runtime integration).

### Carry-forward to U-25 (SNTM Phase 3)
- `src/kernel/pmp/generated.rs`: sntm-validate-üretilen PMP_PROFILES tablo
  (şu an placeholder EMPTY).
- `--output-rs` flag to sntm-validate (manifest → generated.rs codegen).
- `scheduler/mod.rs`: context switch'te multi-region PMP profile reload.
- `is_valid_user_ptr`: multi-region awareness (SNTM design §5.2).
- TLA+ spec: `SipahiSNTM.tla` task lifecycle.

## [Unreleased] - U-23 SNTM Phase 1

### Added (SNTM Phase 1 — sipahi_api body + task_hello scaffold)
- **`sipahi_api` body**: Error enum (8 variant + `from_kernel(usize) -> Option<Error>`),
  `ipc::Message` (64B repr(C), kernel binary-compatible), 6 syscall wrapper
  (cap_invoke/ipc_send/ipc_recv/yield_cpu/task_info/exit) + ecall0-3 trampolines.
- **6th syscall — `SYS_EXIT`** (id=5): kernel-side handler in `dispatch.rs::sys_exit`
  + new `WCET_EXIT = 15c` config sabit + `SYSCALL_COUNT` 5→6 +
  `SYSCALL_TABLE` 6 element + `check_wcet_limits` array 6 element +
  `syscall_ids_match_config` Kani proof SYS_EXIT line.
- **`scheduler::isolate_task`** visibility: `fn` → `pub(crate) fn` —
  mevcut helper kernel-side sys_exit'ten çağrılır (yeni yazılmadı).
- **`tasks/task_hello`** standalone crate: Cargo.toml + TASK-SCOPED
  `.cargo/config.toml` (kernel build etkilenmez) + `build.rs`
  (CARGO_MANIFEST_DIR absolute path linker arg) + `task_hello.ld`
  per-task linker script + `src/main.rs` (_start + yield loop + exit).
  ELF builds at `.text` 0x80100000, 5040 bytes, eh_frame/got discarded.
- **`sipahi.toml`** manifest scaffold (kernel + 1 task + 4 PMP regions);
  sntm-validate aktif Sprint U-24'te.
- **Cargo workspace** members: `[".", "sipahi_api", "tasks/task_hello"]`.
  Kernel Cargo.toml [dependencies] sipahi_api EKLEMEDİ (mimari ayrım).

### Refactored
- **Kernel linker script** `-Tsipahi.ld` `.cargo/config.toml`'dan Makefile
  RUSTFLAGS'a taşındı (kernel-only, task build'lere `union` merge ile
  sızmasın). `make build`/`check`/`debug`/`run-self-test` + feature_matrix.sh
  +KERNEL_RUSTFLAGS.

### Verification (§18.7 + §18.4 quality gates)
- Kani proof count: 197 → 198 (+`syscall_id_set_complete`, SNTM-R1)
- Kernel self-test: `test_syscall_id_table` (SNTM-R2-id, table-driven —
  6 sequential IDs + SYSCALL_COUNT + WCET_EXIT consistency)
- Test-first discipline: G3 (test+proof) WROTE FIRST, saw RED
  (compile error: SYS_EXIT not in config), G4 (kernel SYS_EXIT) made GREEN.
- Both new entries 3-yorum compliant (VERIFIES/CALLS/FAILS-IF).
- SNTM-R2-full (isolate behavior runtime test) DEFERRED to Sprint U-26
  (kernel native task loader requires booted task for runtime test).
- `coverage.toml`: 14 feature mapped (sntm artık active, deferred değil),
  2 requirement ID (SNTM-R1, SNTM-R2-id).
- Feature matrix: 8 → 10 kombinasyon (sntm + self-test,sntm eklendi).
- Production binary: unchanged (sntm default-off, kernel sipahi_api-free).

### SNTM Design Doc (referans — gitignored draft)
- v0.7 → v0.8: §8 stale "yeni syscall gerekmez" iddiası düzeltildi
  (SYS_EXIT panic_handler için zorunlu, §4.8.3 ile tutarlı).

## [1.1.1] - 2026-05-15

### Cleanup (U-22.5 — Pre-SNTM)
- **Removed**: `COMPUTE_COPY/CRC/MAC/MATH` ID constants (4, orphan).
- **Removed**: `WCET_COMPUTE_COPY/CRC/MAC/MATH` constants (4).
- **Removed**: `dispatch_compute()` + 4 helpers (`compute_copy/crc/mac/math`)
  in `src/sandbox/mod.rs` (WASM-tied orphan code).
- **Removed**: `E_COMPUTE_*` sabitleri (4 sandbox-internal error codes).
- **Removed**: Kani Proof 145 (`dispatch_compute_empty_data`).
- **Removed**: 2 verify.rs Kani proofs (`compute_ids_unique`,
  `host_call_budget_bounded` — both compute-path-tied).
- **Removed**: 4 `wcet_ordering_consistent` assertions (compute service ordering).
- **Updated**: `config.rs` tick budget `const_assert` — replaced
  `WCET_COMPUTE_CRC` with `WCET_CONTEXT_SWITCH` (worst-case kernel
  hot path component, re-balanced for post-WASM baseline).

### Fixed (Spec Compliance — SNTM-independent v1.0 bug)
- **`src/arch/pmp.rs::write_per_task_napot`**: Added missing
  `sfence.vma zero, zero` after PMP CSR writes (RISC-V Privileged Spec
  §3.7.2). QEMU TCG silently passes without it but CVA6 and production
  silicon require ordering barrier — fence prevents speculative execution
  from using stale PMP values across U-mode transition.
  **This is a Sipahi v1.0 bug fix independent of SNTM**, surfaced during
  SNTM design review (Codex 3rd round).

### Added (SNTM v0.7 Infrastructure)
- **`sipahi_api`** crate scaffolding (workspace member, empty modules
  `syscall`, `crc`, `ipc`). Implementation in Sprint U-23 (v1.5).
- **Cargo workspace** root manifest with `sipahi_api` member.
- **`sntm`** + **`sntm-safe`** umbrella feature flags (empty bodies,
  v1.5/v1.6+ targets, default-off — no partial SNTM in production).
- **`coverage.toml`**: Feature ↔ test/proof mapping (14 features mapped:
  2 active + 9 deferred + 3 non-safety; 7 grandfather entries;
  SNTM v0.7 §18.4 + §18.7 compliant; verified pre-existing).
- **`scripts/check_coverage.sh`**: Mechanical enforcement (symmetry,
  stale guard, name existence, deferred discipline, 3-comment rule,
  requirement traceability; verified pre-existing).
- **`scripts/check_proof_quality.sh`**: Light tautology detector
  (7 pattern, informational scan; verified pre-existing).
- **`scripts/sntm_sprint_gate.sh`**: SNTM sprint gate with graceful
  degrade (E0 coverage + E0b proof quality + baseline U-22 + E1-E9
  SNTM-specific; verified pre-existing).
- **`SIPAHI_V1_TO_V2_TRANSITION.md`**: Standalone transition planning
  doc, references SNTM v0.7 sprint roadmap + grandfather list.

### Documentation
- **ed25519-dalek**: Annotation about v2.0 migration to `ed25519-compact`
  (no_alloc alternative). Migration sprint: U-29.
- **WCET historical notes**: Compute service WCET values
  (COPY=80c, CRC=1500c, MAC=350c, MATH=200c) preserved in
  `SIPAHI_V1_TO_V2_TRANSITION.md` as historical benchmark reference.
- **`src/sandbox/mod.rs`** stale comments updated (`64KB` → `WASM_HEAP_SIZE`).

### Verification
- Kani proof count: 200 → 197 (3 compute-tied removed).
- All existing tests pass (ALL TESTS PASSED + 6 negative regression).
- Coverage map symmetric (14 features mapped, sntm/sntm-safe added).
- Proof quality scan: 0 warnings (197 proof clean, 4 grandfather).
- sntm_sprint_gate.sh PASS (E1-E9 SKIP — graceful, SNTM v1.5+ hedef).
- Production binary unchanged (no functional code modified outside
  removals and sfence.vma fix).

## [1.1.0] - 2026-05-10

### Security (U-22)
- **MP6**: `provision_key()` low-entropy check (`production-otp` build only).
  Distinct byte count < 8 → reject (KEY_READY false). Test-keys build paterni
  (`[0x5A; 32]`) production'da otomatik filtrelenir.
- **MP7**: `TokenCache::invalidate_all()` + `provision_key` cache flush hook.
  v1.0 boot-once'ta no-op gibi davranır; v2.0+ HSM key rotation senaryosunda
  meşru tokenlerin yanlış accept edilmesini engeller.
- **M11**: `deny.toml` explicit deny — `yanked = "deny"`, `ignore = []`,
  `bans.deny = []`, `bans.allow = []`. Fail-closed posture.
- **M14**: WASM sandbox feature-gated (`wasm-sandbox`). Production binary
  wasmi linklemiyor (~200KB tasarruf), `.wasm_arena` BSS = 0 byte
  (önce 4MB). ed25519 alloc trait için BumpAllocator placeholder kalıyor.
- **M15**: `ct_eq_16` constant-time disassembly CI gate — branch sayım
  fallback (inline durumunda) + sembol bulunduğunda direkt branch yasak.

### Code Quality (U-22)
- **M4**: `build.rs` linker ↔ config drift detection. `sipahi.ld`'de
  `ALIGN(8192)` ve `ALIGN(4096)` zorunlu, kayma compile-time fail.
- **M6**: IPC CRC kernel-enforce KARAR — opt-in by design (U-15
  doktrini), WCET impact 60c→1560c kabul edilemedi. `sys_ipc_send`'e
  policy explicit doc eklendi (v2.0+ HW CRC ile re-evaluate).
- **M7**: `validate_full` 9-step ordering invariant docstring.
- **M8**: `dispatch_compute` distinct error codes
  (E_COMPUTE_INVALID_OP/OVERFLOW/NOT_IMPL/SHORT_DATA).
- **M9**: `device.rs` v2.0 HAL abstraction → `v2-hal` feature-gated.
  Default build dead code yok.
- **M10**: WCET ordering tautoloji yerine compile-time tick-budget
  invariant (`config.rs` const_assert): worst-case syscall zinciri
  < CYCLES_PER_TICK (100_000). 11 toplam const_assert.
- **MP5**: PMP shadow update arasında MIE=0 debug_assert
  (schedule_timer_tick + start_first_task; production'da maliyet 0).
- **L14**: Linker `/DISCARD/` — `.eh_frame`, `.got`, `.comment`,
  `.note*`, `.dynsym/dynstr` bare-metal'de gereksiz, atıldı.
- **Senior**: LEB128 DRY — `read_u32_leb128` artık `read_leb128_u32`
  wrapper, çift implementasyon kaldırıldı.
- **Senior**: Test mesajları Unicode → ASCII (`✓`→`[OK]`, `✗`→`[FAIL]`,
  `→`→`->`, `★`→`*`). Terminal portability + CI grep coverage.

### Documentation (U-22)
- **L1**: Kani proof count senkron (200 actual, 11 const_assert).
- **L8**: `BB_NEXT_SEQ` u32 wrap davranışı: 1 olay/tick @ 100/s →
  ~497 gün, 6/s → ~23 yıl, 100/tick → ~5 gün. v2.0 hedef u64.
- **L11**: TLA+ TLC results repo'da (`Tla+/results/`). 7/7 PASS,
  35,770 distinct states. README.md tarihli özet.
- **L13**: Blackbox doc `seq:2B` → `seq:4B` (u32) — 64-byte record
  layout `[MAGIC:4][VER:2][PAD:2][SEQ:4][TS:4][TASK:1][EVENT:1][DATA:42][CRC:4]`.
- **L15**: `sandbox/mod.rs` stale yorumlar config sabitine yönlendirildi
  (CRC 120c → 1500c, 64KB arena → WASM_HEAP_SIZE).
- **L16**: `SipahiScheduler.tla` SCOPE DISCLAIMER — starvation freedom
  proof YOK (priority scheduling design), real-time deadline meeting
  proof YOK (timing model abstract).
- **L6+L7**: `verify.rs` doctrine docstring — Kani context'inde unwrap
  ve for-iter kabul edilir, production runtime'da YASAK.

### CI/Tooling (U-22)
- **L2**: Cargo.toml exact pin (`=X.Y.Z`) tüm dep'lerde
  (blake3=1.8.4, ed25519-dalek=2.2.0, wasmi=1.0.9). cargo update no-op.
- **G9**: `ct-eq-verify` CI job (G15 implementation).
- **G23**: `binary-guards` CI job — .eh_frame discarded,
  no float instructions, .text < 64KB, .wasm_arena absent in production.
- **G27**: `feature-matrix` CI job + `scripts/feature_matrix.sh` —
  8 kombinasyon (fast-crypto/fast-sign/test-keys × trace/debug-boot/
  self-test/wasm-sandbox/v2-hal).

### Skipped/Deferred (U-22)
- **G22**: src/tests dead code audit — analiz yapıldı, dead test fn YOK.
  test_fail/pass/result helper olarak args ile çağrılıyor.

## [1.0.1] - 2026-05-09

### Security (U-21)
- **H1**: POST production'a taşındı (mtvec, mtime, misa, medeleg/mideleg
  WARL all-ones detect, mcounteren, PMP integrity). halt_system fail-closed.
- **H2**: Default features secure boot kapalı sorunu — `compile_error!`
  guard, `production-otp` feature scaffold.
- **H4**: UART PMP Entry 7 production'da deny (feature-gated).
- **H5**: `schedule_yield()` ayrıldı — yield path'te tick state advance yok
  (blackbox tick, IPC rate, watchdog, budget, period — sadece timer).
- **H6**: Unknown exception triage (livelock yerine fail-closed halt).
- **H7**: `start_first_task` 16 caller-saved register clear (mret öncesi).

### Code Quality (U-21)
- **M1**: `mcounteren = 0` (U-mode timing side-channel kapalı).
- **M2**: `medeleg/mideleg = 0` (M-only delegation, WARL).
- **M3**: `write_mtvec` mode bit mask `& !0x3`.
- **M5**: `TaskContext` size `const_assert` (128 byte invariant).
- **MP1**: Phase 1/1.5 dual bound (`i < count && i < MAX_TASKS`).
- **MP2**: `E_RATE_LIMITED`/`E_INTERNAL` → `usize::MAX-N` schema.
- **MP3**: Pointer leak filter `__text_start..=_end` (kernel image full).
- **MP4**: `watchdog_counter.saturating_add(1)` (49-day overflow safe).
- **Senior S2**: Hot path `#[inline(always)]` consistency.

## [1.0.0] - 2026-05-08

Initial release. Sipahi microkernel v1.0:
- ~9.1K LOC + ~321 ASM
- 200 Kani proofs, 7 TLA+ specs (35,770 distinct states)
- RISC-V RV64IMAC, no_std + alloc-only-in-WASM
- DO-178C DAL-A design principles
