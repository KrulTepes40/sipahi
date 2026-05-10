# Changelog

All notable changes to Sipahi microkernel.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
