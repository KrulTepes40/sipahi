# Sipahi — Teknik Özellikler

Bu belge Sipahi deposunun güncel çalışma ağacındaki özellikleri özetler.
Amaç pazarlama yapmak değil; hangi özelliklerin gerçekten kodda bulunduğunu,
hangilerinin kısmi olduğunu ve hangilerinin yol haritasında kaldığını açık
şekilde ayırmaktır.

**Durum:** kernel `v1.1.1` + SNTM Native Task Model v2.0 +
**SNTM-SAFE faz tamamlandı (v1.9.0, sprint-u33)**
**Hedef:** RISC-V RV64IMAC, QEMU `virt`, single-hart
**Dil:** Rust `no_std + no_alloc`, bare-metal
**Doğrulama:** **204 Kani harness, 9 TLA+ model**, self-test + SAFE gate
(10/10 aktif), coverage map (14F + 20R)

Sipahi sertifikalı bir RTOS değildir. DO-178C/DAL-A tarzı bazı tasarım
prensiplerini uygular; ancak sertifikasyon, gerçek donanım WCET raporu,
bağımsız review, requirements traceability, safety case ve tool qualification
gibi ayrı kanıtlar gerektirir.

---

## 1. Çekirdek Mimari

- Kernel Machine mode'da çalışır.
- Task'lar User mode'a `mret` ile düşer.
- S-mode/MMU kullanılmaz; bellek ayrımı PMP ile yapılır.
- Boot, trap entry, timer interrupt ve context switch RISC-V assembly ile
  desteklenir.
- Kernel tek-hart varsayımıyla tasarlanmıştır. Multi-hart/AMCI çalışmaları
  ayrı tasarım dokümanlarında takip edilir.

### Mevcut ana modüller

- `src/arch`: boot, trap, CSR, PMP, CLINT, UART, context switch
- `src/kernel/scheduler`: task tablosu, priority seçimi, budget, watchdog
- `src/kernel/syscall`: syscall ABI, dispatch table, WCET tracking
- `src/kernel/capability`: token, broker, cache + SAFE-2 `cap_action` /
  `cap_generated` / `local_cap` modülleri
- `src/kernel/loader`: SNTM native task loader (bounded copy + zero fill + PMP)
- `src/kernel/pmp`: manifest-driven `PMP_PROFILES` codegen hedefi
- `src/ipc`: SPSC IPC kanalları + blackbox recorder
- `src/kernel/policy`: failure policy engine (lockstep)
- `sipahi_api`: task-side syscall ABI + typed IPC `channels.rs` codegen hedefi
- `tasks/task_hello`: native task #2
- `tasks/task_world`: native task #3 (SAFE-2 typed IPC consumer)
- **Host tool ekosistemi** (`tools/`): task-lint, sntm-validate, sntm-pack,
  riscv-bin-verify, sntm-cert-gen, sntm-image, sntm-stack — her biri
  sub-workspace, `bash scripts/sntm_safe_gate.sh` ile invoke edilir.
- (v1.x `src/sandbox/` WASM prototype path'i U-29 v2.0'da tamamen kaldırıldı.)

---

## 2. Privilege ve Trap Modeli

### Var olan özellikler

- Kernel M-mode, task'lar U-mode.
- `mstatus.MPP = U` ve `mret` ile task entry.
- `mscratch` tabanlı trap stack swap.
- `task_trampoline` context switch sonrası fresh task'ı U-mode'a geçirir.
- U-18'de giderilen nested fault problemi sonrası trampoline `mscratch` ve
  user stack invariant'ını restore eder.
- U-19'da task entry öncesi caller-saved register temizliği eklendi.
- `mcounteren = 0`; U-mode timing counter erişimi kapalı.
- `medeleg/mideleg` sıfırlanır; M-only kernel modeli korunur.

### Sınırlar

- Gerçek donanımda trap latency ve PMP davranışı FPGA/silikon üzerinde ayrıca
  ölçülmelidir.
- Multi-hart trap/interrupt modeli henüz runtime'da yoktur.

---

## 3. PMP ve Bellek Koruması

### Var olan özellikler

- Kernel `.text`, `.rodata`, `.data+bss+kernel_stack` ve UART MMIO bölgeleri
  PMP ile korunur.
- Kernel bölgelerinde L-bit kullanılır.
- Task stack'leri NAPOT-aligned per-task region'lardadır (manifest-driven).
- Her context switch'te task'a özel multi-region PMP profile programlanır
  (SNTM §17 modeli).
- Per-task PMP yazımından sonra `sfence.vma zero, zero` uygulanır.
- PMP shadow integrity kontrolü scheduler tick path'inde korunur.
- `is_valid_user_ptr(caller_task_id, ptr, size)` task'a özeldir:
  sadece çağıran task'ın kendi region range'i kabul edilir.
- Dead/isolated/uninitialized task için pointer valid range yoktur.

### SNTM Native Task Model (v2.0)

- `sipahi.toml` manifest tam dolu: kernel + platform + tasks + regions +
  resources + channels + local_caps.
- `src/kernel/pmp/generated.rs` (`PMP_PROFILES`) sntm-validate `--output-rs`
  ile manifest'ten emit edilir (CODEGEN; CI drift gate aktif).
- Runtime multi-region PMP reload çalışıyor (SNTM-R6/R7/R8 Kani doğrulu).
- Cross-task PMP isolation runtime gate (4-gate verify: trap isolation,
  no BROKEN marker, post-trap tick continuity, no FATAL/NF/POLICY).

---

## 4. Scheduler

### Var olan özellikler

- Fixed-priority preemptive scheduler.
- `MAX_TASKS = 8`.
- Task state modeli: Ready, Running, Suspended, Isolated, Dead.
- Priority selection helper'ları Kani proof'larıyla desteklenir.
- `schedule_timer_tick()` ve `schedule_yield()` ayrıdır:
  - timer tick path period/budget/watchdog/blackbox/PMP integrity advance eder
  - yield path sadece priority select + context switch yapar
- Budget accounting `saturating_sub` ile yapılır.
- Watchdog sadece Running task için artar.
- Degrade/recovery path'inde cooldown bulunur.
- `isolate_task()` task'ı Isolated yapar ve capability'leri invalidate eder.
- `SYS_EXIT` handler'ı current task'ı isolate edip scheduler'a kontrol verir.

### Sınırlar

- WCET değerleri şimdilik tahmindir; gerçek cycle ölçümü FPGA/silikon üzerinde
  yapılmalıdır.
- Task migration veya multi-hart scheduling yoktur.

---

## 5. Syscall ABI

### Var olan syscall'lar

| ID | Syscall | Durum |
|---:|---|---|
| 0 | `cap_invoke` | capability kontrol (BLAKE3 MAC + nonce + owner) |
| 1 | `ipc_send` | non-blocking IPC send |
| 2 | `ipc_recv` | non-blocking IPC receive |
| 3 | `yield` | scheduler'a kontrol bırakma |
| 4 | `task_info` | task state/priority/DAL bilgisi |
| 5 | `exit` | voluntary task termination |
| (n/a) | `local_cap_invoke` | SAFE-2 typed cap action (cap_invoke ID + extra check) |

`SYSCALL_COUNT = 6`. SAFE-2 `local_cap_invoke` `sys_cap_invoke` üzerinden
dispatch edilir; `LOCAL_CAP_TABLE[task][resource]` lookup yapılır; argv
reserved bits forward-compat için kontrol edilir.

### Var olan korumalar

- O(1) function pointer dispatch table.
- Geçersiz syscall ID → `E_INVALID_SYSCALL`.
- Kernel pointer benzeri return değeri → `E_INTERNAL` ile sanitize.
- IPC pointer'ları task-specific validation + alignment check.
- `sys_cap_invoke` argüman truncation + SAFE-2 reserved bits check.
- `rdcycle` ile syscall WCET last/max ölçümü.
- `check_wcet_limits()` 6 syscall için limit array'i.
- `Error::from_kernel` (sipahi_api) 8 raw value mapping (SAFE-3 CR-1 ABI hizalı).

---

## 6. Capability Sistemi

### Var olan özellikler

- 32-byte `Token` yapısı.
- BLAKE3 keyed MAC doğrulama path'i.
- `ct_eq_16` constant-time MAC karşılaştırması.
- 4-slot token validation cache.
- Per-task nonce replay guard.
- Token expiry kontrolü.
- Token owner enforcement.
- Cache invalidation by token/owner ve task isolate sırasında capability revoke.
- **SAFE-2 static `LOCAL_CAP_TABLE`** — per-task `[task][resource] →
  CapAction` enforcement, `sipahi.toml [[task.local_cap]]` üzerinden
  sntm-validate CODEGEN ile gelir. Manifest dışı action (None/Read/Write/
  ReadWrite/Execute/All) syscall'da reddedilir; CI drift gate'i runtime
  uyumsuzluğu yakalar.
- `production-otp` feature path'i production provisioning için extern symbol
  bekler; yanlış production build link-time fail.
- `test-keys` development/CI default path'idir.

### Sınırlar

- BLAKE3 v1.x hızlı prototip MAC olarak kullanılıyor.
- Kani kriptografik güvenliği kanıtlamaz; sadece bounds + ordering + API
  invariant'larını kontrol eder.

---

## 7. IPC

### Var olan özellikler

- 8 statik SPSC channel (`MAX_IPC_CHANNELS = 8`).
- Her kanal 16 slot ve 64-byte mesaj (`IPC_MSG_SIZE = 64`).
- `AtomicU16` head/tail, Release/Acquire ordering.
- `send` ve `recv` O(1), non-blocking.
- Channel ownership: boot'ta atanır, `seal_channels()` ile kilitlenir.
- Atanmamış channel default-deny davranır.
- `can_send` / `can_recv` ownership check sağlar.
- CRC32 helper'ları (`set_crc`, `verify_crc`).
- IPC send rate limiting tick başına uygulanır.
- **SAFE-2 typed IPC**: `sipahi_api::channels::send_<msg>` /
  `recv_<msg>` wrapper'lar manifest'teki `[[channel]]` üzerinden
  sntm-validate ile emit edilir. Yanlış struct = task crate'de compile error.
- `BOOT_CHANNELS` tablosu producer/consumer çiftini emit eder (CI regen drift).

### Sınırlar

- CRC kullanımı kernel tarafından zorunlu kılınmaz; helper-driven.
- SPSC model enforced edilir; MPMC yoktur.

---

## 8. Policy Engine ve Degrade

### Var olan özellikler

- Pure `decide_action(event, restart_count, dal)` fonksiyonu.
- Failure mode seti: Restart, Isolate, Degrade, Failover, Alert, Shutdown.
- `PolicyEvent` variants: `BudgetExhausted=0`, `StackOverflow=1`, `TaskFault=2`,
  `CapViolation=3`, `IopmpViolation=4`, `PmpIntegrityFail=5`,
  `WatchdogTimeout=6`, `DeadlineMiss=7`, `MultiModuleCrash=8`.
- Failover v1.x'te gerçek hot-standby switch değil; Degrade fallback.
- Policy lockstep: karar fonksiyonu iki kez; farklı sonuç Shutdown'a gider.
- `black_box` input/output fence ile compiler CSE riski azaltılır.
- Restart counters saturating davranır.
- Degrade DAL-C/D task'ları durdurur, DAL-A/B önceliği korur.
- Recovery path cooldown ile flapping azaltır.
- **SAFE-4 Kani-doğrulu**: `stack_overflow_policy_event_mapping` —
  `PolicyEvent::StackOverflow` → decide_action tüm DAL×restart kombinasyonu
  için yalnız Restart veya Isolate döner (K7 no dead arms; DAL-D 3-restart
  politikası korunur).

### Sınırlar

- Gerçek yedek task failover runtime'ı yoktur.

---

## 9. Blackbox Flight Recorder

### Var olan özellikler

- 8 KB statik circular buffer.
- 128 kayıt, her kayıt 64 byte.
- CRC32-protected record format.
- Monotonic tick kaydı (u64).
- Policy, PMP, watchdog, lockstep, POST warning event'leri loglanır.
- Write position bounds guard bulunur.

### Sınırlar

- Kalıcı storage'a otomatik flush yoktur.
- Multi-hart aggregation yoktur.

---

## 10. WASM Sandbox Durumu (tarihsel — v2.0'da kaldırıldı)

**U-29 v2.0: WASM tamamen kaldırıldı.** v1.x'te (Wasmi 1.0.9 + 4MB bump
allocator + float opcode rejection + LEB128 parser + fuel metering) prototype
path olarak vardı, `wasm-sandbox` feature arkasında gated. U-29'da:

- `wasmi` dep silindi
- `wasm-sandbox` feature silindi
- `src/sandbox/` klasörü tamamen silindi (~700 LOC)
- `.wasm_arena` linker section silindi
- ~13-15 WASM-tied Kani proof silindi (213 → 189)
- `extern crate alloc` + `#[global_allocator]` + `#[alloc_error_handler]` silindi
- `ed25519-dalek` (alloc dep) → `ed25519-compact` (pure no_alloc) migration

Kernel artık pure `no_std + no_alloc`. SNTM Native Task Model v2.0 final.

---

## 11. SNTM Native Task Model (post-Phase-5)

### Var olan özellikler

- `sipahi_api` crate (no_std + no_alloc, ed25519-compact + blake3 path-dep):
  - `Error` enum 8 variant (`InvalidSyscall=0`..`Internal=7`) +
    `from_kernel` mapping (SAFE-3 CR-1 ABI hizalı).
  - 64-byte `ipc::Message` + typed `send_<msg>`/`recv_<msg>` codegen wrapper.
  - 6 syscall wrapper + SAFE-2 `local_cap_invoke`.
  - Per-task feature flag `task_<name>` channel erişimini gate eder.
- `SYS_EXIT = 5` kernel syscall handler.
- `tasks/task_hello` (id=2) yield + IPC + exit loop.
- `tasks/task_world` (id=3, SAFE-2) typed IPC consumer.
- `sipahi.toml` manifest tam dolu.
- `sntm` + `sntm-safe` feature flag'leri; default-off; SAFE-1..4 işi
  build-time gate driven (runtime feature değil).
- Kernel crate `sipahi_api` dep almaz; task crate doğrudan path dep kullanır.

### Cross-task isolation

- **Statik kanıt** (Kani SNTM-R12): manifest-driven per-task PMP profilleri +
  symmetric region rejection — her iki yön de Kani-doğrulu.
- **Runtime kanıtı** (`scripts/check_cross_isolation.sh`): 4-gate
  (trap isolation, no BROKEN marker, post-trap tick continuity, no
  FATAL/NF/POLICY; DAL-D 3-restart policy validated).

---

## 12. SNTM-SAFE phased rollout (sprint-u30..u33)

### SAFE-1 (v1.6.1) — task-lint Safe Native Profile

- `tools/task-lint/` host tool (~700 LOC, syn 2.0 AST visitor, cfg-aware).
- **11 yasak kural** task source'ta:
  1. `unsafe` block
  2. `extern "C"` FFI
  3. `alloc::*` import (heap-free task disiplini)
  4. inline `asm!`
  5. recursion (call graph cycle detect)
  6. `dyn` trait + function pointer
  7. `panic_unwind`
  8. `#[link_section = ".init_array"]`
  9. `f32`/`f64` floating-point
  10. `core::sync::atomic`
  11. MMIO raw pointer cast (volatile arithmetic)
- **DAL-aware `trust_tier` enforcement**:
  - `safe` (default) → 11 kural HARD-FAIL
  - `trusted_unsafe` (manifest opt-in):
    - DAL-A/B → HARD-FAIL (doctrine)
    - DAL-C/D → `waiver_reason` zorunlu + `demo_feature_waivers` Cargo feature
      listesi (default-OFF; CI drift guard)
- Safe gate [2/10] aktif; CI `task-lint` job + production binary unsafe leak
  guard (objdump, cfg compile-out check).
- 18 integration test.

### SAFE-2 (v1.7.0) — Static cap table + typed IPC

- `src/kernel/capability/cap_action.rs` — `CapAction` 6-variant enum +
  `from_u8` (None, Read, Write, ReadWrite, Execute, All).
- `src/kernel/capability/cap_generated.rs` — CODEGEN: `LOCAL_CAP_TABLE` (per
  task × resource action grant) + `BOOT_CHANNELS` (id, producer, consumer).
- `src/kernel/capability/local_cap.rs` — `local_cap_invoke` syscall wrapper.
- `sipahi_api/src/channels.rs` — CODEGEN: per-channel typed `send_<msg>` /
  `recv_<msg>` wrapper.
- Manifest schema genişletme: `[[resource]]`, `[[channel]]`, `[[task.local_cap]]`.
- TLA+ `ChannelOwnershipInvariant` + `StrongChannelOwnership` (sealed
  atomicity birleşik).
- Safe gate [3/10] cargo +nightly build (typed IPC compile guard) +
  [7/10] cap_generated drift + [8/10] channels drift.
- Kani +7 harness (typed_ipc cross-crate K8, CapAction roundtrip,
  BOOT_CHANNELS well-formed, sys_cap_invoke reserved bits).

### SAFE-3 (v1.8.0) — Binary verifier + TaskCertificate + signed image

- `tools/riscv-bin-verify/` — RV64IMAC instruction whitelist (~1700 LOC):
  - ALLOW: base RV64I + M + A + RVC (c.ld/c.sd/c.ldsp/c.sdsp = integer)
  - ALLOW: `ecall` (kernel syscall — CR-10)
  - REJECT: F/D floating-point (c.fld/c.fsd reject; integer RVC OK)
  - REJECT: CSR instructions, `mret`, `ebreak`
  - Symbol filter: STT_FILE/STT_SECTION/SHN_ABS/SHN_UNDEF SKIP (CR-11)
  - Region check: task code kernel range dışında
  - 18 unit + 21 integration test (synthetic ELF builder)
- `tools/sntm-cert-gen/` — TaskCertificate `repr(C)` 424B ABI v1:
  - BLAKE3 hash chain: manifest, toolchain (`rust-toolchain.toml`),
    `source_commit` (git HEAD veya zero sentinel), text/rodata/data
  - ed25519-compact RFC 8032 sign + verify
  - 14 integration test (RFC 8032 + tamper + SAFE-4 cert flow).
- `tools/sntm-image/` — Signed image:
  - `SIPI1` 5-byte magic + 64-byte header (kernel/body/tail_sig offsets)
  - Kernel ELF + task cert'ler + task `.bin` payload'lar
  - 64-byte tail ed25519 signature
  - 11 integration test (roundtrip + tamper magic/body/sig).
- `Tla+/SipahiSecureBoot.tla` — 6-state image verify spec, invariant:
  `StartedImpliesValid`, `NoFalseAccept`, `AtomicVerify`,
  `SigValidImpliesHeader`.
- `keys/dev-image.{priv,pub}` ed25519 keypair (`gen_dev_key.sh` bootstrap;
  `.priv` gitignored).
- Safe gate [4/10] riscv-bin-verify + [9/10] cert sign+verify + [10/10] image
  assemble+verify.
- Kani +6 harness (cert_field_layout_pin, image_magic_invariant,
  image_header_size_invariant, verify_cert_signature_bounded,
  syscall_error_abi_alignment).

### SAFE-4 (v1.9.0) — Stack analyzer + 10/10 gate (Plan B)

- **Plan B karar**: `cargo-call-stack 0.1.16` current nightly ile uyumsuz
  (`error: unsupported rust toolchain`; rustc wrapper intercept 2023-11
  hard-coded). LLVM `-Z emit-stack-sizes` ELF section direkt parse.
- `tools/sntm-stack/` host tool (~800 LOC):
  - `object 0.36.5` ELF parser
  - ULEB128 `.stack_sizes` decode (8-byte LE addr + ULEB128 size per fn)
  - **AUIPC+JALR pair detect** (linker-resolved direct call/tail)
  - **Indirect REJECT**: bare JALR (rd!=x0), c.jalr, c.jr (rs1!=x1)
  - **JAL / c.j direct edge**
  - **DFS recursion cycle detect**
  - **Sum-of-frames over-approximation** (raporda açık caveat)
  - 23 unit + 9 integration test
  - Golden fixture `task_hello.stack.golden.txt` committed
- `src/common/config.rs`:
  - `STACK_ANALYSIS_MARGIN_BYTES = 256` (CR-5 doctrine)
  - `STACK_ANALYSIS_UNKNOWN_SENTINEL = 0xFFFF_FFFF` (CR-4)
- `tools/sntm-validate/src/stackreport.rs` + `validate.rs::check_stack_bounds`:
  - `stack_size ≥ observed_max + margin` formula (exact equality REJECT)
  - UNKNOWN sentinel her zaman REJECT
  - 12 unit + 5 integration test
- `tools/sntm-cert-gen/src/stackreport.rs` (FIX-G shared crate deferred):
  - `--call-stack-report` sntm-stack çıktısını parse eder
  - Eksik veya FAIL → `max_stack_bytes = UNKNOWN_SENTINEL`
  - **Manifest `stack_size` ASLA fallback** (CR-4: allocation vs observation)
  - 4 yeni integration test.
- `Tla+/SipahiSNTM.tla` `StackRegionBound` invariant (state count 138 baseline
  korundu).
- `src/verify.rs` +3 Kani: `stack_analysis_margin_pin` (K2 const literal),
  `stack_bounds_invariant` (K3+K5 sembolik formula + exact equality reject),
  `stack_overflow_policy_event_mapping` (K7 PolicyEvent::StackOverflow=1).
- **Safe gate [5/10] aktif — 10/10 aktif, DEFER yok, SAFE faz kapanışı.**
- `scripts/stack_analysis.sh` runner `env -u RUSTFLAGS` ile (SAFE-3 lesson).
- CI `sntm-stack` job: build + integration + stack bound validation.
- `docs/safe/cert_abi_v2_migration.md` — ABI v2 plan (doc only, post-CFI).
- `coverage.toml` `SNTM-SAFE-R6` requirement `required_tool_tests` schema ile
  (8 tool test + 3 Kani proof + 2 script).

### Carry-forward (post-SAFE faz)

- **CFI hardware faz** (Zicfilp landing pad + Zicfiss shadow stack —
  CVA6-CFI ready)
- **Stack scribble debug-boot redesign** (low-watermark region-bottom scan —
  SAFE-4 CR-6 doctrine; "stack top -8 sentinel" yanlış konum — RISC-V
  downward stack growth)
- **HSM/OTP production key sprint** (`keys/dev-image.priv` →
  HSM-provisioned)
- **TaskCertificate ABI v2** (CFI landing pad list, post-quantum sig
  migration)
- **Shared `sntm-manifest` lib crate** (SAFE-2 FIX-G — sntm-validate +
  riscv-bin-verify + sntm-cert-gen + sntm-stack manifest struct
  unification)
- **`sipahi_api` task-lint scope** (SAFE-2 CR-4 — `[[support_crate]]`
  design)

---

## 13. Doğrulama Altyapısı

### Var olanlar

- **204 Kani harness** (kernel-side; host tool fixture'ları cargo test'te).
- **9 TLA+ spec** (Scheduler, Capability, Policy, Watchdog, DegradeRecover,
  BudgetFairness, IPC, **SNTM** [138 states, post-SAFE-4 StackRegionBound ek],
  **SecureBoot** [6 states, SAFE-3]).
- `make check` — Clippy `-D warnings`.
- `make run-self-test` — POST + integration/self-test path.
- `scripts/sipahi_sprint_gate.sh` — legacy kernel umbrella.
- `scripts/sntm_sprint_gate.sh` — SNTM v1.x umbrella.
- **`scripts/sntm_safe_gate.sh` — SAFE umbrella (10/10 aktif, DEFER yok).**
- `scripts/stack_analysis.sh` — SAFE-4 sntm-stack runner.
- `scripts/check_coverage.sh` — coverage.toml ↔ source traceability
  (14 feature + 20 requirement; `required_tool_tests` SAFE-4 schema).
- `scripts/check_proof_quality.sh` — Kani harness adequacy heuristic.
- `scripts/feature_matrix.sh` — 10 feature kombinasyonu build.
- `scripts/check_cross_isolation.sh` — SNTM-R12 4-gate runtime.
- GitHub Actions: 15 job (build, qemu, audit, Kani full + PR subset,
  task-lint, sntm-validate, sntm-pack, sntm-stack, mutation, ct-eq, ...).

### Doğrulama sınırları

- Kani bounded model checking yapar; tüm concurrency/hardware davranışını
  kanıtlamaz.
- TLA+ modelleri soyut protokolleri kontrol eder; Rust implementation ile
  birebir refinement proof yoktur.
- Coverage check isim tabanlı mekanik guard'dır; test/proof semantik kalitesi
  review gerektirir (`// VERIFIES: ID` + `// CALLS: ...` + `// FAILS-IF: ...`
  üçlüsü non-grandfathered için zorunlu).
- QEMU gerçek cache, bus contention, PMP timing ve FPGA platform etkilerini
  modellemez.
- SAFE-4 stack analyzer sum-of-frames over-approximation kullanır;
  call-graph-aware transitive analiz post-SAFE roadmap.

---

## 14. Bilinçli Sınırlar ve Roadmap

Bu özellikler dokümanlarda/planlarda yer alır ama mevcut runtime guarantee
değildir:

- AMCI multi-hart runtime
- SPMP / WorldGuard / IOPMP production enforcement
- CLIC entegrasyonu
- Scratchpad/TCM optimizasyonu
- CHERIoT research branch
- Hardware CFI (Zicfilp + Zicfiss)
- TaskCertificate ABI v2 (post-CFI)
- HSM-provisioned production key chain (post-SAFE)
- Smepmp adoption (`mseccfg.MML=1`)
- Real FPGA WCET database
- TLAPM tabanlı Rust refinement proof
- Call-graph-aware transitive stack analysis (SAFE-4 sum-of-frames yerine)
- Runtime stack-overflow watermark (SAFE-4 CR-6 redesign)

Bu ayrım özellikle önemlidir: Sipahi'de tasarım yönü güçlüdür, ama her tasarım
fikri mevcut kod özelliği değildir.
