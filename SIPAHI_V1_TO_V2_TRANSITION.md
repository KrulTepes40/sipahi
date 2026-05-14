# Sipahi v1.x → v2.0 Transition Plan

> **Tarih:** 2026-05-15 (U-22.5 sonrası)
> **Tag:** v1.1.1 (Pre-SNTM cleanup) — pending user approval
> **Sonraki:** SNTM Phase 1 (Sprint U-23, v1.5)
> **Referans:** `SIPAHI_SNTM_DESIGN.md` v0.7 §11.5/§11.6/§11.7 + §17 + §18

---

## Durum Akışı

```
v1.1.0  U-22 cleanup tamamlandı (WASM feature gate, supply chain, build.rs)
v1.1.1  U-22.5 Pre-SNTM cleanup (bu sprint)
v1.5    SNTM Phase 1 (Sprint U-23 — sipahi_api + task_hello)
v1.6    SAFE-1 (Safe Native Profile + task-lint)
v1.7    SAFE-2 (Static Cap Table + Typed IPC) — EN YÜKSEK NET WIN
v1.8    SAFE-3 (Binary Verifier + Task Certificate)
v1.9    SAFE-4 (Stack Analyzer + Full CI Gate)
v2.0    SNTM Phase 5 + WASM tamamen sil + ed25519-compact
```

---

## v1.1.1 Sonrası Sipahi Durumu

```
TEMİZLENDİ (U-22.5):
  ✗ COMPUTE_* ID sabitleri (4)
  ✗ WCET_COMPUTE_* sabitleri (4)
  ✗ dispatch_compute fonksiyonu + 4 helper (compute_copy/crc/mac/math)
  ✗ E_COMPUTE_* sabitleri (4, sandbox-internal)
  ✗ Proof 145 (dispatch_compute_empty_data)
  ✗ verify.rs compute-tied 2 proof + 4 assertion
  ✗ Kani proof count: 200 → 197 (-3)

FIXED (Sipahi v1.0 spec compliance, SNTM-bağımsız):
  ✓ pmp.rs::write_per_task_napot — sfence.vma zero, zero
    (RISC-V Privileged Spec §3.7.2 — CVA6 speculative execution
    + memory pipeline ordering barrier zorunlu)

EKLENDİ (SNTM v0.7 Infrastructure):
  ✓ sipahi_api crate scaffolding (workspace member)
  ✓ Cargo workspace structure ([workspace] members = [".", "sipahi_api"])
  ✓ sntm / sntm-safe umbrella feature flags (boş, default-off)
  ✓ coverage.toml (14 feature + 7 grandfather + 9 deferred + 3 non-safety)
  ✓ scripts/check_coverage.sh (§18.4 mechanical enforcement, Python tomllib)
  ✓ scripts/check_proof_quality.sh (§18.7 light tautology detector)
  ✓ scripts/sntm_sprint_gate.sh (§18.8 graceful degrade E0/E0b/baseline/E1-E9)
  ✓ ed25519-compact migration note (v2.0 Sprint U-29 hedef)
  ✓ Sandbox stale yorum güncellendi (64KB → WASM_HEAP_SIZE 4MB)

KORUNDU (gate'li, v2.0'da silinecek):
  - src/sandbox/mod.rs (wasm-sandbox feature, WasmSandbox struct)
  - src/sandbox/allocator.rs (wasm-sandbox feature, BumpAllocator)
  - extern crate alloc (ed25519-dalek dependency)
  - global_allocator + alloc_error_handler (main.rs)
  - .wasm_arena linker section (production'da 0 byte, NOLOAD)
  - WASM Kani proofs (~13 proof, wasm-sandbox feature-gated)
```

---

## Historical WCET Reference (Compute Services — Removed)

Compute service WCET estimates pre-U-22.5 (FPGA pending):

| Service | WCET (cycle) | Algorithm |
|---------|--------------|-----------|
| COMPUTE_COPY | 80 | Memory copy (64B block, stub v1.0) |
| COMPUTE_CRC | 1500 | CRC32 bit-by-bit (64B input) |
| COMPUTE_MAC | 350 | BLAKE3 keyed hash (32B key + msg) |
| COMPUTE_MATH | 200 | Q32.32 vector dot product |

Bu değerler U-22.5'te silindi; SNTM v1.5 task-side typed IPC ile kategorize değiştirilecek (kernel'de compute service kavramı yok, task'lar kendi algoritmasını kendi PMP region'ında çalıştırır).

---

## Kani Proof Sayısı (Evolution)

```
v1.0.0:  ~177 proof (initial)
v1.0.1:  ~195 proof (U-21 6 regression test)
v1.1.0:  200 proof (post U-22, +5 SAFE/quality)
v1.1.1:  197 proof (U-22.5 -3 compute-tied) ← BU SPRİNT
v1.5:    197 proof + SNTM proof'lar (Sprint U-23)
v1.6+:   SAFE-1..4 ek proof'lar
v2.0:    ~185 proof (WASM tamamen silince ~13 wasm-sandbox gate'li silinir)
```

---

## SNTM Sprint Roadmap

```
U-23    SNTM Phase 1: sipahi_api + task_hello (~1 hafta)        → v1.5
U-24    SNTM Phase 2: Manifest + sntm-validate (~1.5 hafta)
U-25    SNTM Phase 3: Multi-region PMP profile (~1.5 hafta)
U-26    SNTM Phase 4: sntm-pack + task loader (~1.5 hafta)
U-27    SNTM Phase 5: Two-task demo (~1 hafta)                  → v2.0-rc
U-28    FPGA bring-up (donanım bekliyor)
U-29    WASM tamamen sil + ed25519-compact (~1 gün)             → v2.0
U-30+   SAFE-1..4 (v1.6 → v1.9)
```

---

## Tag Reference

```
v1.0.0  Initial release
v1.0.1  U-21 HIGH fix release
v1.1.0  U-22 cleanup + supply chain audit
v1.1.1  U-22.5 Pre-SNTM cleanup + v0.7 infrastructure ← BU SPRİNT (pending)
v1.5    SNTM Phase 1
v1.6    SAFE-1 (Safe Native Profile)
v1.7    SAFE-2 (Static Cap Table + Typed IPC)
v1.8    SAFE-3 (Binary Verifier + Task Certificate)
v1.9    SAFE-4 (Stack Analyzer)
v2.0-rc SNTM Phase 5 two-task demo
v2.0    WASM removed, ed25519-compact, no_alloc complete
```

---

## v0.7 Sprint Completion Gate Workflow

Tüm sonraki sprint'ler (U-23+) `scripts/sntm_sprint_gate.sh` kullanır:

```
[E0]   Coverage map (mechanical lazy-bypass guard, FAIL on missing entries)
[E0b]  Proof quality light scan (tautology detector, informational)
[BASE] U-22 sprint gate (8-step baseline)
       1. cargo check
       2. make check (clippy -D warnings)
       3. cargo kani (proof count regression check)
       4. make build
       5. production NF/FATAL check
       6. self-test ALL TESTS PASSED + [FAIL] grep
       7. no new TODO/FIXME (git diff)
       8. version banner consistency
[E1]   sipahi_api crate build (graceful, SKIP if not scaffolded)
[E2-E9] SNTM-specific (graceful degrade based on sprint phase)
```

---

## §18.7 Three-Comment Rule (Post-U-22.5)

Yeni eklenecek **her** test/proof için 3 yorum zorunlu (grandfather list muaf):

```rust
// VERIFIES: SNTM-Rx (veya SIPAHI-Rx) requirement ID
// CALLS:    production_fn1, production_fn2 (gerçek prod kod çağrı)
// FAILS-IF: hangi hatalı implementasyonda test/proof fail eder (fault model)
#[kani::proof]
fn new_proof_for_sntm() { /* ... */ }
```

Grandfather list (7 isim, pre-2026-05-13 baseline, §18.7 muaf):
- Proofs: `token_owner_mismatch_always_rejected`, `ct_eq_16_same_input_true`,
  `ct_eq_16_single_byte_diff_false`, `bump_allocator_offsets_never_overlap`
- Tests: `test_token_owner_mismatch_neg`, `test_cross_task_pointer_rejected`,
  `test_allocator_overflow`

Yeni isimler buraya eklenmez — onlar 3-yorum kuralına tabidir.

---

## Referanslar

- `SIPAHI_SNTM_DESIGN.md` v0.7 — Tam SNTM tasarımı (§17 SAFE, §18 sprint gate)
- `SIPAHI_AMCI_DESIGN.md` v0.8 — Multi-hart entegrasyon planı
- `CHANGELOG.md` — Per-release notları
- `coverage.toml` — Feature ↔ test/proof mapping
- `scripts/sntm_sprint_gate.sh` — Sonraki sprint'ler için gate workflow
