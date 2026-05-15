# Sprint U-24: SNTM Phase 2 — Manifest Validator + Type Definitions (14 görev, ~1 hafta)

## ⚠ PREREQUISITE: U-23 KAPAT

Sprint başlamadan ÖNCE worktree temiz olmalı:

```bash
git status --short
# Bekleniyor: temiz veya sadece gitignored draft'lar.
git tag --list | grep -E "v1.5"
# v1.5.0-alpha1 (veya owner kararı) atılmış olmalı — U-23 sonu hedefi.
```

**U-23 uncommitted ise sprint U-24 BAŞLAMASIN** — sprint owner önce U-23
commit + tag atar, sonra U-24 başlar.

---

## SİPAHİ DOKTRİNİ (her görev için geçerli)

```
- no_std + alloc YASAK (kernel kodu). Tools/sntm-validate HOST tool olduğu için
  std + alloc kullanır — istisna.
- panic, float, dynamic dispatch kernel kodunda YASAK.
- Yeni unsafe block → SAFETY:// yorumu zorunlu (kernel + tool).
- Erken çıkış yok lookup/compare path'lerinde.
- Yeni dependency — sadece host tool tarafında, KERNEL'e EKLENMEZ:
    - toml (=0.8.x)       — TOML parser
    - serde derive (=1.x) — Deserialize derive
    - tempfile (=3.x)     — dev-dependency, integration tests
  Kernel sipahi.toml parse etmez (SNTM design §4.6).
- HOST TARGET çağrısı: Sipahi root .cargo/config.toml `[build] target =
  "riscv64imac-unknown-none-elf"` — host tool için EXPLICIT host target gerekli:
    HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')
    cargo {build,test,run} -p sntm-validate --target "$HOST_TARGET" ...
  Aksi: tool RISC-V'e derler, çalıştırılamaz.
- Mevcut pattern taklit et: ct_eq_16, vol_read!, SingleHartCell, set_crc().
- Sembol değişikliği öncesi grep -rn "<symbol>" src/ → tüm call-site atomic.
- Hiçbir feature kombinasyonu cargo check'te FAIL etmeyecek.
- TODO/FIXME/HACK comment YASAK.
- "DONE" demeden önce her başarı kriterini tek tek doğrula.

U-21/U-22/U-22.5/U-23 fix'lerini DEĞİŞTİRME:
    schedule_yield split, POST production, UART PMP gate, exception triage,
    register scrub, error code şema, watchdog saturating, pointer leak filter,
    compile_error guard, mcounteren/medeleg/mideleg, deny.toml, WASM feature
    gate, ct_eq CI gate, build.rs drift detection, feature matrix,
    sfence.vma pmp.rs, sipahi_api body, task_hello scaffold, SYS_EXIT/WCET_EXIT,
    isolate_task pub(crate), syscall_ids_match_config proof,
    Makefile KERNEL_RUSTFLAGS, CI env RUSTFLAGS.

§18.7 3-YORUM KURALI — Yeni proof/test (grandfather list dışı):
- // VERIFIES: SNTM-Rx (requirement ID)
- // CALLS: production_fn1, production_fn2 (gerçek prod kodu çağrı)
- // FAILS-IF: hangi hatalı implementasyon test'i kırar (fault model)

  Bu sprint çok sayıda yeni proof + negative test eklenecek → 3-yorum ZORUNLU.

§18.7 SCOPE HONESTY — Test adı = test ettiği davranış:
  Adı iddialı, scope'u dar test YASAK. "test var ama davranış yok"
  antipattern'i §18.7'nin TAM yakalamak istediği şey.

§18.3 FEATURE GATE:
- sntm-validate HOST TOOL — kernel feature ile bağlanmaz
- PmpProfile/Region struct definitions kernel-side (always-compiled) —
  feature gate yok (memory layout core invariant)
- Generated const tables (PMP_PROFILES) U-24'te PLACEHOLDER (boş static)
  → U-25'te manifest'ten gerçek doldurma
```

---

## SORUN

U-23 SNTM Phase 1 ile sipahi_api body + task_hello scaffold tamamlandı.
Ama:

```
1. sipahi.toml manifest sadece SCAFFOLD — kimse parse etmiyor.
2. PmpProfile/Region struct yok — SNTM design v0.5 §4.5.4'te tanımlı
   ama implement edilmedi (U-23 sırasında "v0.7'de PmpProfile struct
   eklendi" diye claim'ledi ama compile-time gap kalmıştı).
3. PMP region invariant'ları (overlap, alignment, budget) compile-time
   doğrulanmıyor — manifest yanlış olsa runtime'da bozulur.
4. sntm-validate tool yok.
5. CI manifest sanity check'i yok.
```

Bu sprint **SNTM Phase 2**: sntm-validate host tool'unu yazıyoruz +
PmpProfile/Region tip definition'larını kernel-side ekliyoruz +
manifest validation invariant'larını Kani proof'larıyla kanıtlıyoruz.

**Bu sprint DA task BOOT ETMEYECEK** — kernel-side PMP profile runtime
reload Phase 3 (Sprint U-25) hedefi. Burada sadece:
- Manifest type definitions (Rust struct'ları)
- Host validator tool (sntm-validate)
- 5 invariant kontrol (uniqueness, alignment, overlap-internal, overlap-kernel, budget)
- Kani proof: pure `region_overlap()` helper (SNTM-R3)
- 3 yeni requirement: SNTM-R3, SNTM-R4, SNTM-R5
- coverage.toml güncelleme
- CI sntm-validate job

**Detay referans:** `SIPAHI_SNTM_DESIGN.md` (v0.8):
- §4.4 Manifest (sipahi.toml v0.2) — TOML schema
- §4.5.1 PMP Packing Algorithm — NAPOT vs TOR, decision tree
- §4.5.2 PMP Priority Invariant — KRİTİK, kernel-task overlap check
- §4.5.4 Type Definitions: PmpProfile + Region + get_pmp_profile (v0.5)
- §12 Aşama 3 — sntm-validate görev listesi
- §18.4/§18.7 Coverage + 3-yorum gate + scope honesty

---

## TEST-FIRST DİSİPLİN — Bu Sprint'te AKTİF

Validator yeni davranış: 5 invariant kontrol. Her invariant için **önce
negative test** (RED) → sonra validator logic (GREEN). U-21/U-22/U-23
disipliniyle aynı.

Cleanup invariant: **make check + cargo kani + run-self-test** baseline
yeşil. Kani 198 → 200 (+2 yeni proof: `region_overlap_symmetric`,
`napot_alignment_correct`). 3. proof (pmp_profile_bounds) eklenmedi —
get_pmp_profile bounds compile-time const, Kani symbolic input için
trivial; kernel self-test `test_pmp_profile_struct_smoke` ile cover.
Sprint sonu hâlâ yeşil.

---

## ⚠️ KRİTİK GÖREV SIRASI (test-first düzeltildi)

```
G0  U-23 regression gate
G1  src/arch/pmp.rs'e PmpEncoding ekle + src/kernel/pmp/ scaffold
G2  Permission + Region + PmpProfile struct definitions (§4.5.4)
    [PMP_PROFILES placeholder EMPTY, get_pmp_profile bounds-only]
G3  TEST-FIRST Kani proofs (RED görmeli):
    region_overlap_symmetric + napot_alignment_correct
    [helper fn'ler henüz YOK — compile RED]
G4  TEST-FIRST kernel self-test (RED görmeli):
    test_regions_overlap_table + test_napot_alignment_table +
    test_pmp_profile_struct_smoke
    [helper fn'ler henüz YOK — compile RED]
G5  Pure helpers implement (RED → GREEN):
    regions_overlap() + valid_napot_alignment() in kernel/pmp/overlap.rs
G6  tools/sntm-validate Cargo + scaffold (host tool, dep:toml/serde/tempfile)
G7  TOML schema types (Manifest, KernelEntry, PlatformEntry, TaskEntry, RegionEntry)
G8  TOML parser + CLI
G9  Validator: 6 invariant kontrol — kernel-task overlap INCLUDED
G10 Generated const tables placeholder (U-25 hedef)
G11 5 integration test (1 valid + 4 reject, tool-side gerçek negative test)
G12 CI sntm-validate yeni job + sntm_sprint_gate.sh E4 (host target)
G13 coverage.toml: SNTM-R3/R4/R5 + sntm body + 3-yorum doğrulama
G14 Final verification + RAPOR (commit + tag HAZIR, manual approval)
```

⚠ **Codex review fix**: G3-G4 helpers'tan ÖNCE — gerçek RED görmek için
test/proof helper'lardan önce yazılır. U-21/U-22/U-23 disipliniyle aynı.

---

## GÖREV 0: U-23 Regression Gate

```bash
git tag --list | grep -E "v1.5"
git status --short  # temiz olmalı

make check
timeout 25s make run-self-test > /tmp/u24_st.log 2>&1 || true
grep -aq "ALL TESTS PASSED" /tmp/u24_st.log
! grep -aq "^NF$" /tmp/u24_st.log
! grep -aq "\[FAIL\]" /tmp/u24_st.log
grep -aq "SYS_EXIT=5 + SYSCALL_COUNT=6 \[OK\]" /tmp/u24_st.log

make build
timeout 8s qemu-system-riscv64 -machine virt -nographic -bios none \
    -m 512M -smp 1 -kernel target/riscv64imac-unknown-none-elf/release/sipahi \
    > /tmp/u24_prod.log 2>&1 || true
! grep -aq "^NF$" /tmp/u24_prod.log

cargo kani  # 198/198 PASS (U-23 baseline)

# task_hello standalone hâlâ derliyor mu:
(cd tasks/task_hello && cargo build --release 2>&1 | tail -3)

bash scripts/sntm_sprint_gate.sh
```

Geçmezse DUR.

---

## GÖREV 1: src/kernel/pmp/mod.rs Scaffold + PmpEncoding (arch) [20 dk]

**Dizinler:** `src/kernel/pmp/` (yeni) + `src/arch/pmp.rs` (mevcut, küçük ekleme)

```bash
mkdir -p src/kernel/pmp
```

⚠ SNTM design v0.8 §4.5.4 line 1014 yazıyor: `use crate::arch::pmp::PmpEncoding`.
PmpEncoding **arch layer**'da olur (HW-level type), kernel layer import eder.
Layered separation: arch = HW interface + encoding types, kernel = abstraction.

### Adım 1 — `PmpEncoding` arch/pmp.rs'e ekle:

```bash
grep -n 'pub fn pack_pmpcfg\|pub const PMP_' src/arch/pmp.rs | head -5
```

`src/arch/pmp.rs` mevcut helper'ların yanına:

```rust
/// PMP encoding türü — NAPOT (1 entry) veya TOR çifti (2 entry).
/// SNTM design v0.5 §4.5.1 packing algorithm. Manifest'ten sntm-validate
/// üretir (Phase 4); kernel build-time const PMP_PROFILES tüketir.
///
/// U-24 SNTM Phase 2: type definition + arch::pmp layer'da kalır,
/// kernel/pmp/profile.rs `use crate::arch::pmp::PmpEncoding` ile import eder.
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PmpEncoding {
    /// NAPOT — tek entry, size power-of-2, base size-aligned
    Napot { addr: usize, size_log2: u8 },
    /// TOR — iki entry (lo=OFF, hi=TOR), NAPOT-uyumsuz layout'lar için
    Tor { lo: usize, hi: usize },
}
```

### Adım 2 — `src/kernel/pmp/mod.rs`:

```rust
//! Per-task PMP profile types + pure validation helpers.
//!
//! U-24 SNTM Phase 2: PmpProfile + Region high-level abstraction.
//! Encoding type arch::pmp::PmpEncoding'de (HW-level).
//! Manifest-driven build-time const tables; runtime integration (context
//! switch reload) Phase 3 (Sprint U-25).

#![allow(dead_code)] // U-24 sırasında bazıları henüz hot path'te kullanılmıyor

pub mod profile;
pub mod overlap;
```

### kernel/mod.rs'a ekleme:

```bash
grep -n 'pub mod\|pub use' src/kernel/mod.rs | head -5
```

`src/kernel/mod.rs`:
```rust
// ... mevcut pub mod'lar ...
pub mod pmp;
```

---

## GÖREV 2: PmpProfile + Region + Permission Struct'ları [30 dk]

**Dosya:** `src/kernel/pmp/profile.rs`

SNTM design v0.5 §4.5.4'teki tip definition'ları (PmpEncoding G1'de
`arch::pmp`'e eklendi, buradan import):

```rust
//! PMP profile types — manifest-generated, build-time const.
//!
//! Layer separation (SNTM design v0.8 §4.5.4):
//!   src/arch/pmp.rs       = RISC-V CSR low-level + PmpEncoding type
//!   src/kernel/pmp/*.rs   = High-level abstraction (PmpProfile, Region) + helpers
//!
//! v1.5 PMP_PROFILES const'u sntm-validate tarafından üretilir (Phase 4).
//! Şu an PLACEHOLDER empty array — Sprint U-25 runtime integration ekler.

use crate::arch::pmp::PmpEncoding;
use crate::common::config::MAX_TASKS;

/// PMP region permissions (RWX bitleri).
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Permission {
    pub r: bool,
    pub w: bool,
    pub x: bool,
}

impl Permission {
    pub const RX:   Self = Self { r: true,  w: false, x: true  };
    pub const R:    Self = Self { r: true,  w: false, x: false };
    pub const RW:   Self = Self { r: true,  w: true,  x: false };
    pub const NONE: Self = Self { r: false, w: false, x: false };  // guard
}

/// Tek region — task'a grant edilen tek PMP entry (NAPOT) veya
/// entry-çifti (TOR).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Region {
    pub base:     usize,
    pub size:     usize,
    pub encoding: PmpEncoding,
    pub perm:     Permission,
}

/// Task'ın tam PMP profili — max 6 region (text/rodata/data/stack/mmio/guard).
/// region_count actual sayı, regions[0..region_count] valid.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PmpProfile {
    pub region_count: u8,
    pub regions:      [Region; 6],
}

impl PmpProfile {
    /// Boş profile (placeholder) — region_count=0.
    pub const EMPTY: Self = Self {
        region_count: 0,
        regions: [Region {
            base: 0, size: 0,
            encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
            perm: Permission::NONE,
        }; 6],
    };

    /// Valid region slice (0..region_count).
    #[inline]
    pub fn active_regions(&self) -> &[Region] {
        let count = (self.region_count as usize).min(6);
        &self.regions[..count]
    }
}

/// Build-time const — Sprint U-24 placeholder, Sprint U-25 sntm-validate generate.
pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] =
    [PmpProfile::EMPTY; MAX_TASKS];

/// Caller task ID'ye göre PMP profile lookup.
///
/// U-24 placeholder: tüm task'lar EMPTY profile. U-25'te runtime reload
/// + Phase 4 manifest-generated tablo aktif olur.
#[inline]
#[must_use = "PMP profile lookup result must be checked"]
pub fn get_pmp_profile(task_id: u8) -> Option<&'static PmpProfile> {
    let idx = task_id as usize;
    if idx >= MAX_TASKS {
        return None;
    }
    Some(&PMP_PROFILES[idx])
}
```

---

## GÖREV 3: TEST-FIRST — Kernel self-tests (RED expected) [40 dk]

**Dosya:** `src/tests/mod.rs`

⚠ **Codex review fix**: Eski prompt "negative test" diyordu ama gerçekte
helper varlığı + basic semantics testleri. **Doğru ayrım**:
- **Kernel self-test** (bu görev): table-driven **basic semantics**,
  helper davranışını GERÇEKTEN test eder (smoke değil).
- **Tool integration test** (G11): GERÇEK negative test (invalid manifest
  reject — duplicate ID, bad NAPOT, overlap, budget).

§18.7 scope honesty: "table" suffix = multiple case-driven test, scope
explicit.

```rust
// VERIFIES: SNTM-R3 (regions_overlap helper — table-driven semantics)
// CALLS:    crate::kernel::pmp::overlap::regions_overlap
// FAILS-IF: Symmetry break (a↔b farklı sonuç), empty region (size=0) için
//           true, overflow ile saturating_add bypass, ya da disjoint
//           region'lar için yanlış true.
fn test_regions_overlap_table() {
    use crate::kernel::pmp::overlap::regions_overlap;
    // (a_base, a_size, b_base, b_size, expected)
    let cases: &[(usize, usize, usize, usize, bool)] = &[
        // Disjoint — overlap yok
        (0x1000, 0x100, 0x2000, 0x100, false),
        (0x1000, 0x100, 0x1100, 0x100, false),  // touch boundary (half-open)
        // Tam çakışma
        (0x1000, 0x100, 0x1000, 0x100, true),
        // Containment
        (0x1000, 0x200, 0x1080, 0x80, true),    // b içinde a
        (0x1080, 0x80, 0x1000, 0x200, true),    // simetri
        // Partial overlap
        (0x1000, 0x200, 0x10F0, 0x200, true),
        (0x10F0, 0x200, 0x1000, 0x200, true),   // simetri
        // Empty region
        (0x1000, 0, 0x1000, 0x100, false),
        (0x1000, 0x100, 0x1000, 0, false),
        (0x1000, 0, 0x1000, 0, false),
        // Edge: boundary touching (not overlap, half-open)
        (0x1000, 0x100, 0x10FF, 0x1, true),    // 0x1100-1 overlaps end
        (0x1000, 0x100, 0x1100, 0x1, false),   // end == start, no overlap
    ];

    let mut all_pass = true;
    for &(ab, asz, bb, bsz, expected) in cases {
        let actual = regions_overlap(ab, asz, bb, bsz);
        let sym    = regions_overlap(bb, bsz, ab, asz);
        if actual != expected || sym != expected {
            all_pass = false;
        }
    }

    test_result(all_pass,
        "[PASS] regions_overlap 12-case table + symmetry [OK]",
        "[FAIL] regions_overlap table mismatch [FAIL]");
}

// VERIFIES: SNTM-R5 (NAPOT alignment — table-driven)
// CALLS:    crate::kernel::pmp::overlap::valid_napot_alignment
// FAILS-IF: Power-of-2 olmayan size kabul, base aligned olmayan kabul,
//           size < 8 kabul, ya da geçerli kombinasyon reject.
fn test_napot_alignment_table() {
    use crate::kernel::pmp::overlap::valid_napot_alignment;
    // (base, size, expected_valid)
    let cases: &[(usize, usize, bool)] = &[
        // Valid: power-of-2 size ≥ 8 + base aligned to size
        (0x80100000, 8,         true),   // minimum size
        (0x80100000, 0x10,      true),   // 16 byte
        (0x80100000, 0x4000,    true),   // 16K
        (0x80100000, 0x10000,   true),   // 64K
        (0x80104000, 0x4000,    true),   // 16K aligned
        // Size < 8
        (0x80100000, 0,         false),
        (0x80100000, 4,         false),
        (0x80100000, 7,         false),
        // Size not power-of-2
        (0x80100000, 6 * 1024,  false),  // 6K
        (0x80100000, 0x3000,    false),  // 12K
        (0x80100000, 0x5000,    false),  // 20K
        // Base not aligned to size
        (0x80100001, 0x4000,    false),  // off-by-1
        (0x80108000, 0x10000,   false),  // 64K wants 0x10000-aligned, has 0x8000
        (0x80104000, 0x10000,   false),  // 64K base 0x4000-aligned
    ];

    let mut all_pass = true;
    for &(base, size, expected) in cases {
        if valid_napot_alignment(base, size) != expected {
            all_pass = false;
        }
    }

    test_result(all_pass,
        "[PASS] valid_napot_alignment 14-case table [OK]",
        "[FAIL] valid_napot_alignment table mismatch [FAIL]");
}

// VERIFIES: SNTM-R4 (PmpProfile struct + EMPTY const + get_pmp_profile bounds)
// CALLS:    crate::kernel::pmp::profile::{PmpProfile, get_pmp_profile, PMP_PROFILES}
// FAILS-IF: get_pmp_profile out-of-bounds Some döner, EMPTY.region_count != 0,
//           ya da active_regions slice yanlış boyut.
fn test_pmp_profile_struct_smoke() {
    use crate::kernel::pmp::profile::{get_pmp_profile, PmpProfile};
    use crate::common::config::MAX_TASKS;

    // Bounds — all valid IDs return Some
    let mut all_bounds = true;
    let mut i = 0u8;
    while (i as usize) < MAX_TASKS {
        if get_pmp_profile(i).is_none() {
            all_bounds = false;
        }
        i = i.wrapping_add(1);
    }
    // Out-of-bounds → None
    let oob_8  = get_pmp_profile(MAX_TASKS as u8).is_none();
    let oob_ff = get_pmp_profile(0xFF).is_none();

    // EMPTY semantics
    let empty = PmpProfile::EMPTY;
    let count_zero  = empty.region_count == 0;
    let active_zero = empty.active_regions().is_empty();

    let pass = all_bounds && oob_8 && oob_ff && count_zero && active_zero;
    test_result(pass,
        "[PASS] PmpProfile bounds + EMPTY + active_regions [OK]",
        "[FAIL] PmpProfile struct broken [FAIL]");
}
```

`run_all()` içine ekle:

```rust
arch::uart::println("");
arch::uart::println("[TEST] U-24 SNTM Phase 2 — table-driven semantics:");
test_regions_overlap_table();
test_napot_alignment_table();
test_pmp_profile_struct_smoke();
```

### RED doğrulama (G5 ÖNCESİ):

```bash
cargo check --features self-test 2>&1 | tail -5
# Beklenen: "unresolved import crate::kernel::pmp::overlap" ya da
# "cannot find function regions_overlap" — helper yok, RED.
```

---

## GÖREV 4: TEST-FIRST — Kani Proofs (RED expected) [25 dk]

**Dosya:** `src/verify.rs`

⚠ **G5'ten ÖNCE yaz** — helper'lar henüz yok, compile RED görmeli.

```rust
    // ═══════════════════════════════════════════════════════
    // PROOF (U-24 SNTM-R3): Region overlap symmetric + defensive
    //
    // VERIFIES: SNTM-R3 (regions_overlap helper symmetric + saturating_add safe)
    // CALLS:    crate::kernel::pmp::overlap::regions_overlap
    // FAILS-IF: regions_overlap asymmetric (a,b ≠ b,a), veya overflow ile
    //           overlap'ı false negative (saturating_add eksik), veya boş
    //           region (size=0) için overlap true (must be false).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn region_overlap_symmetric() {
        use crate::kernel::pmp::overlap::regions_overlap;
        let a_base: usize = kani::any();
        let a_size: usize = kani::any();
        let b_base: usize = kani::any();
        let b_size: usize = kani::any();
        kani::assume(a_size <= 0x10000);  // bounded for Kani performance
        kani::assume(b_size <= 0x10000);

        // Symmetry: aynı pair iki sırada da aynı sonuç
        let ab = regions_overlap(a_base, a_size, b_base, b_size);
        let ba = regions_overlap(b_base, b_size, a_base, a_size);
        assert_eq!(ab, ba);

        // Empty region: size=0 → asla overlap
        let zero = regions_overlap(a_base, 0, b_base, b_size);
        assert!(!zero);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-24 SNTM-R5): NAPOT alignment correctness
    //
    // VERIFIES: SNTM-R5 (valid_napot_alignment: size power-of-2 ≥ 8 AND base aligned)
    // CALLS:    crate::kernel::pmp::overlap::valid_napot_alignment
    // FAILS-IF: Power-of-2 olmayan size kabul edilirse, veya base aligned değilse
    //           kabul edilirse, veya size < 8 kabul edilirse.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn napot_alignment_correct() {
        use crate::kernel::pmp::overlap::valid_napot_alignment;

        // Symbolic input — Kani tüm size kombinasyonlarını dener (bounded)
        let base: usize = kani::any();
        let size: usize = kani::any();
        kani::assume(size <= 0x100000);  // bounded

        let result = valid_napot_alignment(base, size);

        if result {
            // Valid ise tüm 3 koşul sağlanmalı
            assert!(size >= 8);
            assert!(size & (size - 1) == 0);  // power-of-2
            assert!(base & (size - 1) == 0);  // aligned
        }
        // Invalid case'leri reciprocal assertion (counterexample search):
        if size < 8 || size & (size - 1) != 0 {
            assert!(!result);
        }
    }
```

### RED doğrulama (G5 ÖNCESİ):

```bash
cargo kani --harness region_overlap_symmetric 2>&1 | tail -5
# Beklenen: cannot find module 'overlap' — helper yok, RED.
```

---

## GÖREV 5: Pure Helpers Implement (RED → GREEN) [25 dk]

**Dosya:** `src/kernel/pmp/overlap.rs`

G3 (kernel self-tests) + G4 (Kani proofs) RED görüldü. Şimdi helper'lar
yazılır, ikisi de GREEN olur.

```rust
//! Pure region overlap + NAPOT alignment helpers.
//!
//! U-24 SNTM Phase 2: build-time + runtime invariant kontrolleri.
//! Kani proof'lar (verify.rs SNTM-R3, SNTM-R5) bu helper'ları doğrular
//! (symbolic input). Kernel self-test'ler (tests/mod.rs) table-driven
//! basic semantics test eder. sntm-validate tool (G6+) DUPLICATE pure
//! fn implement eder (no_std vs std crossing, U-24 pragmatik karar).

/// İki region adres aralığı kesişiyor mu (half-open [base, base+size)).
///
/// SAFETY/CORRECTNESS:
///   - saturating_add overflow yok (cosmic ray + bug-late-injection defansif)
///   - Symmetric: regions_overlap(a, b) == regions_overlap(b, a) (SNTM-R3 invariant)
///   - Empty region (size=0): asla overlap (false)
#[inline]
#[must_use]
pub const fn regions_overlap(
    a_base: usize, a_size: usize,
    b_base: usize, b_size: usize,
) -> bool {
    if a_size == 0 || b_size == 0 {
        return false;
    }
    let a_end = a_base.saturating_add(a_size);
    let b_end = b_base.saturating_add(b_size);
    !(a_end <= b_base || b_end <= a_base)
}

/// NAPOT-uyumlu mu: size power-of-2 ≥ 8 byte VE base aligned to size.
/// SNTM design v0.5 §4.5.1 NAPOT decision tree.
#[inline]
#[must_use]
pub const fn valid_napot_alignment(base: usize, size: usize) -> bool {
    if size < 8 {
        return false;
    }
    // power-of-2 check: size & (size-1) == 0
    if size & (size - 1) != 0 {
        return false;
    }
    // base aligned to size
    base & (size - 1) == 0
}
```

### GREEN doğrulama:

```bash
make check          # 0 warning
cargo kani --harness region_overlap_symmetric    # PASS
cargo kani --harness napot_alignment_correct     # PASS
timeout 25s make run-self-test 2>&1 | grep -E "regions_overlap.*\[OK\]|napot_alignment.*\[OK\]"
# Beklenen: 2 satır [OK]
```

---

## GÖREV 6: tools/sntm-validate Cargo + Scaffold [25 dk]

⚠ **HOST tool** — workspace member, std + alloc kullanır.

```bash
mkdir -p tools/sntm-validate/src
```

### Dosya: `tools/sntm-validate/Cargo.toml`

```toml
[package]
name = "sntm-validate"
version = "0.1.0"
edition = "2021"
description = "Sipahi SNTM manifest validator (host tool)"
license = "Apache-2.0"

[[bin]]
name = "sntm-validate"
path = "src/main.rs"

[dependencies]
# U-24 NEW DEP: TOML parser — sadece HOST tool, kernel'e EKLENMEZ.
# Exact pin (U-22 G11 doctrine).
toml = "=0.8.19"
serde = { version = "=1.0.219", features = ["derive"] }
```

### Workspace Cargo.toml güncelle:

```bash
grep -n '^members' Cargo.toml
```

```toml
[workspace]
members = [
    ".",
    "sipahi_api",
    "tasks/task_hello",
    "tools/sntm-validate",  # U-24: SNTM Phase 2 host validator
]
resolver = "2"
```

⚠ tools/sntm-validate workspace member ama **kernel target'tan ayrı** —
host (x86_64) target'a derler. Kernel build'ini etkilemez.

---

## GÖREV 7: TOML Schema Types [30 dk]

**Dosya:** `tools/sntm-validate/src/manifest.rs`

```rust
//! sipahi.toml deserialization types.
//!
//! Manifest schema SNTM design v0.8 §4.4'e uygun.

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub kernel:   KernelEntry,
    pub platform: PlatformEntry,
    #[serde(default, rename = "task")]
    pub tasks:    Vec<TaskEntry>,
}

#[derive(Deserialize, Debug)]
pub struct KernelEntry {
    pub name:       String,
    pub version:    String,
    pub binary:     String,
    pub stack_size: usize,
}

#[derive(Deserialize, Debug)]
pub struct PlatformEntry {
    pub target:      String,
    pub machine:     String,
    pub pmp_entries: u8,
    pub ram_base:    usize,
    pub ram_size:    usize,
}

#[derive(Deserialize, Debug)]
pub struct TaskEntry {
    pub name:          String,
    pub binary:        String,
    pub task_id:       u8,
    pub priority:      u8,
    pub period_ticks:  u32,
    pub budget_cycles: u32,
    pub dal_level:     String,
    #[serde(default, rename = "region")]
    pub regions:       Vec<RegionEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegionEntry {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub perm: String,  // "RX", "R", "RW", "NONE"
}
```

---

## GÖREV 8: TOML Parser [15 dk]

**Dosya:** `tools/sntm-validate/src/main.rs` (scaffold için)

```rust
//! Sipahi SNTM Manifest Validator — host tool.
//!
//! Usage: sntm-validate --manifest sipahi.toml [--output-rs PATH]
//!
//! Validates 5 invariants (SNTM-R3, R4, R5 + uniqueness + budget):
//!   1. Task ID uniqueness
//!   2. NAPOT alignment (or TOR fallback)
//!   3. Region overlap (intra-task)
//!   4. Region overlap (cross-task)
//!   5. Region overlap (kernel-task)
//!   6. PMP entry budget (kernel 6 + per-task ≤ platform pmp_entries)

mod manifest;
mod validate;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut manifest_path: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                manifest_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            other => {
                eprintln!("Unknown arg: {}", other);
                return ExitCode::from(2);
            }
        }
    }

    let path = match manifest_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: sntm-validate --manifest sipahi.toml");
            return ExitCode::from(2);
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAIL: cannot read {}: {}", path.display(), e);
            return ExitCode::from(1);
        }
    };

    let m: manifest::Manifest = match toml::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL: TOML parse error: {}", e);
            return ExitCode::from(1);
        }
    };

    match validate::validate_all(&m) {
        Ok(()) => {
            println!("PASS: manifest valid ({} tasks, {} regions)",
                m.tasks.len(),
                m.tasks.iter().map(|t| t.regions.len()).sum::<usize>());
            ExitCode::from(0)
        }
        Err(errs) => {
            for e in &errs {
                eprintln!("FAIL: {}", e);
            }
            ExitCode::from(1)
        }
    }
}
```

---

## GÖREV 9: Validator — 6 Invariant Kontrol (RED → GREEN) [70 dk]

**Dosya:** `tools/sntm-validate/src/validate.rs`

⚠ Bu görev G4-G5 RED'i GREEN yapar.

```rust
//! Manifest invariant validators.
//!
//! Pure logic — kernel/pmp/overlap.rs helper'larını çağırmak istesek
//! sipahi crate dependency gerekir; bu HOST tool için sürdürülebilir
//! değil. Duplicate logic kabul: kernel + tool aynı pure fn'i implement
//! eder. SNTM-R3/R5 Kani proof'ları kernel tarafını kanıtlar; sntm-validate
//! integration test'i tool tarafını kanıtlar (G11).

use crate::manifest::{Manifest, RegionEntry, TaskEntry};

const KERNEL_PMP_ENTRIES: u8 = 6;  // SNTM design v0.5 §4.5.1 static budget
const MAX_REGIONS_PER_TASK: usize = 6;

// Kernel address range (Sipahi v1.5 sabit layout — sipahi.ld'den).
// Kernel image 0x80000000..0x80100000 (1MB), task'lar 0x80100000+.
// U-25'te dinamik kernel.size manifest'ten okunacak.
const KERNEL_BASE: usize = 0x8000_0000;
const KERNEL_SIZE: usize = 0x10_0000;  // 1MB kernel image (rough upper bound)

pub fn validate_all(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Err(es) = check_task_id_uniqueness(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_napot_alignment(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_intra_task_overlap(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_cross_task_overlap(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_kernel_task_overlap(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_pmp_budget(m) {
        errors.extend(es);
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// SNTM-R3 (kernel half): task region kernel address range ile çakışmamalı.
/// Critical: PMP priority + kernel-task overlap = izolasyon ihlali
/// (SNTM design v0.8 §4.5.2 shadow attack scenario).
fn check_kernel_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        for r in &t.regions {
            if regions_overlap(r.base, r.size, KERNEL_BASE, KERNEL_SIZE) {
                errs.push(format!(
                    "task '{}' region '{}' (base=0x{:x} size=0x{:x}) \
                     overlaps kernel range [0x{:x}..0x{:x}+0x{:x})",
                    t.name, r.name, r.base, r.size,
                    KERNEL_BASE, KERNEL_BASE, KERNEL_SIZE
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_task_id_uniqueness(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut seen = std::collections::HashMap::new();
    let mut errs = Vec::new();
    for t in tasks {
        if let Some(prev) = seen.insert(t.task_id, &t.name) {
            errs.push(format!(
                "task_id={} duplicate: '{}' and '{}'",
                t.task_id, prev, t.name
            ));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_napot_alignment(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        if t.regions.len() > MAX_REGIONS_PER_TASK {
            errs.push(format!(
                "task '{}': {} regions > MAX_REGIONS_PER_TASK ({})",
                t.name, t.regions.len(), MAX_REGIONS_PER_TASK
            ));
        }
        for r in &t.regions {
            if !valid_napot_alignment(r.base, r.size) {
                errs.push(format!(
                    "task '{}' region '{}': base=0x{:x} size=0x{:x} \
                     not NAPOT-aligned (size power-of-2 + base aligned)",
                    t.name, r.name, r.base, r.size
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_intra_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        for i in 0..t.regions.len() {
            for j in (i + 1)..t.regions.len() {
                let a = &t.regions[i];
                let b = &t.regions[j];
                if regions_overlap(a.base, a.size, b.base, b.size) {
                    errs.push(format!(
                        "task '{}': region '{}' overlaps '{}' \
                         (a=0x{:x}+0x{:x}, b=0x{:x}+0x{:x})",
                        t.name, a.name, b.name,
                        a.base, a.size, b.base, b.size
                    ));
                }
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_cross_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for i in 0..tasks.len() {
        for j in (i + 1)..tasks.len() {
            let ta = &tasks[i];
            let tb = &tasks[j];
            for ra in &ta.regions {
                for rb in &tb.regions {
                    if regions_overlap(ra.base, ra.size, rb.base, rb.size) {
                        errs.push(format!(
                            "task '{}' region '{}' overlaps task '{}' region '{}' \
                             (a=0x{:x}+0x{:x}, b=0x{:x}+0x{:x})",
                            ta.name, ra.name, tb.name, rb.name,
                            ra.base, ra.size, rb.base, rb.size
                        ));
                    }
                }
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_pmp_budget(m: &Manifest) -> Result<(), Vec<String>> {
    // Worst case: tüm region NAPOT (1 entry per region).
    // Per-task max region sayısı budget'ı belirler.
    let max_per_task = m.tasks
        .iter()
        .map(|t| t.regions.len())
        .max()
        .unwrap_or(0);

    let required = KERNEL_PMP_ENTRIES as usize + max_per_task;
    let available = m.platform.pmp_entries as usize;

    if required > available {
        return Err(vec![format!(
            "PMP budget exceeded: kernel({}) + max_per_task({}) = {} > platform.pmp_entries({})",
            KERNEL_PMP_ENTRIES, max_per_task, required, available
        )]);
    }
    Ok(())
}

// ─── Pure helpers (kernel/pmp/overlap.rs ile duplicate, host-side) ──

#[inline]
fn regions_overlap(a_base: usize, a_size: usize, b_base: usize, b_size: usize) -> bool {
    if a_size == 0 || b_size == 0 {
        return false;
    }
    let a_end = a_base.saturating_add(a_size);
    let b_end = b_base.saturating_add(b_size);
    !(a_end <= b_base || b_end <= a_base)
}

#[inline]
fn valid_napot_alignment(base: usize, size: usize) -> bool {
    if size < 8 {
        return false;
    }
    if size & (size - 1) != 0 {
        return false;
    }
    base & (size - 1) == 0
}
```

⚠ **Duplicate logic kabul edildi**: kernel-side `kernel/pmp/overlap.rs`
ve tool-side `validate.rs` aynı pure fn'leri içerir. Kernel Kani-proven,
tool integration-test'lenmiştir (G11). v2.0'da `sntm-validate` `sipahi`
crate'ini `dev-dependency` olarak alabilir — şimdi pragmatik.

---

## GÖREV 10: Generated Const Tables Placeholder [10 dk]

**Dosya:** `tools/sntm-validate/src/main.rs` (G8'in devamı)

Validator PASS sonrası placeholder output:

```rust
// main.rs içinde, validate_all PASS sonrası:
//
// Placeholder: U-24'te sadece "PASS" yazıyoruz. U-25'te:
//   --output-rs src/kernel/pmp/generated.rs
// flag ile PMP_PROFILES const'unu üretiriz. Şimdilik validation-only.

if let Ok(()) = validate::validate_all(&m) {
    println!("PASS: manifest valid");
    println!("PLACEHOLDER: generated const tables (PMP_PROFILES) — Sprint U-25 hedefi");
}
```

---

## GÖREV 11: CLI Integration Test [25 dk]

**Dosya:** `tools/sntm-validate/tests/integration.rs`

```rust
//! sntm-validate integration tests — fault injection scenarios.
//!
//! Her invariant için 1+ negative case + positive case.
//! VERIFIES: SNTM-R3/R4/R5 tool-side coverage.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_sntm-validate");

fn run(toml_content: &str) -> (i32, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sipahi.toml");
    std::fs::write(&path, toml_content).unwrap();
    let out = Command::new(BIN)
        .arg("--manifest")
        .arg(&path)
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    (out.status.code().unwrap_or(-1), combined)
}

#[test]
fn valid_manifest_passes() {
    let toml = r#"
        [kernel]
        name = "sipahi" version = "1.5.0" binary = "x" stack_size = 16384
        [platform]
        target = "riscv64imac-unknown-none-elf" machine = "qemu-virt"
        pmp_entries = 16 ram_base = 0x80000000 ram_size = 0x20000000
    "#;
    let (code, _) = run(toml);
    assert_eq!(code, 0);
}

#[test]
fn duplicate_task_id_rejected() {
    let toml = r#"
        [kernel]
        name = "sipahi" version = "1.5.0" binary = "x" stack_size = 16384
        [platform]
        target = "x" machine = "x" pmp_entries = 16 ram_base = 0 ram_size = 0
        [[task]] name = "a" binary = "" task_id = 0 priority = 1
          period_ticks = 1 budget_cycles = 1 dal_level = "D"
        [[task]] name = "b" binary = "" task_id = 0 priority = 1
          period_ticks = 1 budget_cycles = 1 dal_level = "D"
    "#;
    let (code, out) = run(toml);
    assert_ne!(code, 0);
    assert!(out.contains("duplicate"));
}

#[test]
fn napot_alignment_violation_rejected() { /* ... 6K size case ... */ }

#[test]
fn intra_task_overlap_rejected() { /* ... 2 overlapping regions ... */ }

#[test]
fn pmp_budget_exceeded_rejected() { /* ... 10 region task, pmp_entries=8 ... */ }
```

⚠ Bu test'ler için `tempfile` dev-dependency ekle:

```toml
[dev-dependencies]
tempfile = "=3.12.0"
```

### Test çalıştır:

```bash
cd tools/sntm-validate
cargo test 2>&1 | tail -10
# Beklenen: 5 PASS
```

---

## GÖREV 12: CI Job + sntm_sprint_gate.sh E4 [15 dk]

**Dosya:** `.github/workflows/ci.yml`

Yeni job ekle:

⚠ **Codex review fix**: Root `.cargo/config.toml` `[build] target =
"riscv64imac-unknown-none-elf"` host tool'u da RISC-V'e derler.
HOST_TARGET override explicit gerekli.

```yaml
  sntm-validate:
    name: SNTM Manifest Validator
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust + RISC-V target
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly-2026-03-01
          components: rust-src
      - name: Build sntm-validate (host tool — explicit host target)
        run: |
          HOST=$(rustc -vV | sed -n 's/^host: //p')
          cargo build -p sntm-validate --target "$HOST" --release
      - name: Run integration tests (host target)
        run: |
          HOST=$(rustc -vV | sed -n 's/^host: //p')
          cargo test -p sntm-validate --target "$HOST" --release
      - name: Validate sipahi.toml (host target)
        run: |
          HOST=$(rustc -vV | sed -n 's/^host: //p')
          cargo run -p sntm-validate --target "$HOST" --release -- \
            --manifest sipahi.toml
```

**Dosya:** `scripts/sntm_sprint_gate.sh`

E4 step'i güncelle — şu an SKIP, gerçek çalıştır + HOST target:

```bash
# ─── E4: sntm-validate manifest check (host target explicit) ───
if [ -f "sipahi.toml" ] && [ -d "tools/sntm-validate" ]; then
    HOST=$(rustc -vV | sed -n 's/^host: //p')
    echo "[E4] sntm-validate --manifest sipahi.toml (target=$HOST)"
    if cargo run -p sntm-validate --target "$HOST" --release -- \
        --manifest sipahi.toml > /tmp/u24_validate.log 2>&1; then
        echo "  PASS: $(cat /tmp/u24_validate.log | tail -1)"
    else
        echo "  FAIL: sntm-validate"
        cat /tmp/u24_validate.log
        exit 1
    fi
else
    echo "[E4] SKIP: sntm-validate tool veya sipahi.toml yok"
fi
```

---

## GÖREV 13: coverage.toml + SNTM-R3/R4/R5 Requirements [20 dk]

**Dosya:** `coverage.toml`

### Adım 1 — `[feature.sntm]` body güncelle:

```toml
[feature.sntm]
description             = "SNTM base — sipahi_api body + task_hello + manifest validator (v1.5+)"
required_negative_tests = [
    "test_sys_exit_id_registered",
    "test_regions_overlap_table",
    "test_napot_alignment_table",
    "test_pmp_profile_struct_smoke",
]
required_kani_proofs = [
    "syscall_id_set_complete",
    "region_overlap_symmetric",
    "napot_alignment_correct",
]
```

### Adım 2 — Yeni requirement blokları:

```toml
[requirement.SNTM-R3]
description     = "Region overlap symmetric + saturating safe + empty region (size=0) no overlap"
required_tests  = ["test_regions_overlap_table"]
required_proofs = ["region_overlap_symmetric"]
fault_model     = "regions_overlap asymmetric, overflow ile false negative, veya boş region (size=0) için true (must be false)"

[requirement.SNTM-R4]
description     = "PmpProfile bounds + MAX_TASKS guard + EMPTY semantics"
required_tests  = ["test_pmp_profile_struct_smoke"]
required_proofs = []
fault_model     = "get_pmp_profile(MAX_TASKS) None döndürmüyor, ya da PmpProfile::EMPTY region_count != 0"

[requirement.SNTM-R5]
description     = "NAPOT alignment: size power-of-2 ≥ 8 AND base aligned to size"
required_tests  = ["test_napot_alignment_table"]
required_proofs = ["napot_alignment_correct"]
fault_model     = "Power-of-2 olmayan size kabul, ya da unaligned base kabul"
```

### Adım 3 — Validation:

```bash
bash scripts/check_coverage.sh
# Beklenen: 14 feature, 5 requirement ID (SNTM-R1, R2-id, R3, R4, R5)
```

⚠ G4 proof'ları + G5 test'lerin source'da `// VERIFIES: SNTM-R*` yorumu
zorunlu (3-yorum kuralı). Yoksa coverage gate FAIL.

---

## GÖREV 14: Final Verification + RAPOR

```bash
make check
make build
timeout 8s qemu-system-riscv64 -machine virt -nographic -bios none \
    -m 512M -smp 1 -kernel target/riscv64imac-unknown-none-elf/release/sipahi \
    > /tmp/u24_prod.log 2>&1 || true
! grep -aq "^NF$" /tmp/u24_prod.log

timeout 25s make run-self-test > /tmp/u24_st.log 2>&1 || true
grep -aq "ALL TESTS PASSED" /tmp/u24_st.log
! grep -aq "\[FAIL\]" /tmp/u24_st.log
grep -aq "regions_overlap 12-case table.*\[OK\]" /tmp/u24_st.log
grep -aq "valid_napot_alignment 14-case table \[OK\]" /tmp/u24_st.log
grep -aq "PmpProfile bounds + EMPTY + active_regions \[OK\]" /tmp/u24_st.log

cargo kani
# 200/200 PASS (198 + 2: region_overlap_symmetric, napot_alignment_correct)

# sntm-validate self-test (HOST target explicit):
HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')
cargo build -p sntm-validate --target "$HOST_TARGET" --release
cargo test  -p sntm-validate --target "$HOST_TARGET" --release

# Real manifest validation:
cargo run -p sntm-validate --target "$HOST_TARGET" --release -- \
    --manifest sipahi.toml
# Beklenen: "PASS: manifest valid (1 tasks, 4 regions)"

# task_hello hâlâ derliyor mu:
(cd tasks/task_hello && cargo build --release 2>&1 | tail -3)

# Coverage + gates:
bash scripts/check_coverage.sh
# 14 feature, 5 requirement (SNTM-R1/R2-id/R3/R4/R5)

bash scripts/check_proof_quality.sh
# 201 proof, 0 tautoloji

bash scripts/sntm_sprint_gate.sh
# E0/E0b + baseline + E1 + E4 (sntm-validate) PASS

bash scripts/feature_matrix.sh
# 10/10 (sntm kombinasyonu hâlâ pass)
```

### ⚠ COMMIT + TAG — Sprint owner manual approval

```bash
# === USER APPROVAL REQUIRED — DO NOT RUN AUTONOMOUSLY ===
#
# git status
# git add -A
# git commit -m "sprint-u24: SNTM Phase 2 — manifest validator + PmpProfile types
#
# Added (SNTM v1.5 Phase 2):
# - src/kernel/pmp/{profile,overlap}.rs: PmpProfile + Region + Permission +
#   PmpEncoding struct'ları, pure helpers (regions_overlap, valid_napot_alignment)
# - PMP_PROFILES placeholder static + get_pmp_profile() (U-25 runtime integration)
# - tools/sntm-validate host tool: 5 invariant kontrol (uniqueness, NAPOT,
#   intra-overlap, cross-overlap, PMP budget), CLI, integration test'ler
# - Workspace members: kernel + sipahi_api + tasks/task_hello + tools/sntm-validate
# - CI yeni job: sntm-validate (build + test + sipahi.toml validation)
#
# Verification (§18.7 + §18.4):
# - Kani: 198 → 200 (+region_overlap_symmetric, +napot_alignment_correct, SNTM-R3/R5)
# - 3 new tests: test_regions_overlap_table, test_napot_alignment_table,
#   test_pmp_profile_struct_smoke (3-yorum compliant)
# - Test-first: G4-G5 RED görüldü (kernel/pmp/ yoktu) → G6-G9 GREEN yaptı
# - coverage.toml: 14 feature, 5 requirement (R1, R2-id, R3, R4, R5)
# - sntm-validate integration: 5 fault scenario reject + 1 valid pass
# - Production binary unchanged (sntm default-off, kernel sipahi_api-free,
#   PMP_PROFILES placeholder EMPTY)
#
# Tag: git tag -a v1.5.0-alpha2 ... (sprint owner manuel)"
```

---

## DOĞRULAMA

```bash
echo "=== U-24 METRICS ==="

echo "Kani proof count:"
grep -rn "#\[kani::proof\]" src/ | wc -l
# Beklenen: 201

echo "Yeni proof'lar:"
grep -rn "fn region_overlap_symmetric\|fn napot_alignment_correct" src/

echo "Yeni test'ler:"
grep -rn "fn test_regions_overlap_table\|fn test_napot_alignment_table\|fn test_pmp_profile_struct_smoke" src/

echo "3-yorum varlığı:"
for n in region_overlap_symmetric napot_alignment_correct \
         test_regions_overlap_table test_napot_alignment_table \
         test_pmp_profile_struct_smoke; do
    cnt=$(grep -B 6 "fn $n" src/ -r | grep -cE "VERIFIES|CALLS|FAILS-IF")
    echo "  $n: $cnt/3"
done
# Beklenen: hepsi 3/3

echo "kernel/pmp/ struct definitions:"
test -f src/kernel/pmp/profile.rs && echo "profile.rs OK"
test -f src/kernel/pmp/overlap.rs && echo "overlap.rs OK"

echo "tools/sntm-validate:"
test -d tools/sntm-validate && echo "OK"
test -f tools/sntm-validate/Cargo.toml && echo "Cargo OK"

echo "Workspace check:"
cargo check --workspace --quiet
# Beklenen: 0 hata (kernel + sipahi_api + task_hello + sntm-validate)

echo "Manifest validation (HOST target):"
HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')
cargo run -p sntm-validate --target "$HOST_TARGET" --release -- \
    --manifest sipahi.toml
# Beklenen: PASS

echo "Coverage:"
bash scripts/check_coverage.sh 2>&1 | tail -3

echo "=== ALL METRICS PASS ==="
```

---

## RAPOR FORMAT (SNTM v0.7 §18.5)

```markdown
## Sprint U-24 — Final Report

### Completed (with evidence)
- G0:  U-23 regression gate — baseline solid, v1.5.0-alpha1 tag mevcut
- G1:  src/kernel/pmp/ scaffold (mod.rs + 2 submodule)
- G2:  Permission + Region + PmpEncoding + PmpProfile + PMP_PROFILES +
       get_pmp_profile (src/kernel/pmp/profile.rs:XX-YY)
- G3:  regions_overlap + valid_napot_alignment pure helpers
       (src/kernel/pmp/overlap.rs)
- G4:  TEST-FIRST — region_overlap_symmetric + napot_alignment_correct
       Kani proof'ları yazıldı. G3 öncesi compile RED gözlendi.
- G3:  TEST-FIRST — 3 kernel self-test (table-driven semantics, scope-honest):
       test_regions_overlap_table (12 case + symmetry),
       test_napot_alignment_table (14 case), test_pmp_profile_struct_smoke.
       Gerçek negative test'ler tool-side: G11 integration tests.
       G2-G3 öncesi compile RED gözlendi.
- G6:  tools/sntm-validate Cargo + scaffold (host workspace member)
- G7:  TOML schema types (Manifest, KernelEntry, PlatformEntry, TaskEntry, RegionEntry)
- G8:  TOML parser (serde + toml crate)
- G9:  5 invariant validator (RED → GREEN; G4-G5 testleri ve integration
       tests'i GREEN yaptı)
- G10: PLACEHOLDER output (U-25'te generated const tables)
- G11: 5 integration test (1 valid + 4 fault injection)
- G12: CI sntm-validate yeni job + sntm_sprint_gate.sh E4 aktif
- G13: coverage.toml — SNTM-R3/R4/R5 requirements + sntm body 6 entry
       (3 test + 3 proof), 3-yorum compliant
- G14: Final verify PASS; commit + tag commands HAZIR

### Test sonuçları
- Self-test: ALL TESTS PASSED + 3 yeni helper test [OK]
- Kani: 198 → 200 (+region_overlap_symmetric, +napot_alignment_correct)
- sntm-validate integration: 5 test PASS (1 valid + 4 reject)
- Coverage: 14 feature mapped, 5 requirement ID
- Quality: 0 tautoloji
- Feature matrix: 10/10
- sntm_sprint_gate.sh: PASS (E1 + E4 PASS, diğerleri graceful SKIP)
- Production binary: unchanged

### Scope honesty (Codex §18.7)
- test_*_helper_exists: SADECE helper varlığı + basic semantics
- region_overlap_symmetric/napot_alignment_correct: pure fn'lerin Kani-proven
- SNTM-R3/R5 tam coverage: kernel proofs + tool integration tests beraber
- SNTM-R4 partial: get_pmp_profile bounds + EMPTY check (runtime aktif kullanım U-25)
- PMP_PROFILES placeholder EMPTY — gerçek manifest generation U-25 hedefi

### No-Go check (§18.6 zorunlu)
□ Production NF-free
□ Self-test PASS, [FAIL] yok
□ Kani ≥ baseline (201 ≥ 198)
□ sntm feature default-off
□ Manifest validator çalışıyor + invalid manifest reject ediyor
□ Bin verifier yok (Phase 3 SAFE-3 hedef, N/A)

### Carry-forward (U-25 hedefi)
- src/kernel/pmp/generated.rs: PMP_PROFILES sntm-validate-üretilen tablo
- scheduler/mod.rs: context switch'te per-task PMP profile reload
  (mevcut tek-NAPOT-entry-8 yerine multi-region)
- is_valid_user_ptr: multi-region (SNTM design §5.2)
- Kani proof: pmp_profile_collision_detection (SNTM-R6 yeni)
- TLA+ spec: SipahiSNTM.tla task lifecycle

### Commit + tag — user approval bekliyor
```

---

## YAPMA

- WASM feature gate mantığını DEĞİŞTİRME
- sandbox/* dokunma
- pmp.rs (arch low-level) içine yeni şey ekleme — TEK İSTİSNA: G1'de
  PmpEncoding enum ekleme (SNTM design §4.5.4 use crate::arch::pmp::PmpEncoding).
  Başka low-level PMP logic (write_per_task_napot, sfence.vma, vb.) DEĞİŞMEZ.
- U-21/U-22/U-22.5/U-23 fix'leri değiştirme
- Mevcut Kani proof'ları silme (sadece +2 ekleme)
- task_hello'yu kernel image'a embed etmeye çalışma (Phase 3 hedef)
- Kernel loader stub eklemeye çalışma (Phase 3 hedef)
- Runtime PMP profile reload implement etmeye çalışma (Phase 3 hedef — U-25)
- is_valid_user_ptr multi-region update (U-25 hedef)
- TLA+ SipahiSNTM.tla yazmaya çalışma (U-25 hedef)
- Kernel Cargo.toml [dependencies]'ine toml/serde ekleme (HOST tool only)
- §18.7 3-yorum kuralını yeni proof/test'te atlama
- §18.7 scope honesty ihlal (test adı = davranış)
- Test-first disiplini bozma (G4-G5 ÖNCE G6-G9'dan)
- coverage.toml grandfather list'e yeni isim ekleme
- sntm feature default'a ekleme (default-off zorunlu)
- API uydurma — sembol değişikliği öncesi grep -rn doğrula
- PmpProfile struct alanlarını değiştirme (SNTM v0.5 §4.5.4 ile birebir uyumlu)
- regions_overlap signature değiştirme (kernel + tool ortak imza)
- `git commit` autonomous çalıştırma — no-auto-commit doktrini
- `git tag` autonomous çalıştırma — user manuel commit sonrası
