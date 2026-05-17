# Sprint U-29 — WASM removal + ed25519-compact migration (v2.0 closure)

**Hedef:** v2.0 final tag. WASM tüm artefaktları sil + ed25519-dalek → ed25519-compact migration → kernel pure `no_std + no_alloc` doctrine. Süre tahmini: **~1 gün**. SNTM design §11.5/§11.6/§11.7 + §12 Aşama 6 ile birebir hizalı.

**Önkoşul:** U-27.5 commit + tag (v1.5.1) tamamlanmış. Working tree clean. Kani 213, TLA 8/8, smoke clean, coverage 15F/14R, cross-isolation gate 4/4 PASS.

**Stratejik konum:** v1.5 SNTM Phase 1+2+3+4+5 + U-27.5 runtime observation tamamlandı. U-29 **v1.5 → v2.0 cleanup** sprint'i; yeni feature YOK, kernel surface küçültme + alloc bağımlılığından kurtuluş + WASM-tied legacy adacık temizliği.

---

## 1. Scope

### IN scope
1. **ed25519-dalek → ed25519-compact migration** (real exact-pin Cargo.toml; placeholder kullanılmaz; G1'de `cargo search` ile latest stable bulunup pin; alloc bağımlılığından kurtuluş; secure_boot_check API port; RFC8032 TV1 + tampered sig + wrong key test'leri bit-eşit)
2. **`extern crate alloc` + `#[global_allocator]` + `#[alloc_error_handler]` + `#![feature(alloc_error_handler)]` removal** — kernel artık pure `no_std + no_alloc`
3. **`src/sandbox/` klasörü tamamen sil** (mod.rs ~600 satır + allocator.rs ~100 satır)
4. **`wasmi` optional dep + `wasm-sandbox` feature sil** (Cargo.toml + check-cfg list)
5. **`WASM_HEAP_SIZE` sabiti sil** (`src/common/config.rs:83`) + `src/verify.rs` `mem_budget_within_ram` proof body update (WASM_HEAP_SIZE referansı kaldırılır)
6. **`PolicyEvent::WasmTrap` → `PolicyEvent::TaskFault` rename** — `src/kernel/policy/mod.rs` enum + `src/kernel/scheduler/mod.rs::handle_task_fault` + `src/arch/trap.rs` (5 ref). WASM tüm kaldırılırken policy event ismi de WASM-bağımsız semantik almalı.
7. **WASM-tied Kani proofs sil** (~13-15: COMPUTE_* WCET, dispatch_compute_empty, allocator wrapping_add ×2, LEB128 ×3, float reject ×2, WASM exec path ×5). Kani 213 → ~200 hedef.
8. **`sipahi.ld` `.wasm_arena` section sil** (4MB NOLOAD reserve kalkar) — `_end` ve `__clear_end` ~4MB küçülür → boot clear loop süresi azalır (`.task_stacks → .wasm_arena → __clear_end → _end` sırası: NOLOAD section yine location counter ilerletir; silinince `_end` küçülür)
9. **Self-test Sprint 12 WASM block sil** (`src/tests/mod.rs` ~50 satır + `test_wasm()` çağrısı)
10. **COMPUTE_* / WCET_COMPUTE_* kalıntı yorumları sil** (`src/common/config.rs` line 184-240 area — U-22.5 G2'de sabitler silindi, yorumlar kaldı; v2.0'da yorumlar da kalkar)
11. **CI/script cleanup**:
    - `.github/workflows/ci.yml:482-494` `.wasm_arena absent` guard → "no wasm artifacts" guard'a dönüştürülür (post-v2.0 amaç: section'ın hiç olmaması)
    - `scripts/feature_matrix.sh:23-25` `wasm-sandbox` kombinasyon satırları silinir
    - `scripts/u19_remove_blanket.sh:15` `src/sandbox/mod.rs` referansı silinir (script U-19 archive olduğu için ek banner ekle)
12. **Doc senkronu (kapsamlı)** — `README.md`, `ARCHITECTURE.md`, `CHANGELOG.md`, `STRUCTURE.md`, `docs/sipahi_context.md`, `docs/sipahi_features_en.md`, `docs/sipahi_features_tr.md`, `SIPAHI_SNTM_DESIGN.md` status section. Kani sayısı update + WASM bölüm "tarihsel/v1.0'da vardı" context'e taşı. `coverage.toml` `[feature.wasm-sandbox]` entry sil.

### OUT of scope (DEFER)
| İtem | Hedef | Sebep |
|------|-------|-------|
| FPGA bring-up (CVA6 silicon) | U-28 | hardware bekliyor |
| Typed IPC codegen | v1.7+ SAFE-2 | SNTM design §17.6 phased rollout |
| Static cap table | v1.7+ SAFE-2 | SNTM design §17.5 |
| Binary verifier | v1.8+ SAFE-3 | SNTM design §17.3 |
| Task certificate | v1.8+ SAFE-3 | SNTM design §17.4 |
| Stack analyzer | v1.9+ SAFE-4 | SNTM design §17.7 |
| HSM/OTP production key | v2.0+ HSM sprint | hardware integration |

---

## 2. Invariants — sprint boyunca BOZULMAYACAK

**U-27.5 carry-forward (15 invariant):**
1-14: U-27 invariant'ları (PMP, scheduler, sealed channel, cross-task statik, vb.) korunur
15: SNTM-R12 runtime ihlal observe (cross-isolation-demo + 4-gate script) korunur

**U-29 yeni (4 invariant):**
16. **Kernel `no_std + no_alloc` saf** — `extern crate alloc`, `#[global_allocator]`, `#[alloc_error_handler]`, `#![feature(alloc_error_handler)]` HİÇBİRİ kernel kaynak ağacında YOK. `grep -rE "extern crate alloc|global_allocator|alloc_error_handler" src/` boş.
17. **WASM compile-out absolute** — `wasmi`, `src/sandbox/`, `wasm_arena`, `wasm-sandbox` feature, `COMPUTE_*` sabit, `WASM_HEAP_SIZE` sabit, `dispatch_compute` fonksiyonu, `PolicyEvent::WasmTrap` enum variant HİÇBİRİ kernel kaynak ağacında YOK. Tarihsel context için `CHANGELOG.md` ve `SIPAHI_SNTM_DESIGN.md` §11.5'te yorum kalabilir; runtime artifact kalmaz.
18. **ed25519-compact RFC8032 TV1 valid** — secure_boot_check + ed25519 self-test path bit-eşit; pubkey 32B + sig 64B format aynı; QEMU_TEST_SIGNATURE bit-aynı; ed25519-compact Cargo.toml'da gerçek exact pin (placeholder DEĞIL).
19. **PolicyEvent::WasmTrap rename complete** — `PolicyEvent::TaskFault` (veya `NativeTaskFault`) olarak yeniden adlandırıldı; `decide_action` event=2 branch davranışı aynı (restart_count<MAX_RESTART_FAULT → Restart, else Isolate); handle_task_fault çağrısı + 5 referans güncellendi.

**Carry guard (regression test):** U-27 invariant 11 (seal_channels atomic) + 12 (cross-task isolation statik) + 13 (native_create_task idempotent) self-test'leri **DOKUNULMAZ** — alloc/WASM cleanup bunları etkilemez. G10 doğrular.

---

## 3. Codex Pre-Review Fix List (anticipate)

### FIX-A — ed25519-compact API farkı + format invariant + REAL exact pin
`ed25519-dalek v2.2.0` mevcut API:
```rust
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
let vk = VerifyingKey::from_bytes(pubkey_32)?;
let sig = Signature::from_bytes(sig_64);
vk.verify(msg, &sig).is_ok()
```

`ed25519-compact` (no_alloc) API (mental model — port sırasında **`cargo doc` ile DOĞRULA**):
```rust
use ed25519_compact::{PublicKey, Signature};
let pk = PublicKey::from_slice(pubkey_32)?;     // returns Result
let sig = Signature::from_slice(sig_64)?;       // returns Result
pk.verify(msg, &sig).is_ok()
```

**Format invariant**: pubkey 32B, sig 64B — aynı (RFC8032). Wire format DEĞIŞMEZ. Test vectors (`QEMU_TEST_SIGNATURE` bytes) korunur. Sadece library imports + error handling değişir.

**Cargo pin (Codex pre-review fix — REAL version):**
- G1 ÖNCE: `cargo search ed25519-compact --limit 1` ile latest stable çek
- Cargo.toml'a `=X.Y.Z` formatında REAL exact-pin yaz (placeholder `=2.x.x` YASAK)
- `cargo doc -p ed25519-compact --open` ile API yüzeyini doğrula
- **Eğer API mental model ile uyuşmazsa**: dur, raporla, kullanıcıya alternatif sun (örn. `from_bytes` yerine farklı constructor). FAKE GREEN YAPMA.
- Migration patlarsa (link error, type mismatch, no_std uyumsuzluğu): G1 STOP, ed25519-compact yerine başka no_alloc Ed25519 alternatifi araştır (örn. `ed25519`/`ed25519-zebra` `default-features = false`).

### FIX-B — Migration sıralaması KRİTİK
WASM cleanup ve ed25519-compact migration **birbirine bağlı**:
- `extern crate alloc` ve `#[global_allocator]` ed25519-dalek için var (Vec kullanır verify path'inde)
- ed25519-compact'a geçmeden `#[global_allocator]` silinirse → link error (allocator missing)
- WASM silinmeden ed25519-compact'a geçilirse → ed25519-dalek alloc-feature OFF da denenebilir ama compact daha temiz

**Doğru sıra:**
1. G1: ed25519-compact migrate (Cargo.toml + secure_boot.rs API port + test'ler PASS)
2. G2: ed25519 self-test verify (RFC8032 TV1 + tampered + wrong key)
3. G3: WASM source (sandbox/) sil + alloc removal (main.rs)
4. G4-G6: WASM dep / linker / self-test temizliği
5. G7-G8: Sabit yorum + feature flag pruning
6. G9-G10: Doc + verification battery

**G1 öncesi G3** denenirse: ed25519-dalek alloc dep'i breaking — link fail.

### FIX-C — main.rs alloc cleanup ordering
`src/main.rs` mevcut (kontrol et):
```rust
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;
// ...
#[global_allocator]
static ALLOCATOR: BumpAllocator = ...;
#[alloc_error_handler]
fn oom(_layout: core::alloc::Layout) -> ! { ... }
```

U-29 sonu hepsi kalkar. Kontrol: `cargo build --release` PASS (artık alloc bağımlı dep yok), `cargo build --release --features self-test` PASS (Sprint 12 WASM block silinince alloc gerek yok).

**Edge:** `Vec`, `Box`, `String` referansı kernel'da varsa link fail. G1 sonrası `grep -rE "Vec<|Box<|String::|format!" src/` boş olmalı (production build için).

### FIX-D — Linker script .wasm_arena impact (Codex pre-review düzeltme)
`sipahi.ld` mevcut layout (sirasıyla):
```
.bss   → __bss_end
kernel_stack (16K) → __stack_top
__pmp_data_end
.task_stacks (NOLOAD)
.wasm_arena (NOLOAD) ALIGN(4096) { . = . + 4M; }   ← BU SİLİNECEK
__clear_end = .;    ← wasm_arena SONRASINDA — location counter ilerledi
_end = .;           ← aynı şekilde wasm_arena sonrasında
```

**Önemli (Codex fix):** `NOLOAD` section yine location counter ilerletir. `.wasm_arena` silinince `__clear_end` ve `_end` **gerçekten ~4MB küçülür**. Bunun pozitif etkileri:
- **Boot clear loop süresi azalır** — `boot.S` `__bss_start → __clear_end` arası sıfırlar; aralık 4MB küçülünce binlerce CPU cycle tasarruf
- **PMP entry 4-5 RW kapsamı dolaylı küçülmez** (PMP_data_end .bss + kernel_stack sonu, wasm_arena ondan SONRA — etkilenmez)
- `__native_task_base = 0x80600000` (U-26 FIX-A) ASSERT `_end <= __native_task_base` korunur (eski _end ~0x80500000, yeni _end ~0x80100000 civarı)
- QEMU memory 512MB ham — etkilenmez

**G5 boot.S kontrolü:** Eğer `boot.S` `__bss_start..__clear_end` arası clear loop yapıyorsa, _end küçülmesi otomatik (linker symbol re-resolve eder, kod değiştirilmesi gerekmez). G5'te disasm + objdump ile _end ve __clear_end yeni adresini verify et.

**Yorum güncellemesi:** sipahi.ld yorum satırlarında "wasm arena" mention'ları sil veya tarihsel context'e taşı.

### FIX-E — Kani proof silme + verify.rs compile guard
WASM Kani proof'ları silmek:
1. `src/sandbox/mod.rs::verification` mod tamamen kalkacak (G3'te sandbox/ silinince)
2. `src/sandbox/allocator.rs` Kani proof'ları silinecek (G3'te)
3. `src/verify.rs` içindeki:
   - **Proof 12** (compute WCET ordering) — COMPUTE_* sabitleri zaten yok, proof body comment-only kalmış olabilir; tamamen sil
   - **Proof 4** (wcet_ordering_consistent) içindeki WCET_COMPUTE_* assertion'lar — zaten U-22.5 G4'te silinmiş; check + kalan referans varsa sil

**Sıra:** sandbox/ silmeden ÖNCE `verify.rs` proof temizliği yapma; G3 sandbox/ silince ilgili proof'lar otomatik gider. Sadece `verify.rs`'de KALAN orphan referans varsa G4'te ayıkla. `cargo kani` derler durumda olmalı her G-task sonu.

Kani sayı hedefi: 213 → **~198-200** (delta -13 ile -15 arası, design §11.7'ye göre).

### FIX-F — self-test build feature zincirleme
`self-test = ["test-keys", "trace", "debug-boot", "wasm-sandbox"]` → U-29'da `wasm-sandbox` kalkar:
```toml
self-test = ["test-keys", "trace", "debug-boot"]
```

`check-cfg` list:
```toml
# Önce
unexpected_cfgs = { ..., values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "wasm-sandbox", "sntm", "sntm-safe", "cross-isolation-demo") }

# Sonra
unexpected_cfgs = { ..., values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "sntm", "sntm-safe", "cross-isolation-demo") }
```

`coverage.toml` `[feature.wasm-sandbox]` entry silinecek; gate symmetric kuralı (Cargo.toml ↔ coverage.toml) korunur.

### FIX-G — sipahi_api workspace member taraması
`sipahi_api` crate'i ed25519 kullanmıyor (task-side API, sadece syscall wrappers). Yine de:
```bash
grep -rE "ed25519|wasmi|alloc" sipahi_api/ tasks/
```
beklenen: boş (task'lar `panic = syscall::exit(255)` + no_std + no_alloc). G1 öncesi confirm.

### FIX-H — Doc senkronu schema (Codex genişletme: 8 dosya kapsamı)
Sadece README + ARCHITECTURE + CHANGELOG yetmez. Kapsamlı doc senkronu:

| Dosya | Update |
|-------|--------|
| `README.md` | Kani sayısı 213 → ~200; WASM bölümü tarihsel context'e taşı |
| `ARCHITECTURE.md` | `src/sandbox/` bölümü sil; alloc/global_allocator bölümü sil; v2.0 SNTM pure section |
| `CHANGELOG.md` | Yeni `## [2.0.0]` section (release date G10 sonu) |
| `STRUCTURE.md` | `src/sandbox/` dir listesi kaldır; module map güncelle |
| `docs/sipahi_context.md` | WASM mention'lar tarihsel notlara dönüştür |
| `docs/sipahi_features_en.md` | "WASM sandbox" feature listesinden sil; v2.0 cleanup row ekle |
| `docs/sipahi_features_tr.md` | aynı (Türkçe karşılık) |
| `SIPAHI_SNTM_DESIGN.md` | §11.7 doc senkron tablosu **v1.5 row → v2.0 row** transition complete; Implementation Readiness section U-29 PASS marker |

**Tarihsel context kuralı**: Bu 8 dosya **tarihsel WASM mention'larını yasaklamaz** — "v1.0/v1.5'te wasmi vardı, v2.0'da kaldırıldı (no_alloc doctrine)" tarz historical notlar OK. **Runtime claim'leri yasak** — "WASM sandbox aktif" tarz cümleler kalmaz.

### FIX-I — PolicyEvent::WasmTrap rename (Codex zorunluluk)
WASM cleanup yalnızca dep + sandbox/ + linker silmek değil — policy semantic'i de güncellenmeli. `PolicyEvent::WasmTrap` ismi WASM kalkınca semantically dead:

**Mevcut:**
```rust
// src/kernel/policy/mod.rs
pub enum PolicyEvent {
    BudgetExhausted  = 0,
    StackOverflow    = 1,
    WasmTrap         = 2,        // ← WASM-specific naming
    ...
}

// src/kernel/scheduler/mod.rs
pub(crate) fn handle_task_fault() {
    let action = apply_policy(current as u8, PolicyEvent::WasmTrap, dal);  // ← WASM ismi
    ...
}
```

**U-29 sonra:**
```rust
pub enum PolicyEvent {
    BudgetExhausted  = 0,
    StackOverflow    = 1,
    TaskFault        = 2,        // ← generic native task fault
    ...
}

pub(crate) fn handle_task_fault() {
    let action = apply_policy(current as u8, PolicyEvent::TaskFault, dal);
    ...
}
```

**Etkilenen dosyalar (5):**
- `src/kernel/policy/mod.rs` — enum variant rename
- `src/kernel/scheduler/mod.rs` — handle_task_fault çağrısı
- `src/arch/trap.rs` — PolicyEvent::WasmTrap referansı (varsa)
- `src/sandbox/mod.rs` — G3'te silinince auto-gone
- Comments/docstrings — `// WasmTrap` → `// TaskFault` (5+ yerde)

**decide_action davranış invariantı**: event=2 branch policy davranışı (restart_count<MAX_RESTART_FAULT → Restart, else Isolate) **DEĞİŞMEZ**. Sadece isim güncellenir.

**U-27.5 cross-isolation gate etkisi**: Cross-task PMP runtime ihlal observe path'i `handle_task_fault` → `PolicyEvent::WasmTrap` → restart×3 → Isolate idi; U-29 sonrası `PolicyEvent::TaskFault` → aynı davranış. **U-27.5 invariant 15 korunur** — script gate marker pattern aynı (4. trap'te `[OK]`).

### FIX-J — CI + scripts cleanup (Codex genişletme)
1. **`.github/workflows/ci.yml:482-494`** mevcut guard:
   ```yaml
   - name: Check .wasm_arena absent in production (G8 verification)
     run: |
       ARENA_SIZE=$(... | awk '/\.wasm_arena/ {print $7; exit}')
       # ARENA <= 64 ise PASS
   ```
   U-29 sonra `.wasm_arena` section **HİÇ YOK** → guard'ı **invert** et:
   ```yaml
   - name: Check no WASM artifacts in production (v2.0 invariant)
     run: |
       BIN=target/.../release/sipahi
       # 1. .wasm_arena section bulunmamalı
       if riscv64-linux-gnu-readelf -S $BIN | grep -q '\.wasm_arena'; then
         echo "FAIL: .wasm_arena section found (v2.0 invariant violated)"; exit 1
       fi
       # 2. wasmi symbol bulunmamalı
       if riscv64-linux-gnu-nm $BIN 2>/dev/null | grep -qi 'wasmi'; then
         echo "FAIL: wasmi symbol found"; exit 1
       fi
       echo "PASS: no WASM artifacts in production binary"
   ```

2. **`scripts/feature_matrix.sh:23-25`** kombinasyonlar:
   ```bash
   # SİL:
   "fast-crypto,fast-sign,test-keys,wasm-sandbox"
   "fast-crypto,fast-sign,test-keys,wasm-sandbox,v2-hal"
   ```
   Yorum güncelle: feature_matrix.sh header'da "wasm-sandbox kombinasyonları U-29'da kaldırıldı" notu.

3. **`scripts/u19_remove_blanket.sh:15`** `src/sandbox/mod.rs` referansı:
   - Bu script U-19 archive (blanket `#[allow]` removal sweep). Aktif değil
   - **Seçenek A**: `src/sandbox/mod.rs` satırını sil + script'i `archive/` altına taşı (eğer böyle bir konvansiyon varsa)
   - **Seçenek B**: script başına `# ARCHIVE — U-19 historical, post-U-29 sandbox/ silindi` banner ekle ve sandbox/mod.rs satırını yorumla (komut çalıştırma olarak)
   - **Öneri**: B (script'i tutmak doğru — gelecek benzer sweep'lerde örnek; sadece sandbox satırını yorumla + banner ekle)

---

## 4. Görev Planı G0..G10

Test-first RED→GREEN. Her G-task GREEN olmadan sonrakine geçilmez.

### G0 — Baseline audit (15dk)

**Önkoşul:** U-27.5 commit + tag (v1.5.1) tamamlanmış. Working tree clean.

Audit:
- [ ] `git status --short` boş (sadece SPRINT_U29_PROMPT.md untracked olabilir)
- [ ] `git log --oneline -1` U-27.5 commit
- [ ] `git tag | grep v1.5.1` → present
- [ ] `cargo kani` → **213/213 PASS**
- [ ] `bash Tla+/run_tlc.sh` → **8/8 PASS**
- [ ] `timeout 30 make run-self-test 2>&1 | grep "ALL TESTS PASSED"` → marker
- [ ] `timeout 30 make run 2>&1 | tee /tmp/u29_baseline_smoke.log` → NF/FATAL/POLICY-free
- [ ] `timeout 30 make run-cross-isolation` → 4-gate PASS
- [ ] `bash scripts/check_coverage.sh` → 15F + 14R PASS
- [ ] `make check` → clippy `-D warnings` PASS
- [ ] `bash scripts/sntm_sprint_gate.sh` → PASS

**Mevcut state probe (G3+ için baseline):**
```bash
grep -c "extern crate alloc" src/main.rs           # > 0 beklenen
grep -c "wasmi" Cargo.toml                          # > 0 beklenen
ls src/sandbox/                                     # var beklenen
grep -c "ed25519-dalek" Cargo.toml                  # = 1 beklenen
grep -c ".wasm_arena" sipahi.ld                     # > 0 beklenen
```

### G1 — ed25519-compact dep migration (60dk) [test-first]

**Dosyalar:** `Cargo.toml`, `src/hal/secure_boot.rs`, `src/hal/key.rs` (varsa)

**G1.0 ÖNCE (Codex fix — REAL version pin):**
```bash
cargo search ed25519-compact --limit 1
# Output örnek: ed25519-compact = "2.1.1"   # crates.io listing
cargo doc --target x86_64-unknown-linux-gnu -p ed25519-compact --open  # API doğrula
```
**Çıkan latest stable version'u Cargo.toml'a EXACT pin olarak yaz** (örn. `=2.1.1`). Placeholder `=2.x.x` YASAK. API mental model uyuşmazsa **DUR, RAPORLA** — G1 GREEN değil.

**Test (RED):**
- `grep -c ed25519-compact Cargo.toml` → 0 (henüz yok)
- Eğer şimdi `cargo build` çalıştırılırsa hala dalek kullanır

**Implement (GREEN):**

**Cargo.toml:**
```toml
# Önce
ed25519-dalek = { version = "=2.2.0", default-features = false, features = ["alloc"] }

# Sonra (X.Y.Z = G1.0'da cargo search ile bulunan REAL latest stable)
# U-29 v2.0 cleanup: ed25519-dalek alloc bağımlılığından kurtuluş.
# ed25519-compact pure no_std + no_alloc — RFC8032 wire format aynı,
# sadece library import + error handling değişti.
ed25519-compact = { version = "=X.Y.Z", default-features = false }
```

**`src/hal/secure_boot.rs`:**
```rust
// Önce (ed25519-dalek v2):
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

pub fn secure_boot_check(
    image: &[u8],
    pubkey: &[u8; 32],
    signature: &[u8; 64],
) -> bool {
    let vk = match VerifyingKey::from_bytes(pubkey) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(signature);
    vk.verify(image, &sig).is_ok()
}

// Sonra (ed25519-compact, no_alloc):
use ed25519_compact::{PublicKey, Signature};

pub fn secure_boot_check(
    image: &[u8],
    pubkey: &[u8; 32],
    signature: &[u8; 64],
) -> bool {
    let pk = match PublicKey::from_slice(pubkey) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = match Signature::from_slice(signature) {
        Ok(s) => s,
        Err(_) => return false,
    };
    pk.verify(image, &sig).is_ok()
}
```

**Build test:**
- `cargo build --release` → PASS (ed25519-dalek kalkar, compact gelir)
- `cargo build --release --features self-test` → PASS

**ÖNEMLİ:** Bu adımda `extern crate alloc` + `#[global_allocator]` HENÜZ silinmez. ed25519-compact alloc gerektirmez ama main.rs hala WASM allocator için tanımlı — G3'te silinir. Sıralama riski yok.

### G2 — ed25519 self-test verify (30dk) [test-first]

**Dosyalar:** `src/tests/mod.rs` (ed25519 test bloğu)

**Test (RED — beklenen davranış):**
- Test runner ed25519 test'leri çalıştırır
- Önceki output:
  ```
  [SEC] Ed25519 RFC8032 TV1 [OK]
  [SEC] Ed25519 tampered sig RED [OK]
  [SEC] Ed25519 wrong key RED [OK]
  ```
- G1 sonrası tests aynı OK marker'larını üretmeli — API değişti, davranış aynı

**Implement (GREEN):**
- Test body'sinde sadece import değişir (`ed25519_dalek::*` → `ed25519_compact::*`)
- Test vectors değişmez (RFC8032 TV1 pubkey + sig + msg byte-aynı)
- `verify().is_ok()` → `verify().is_ok()` (boolean davranış aynı)

**Doğrulama:**
- `timeout 30 make run-self-test 2>&1 | grep "Ed25519"` → 3 satır [OK]
- `timeout 30 make run-self-test 2>&1 | grep "ALL TESTS PASSED"` → marker

### G3 — WASM source removal + alloc cleanup + PolicyEvent rename (90dk) [BÜYÜK adım]

**Dosyalar (sil):**
- `src/sandbox/mod.rs` (~600 satır)
- `src/sandbox/allocator.rs` (~100 satır)
- `src/sandbox/` klasörü tamamen

**Dosyalar (modify):**
- `src/main.rs`:
  ```rust
  // SİL:
  #![feature(alloc_error_handler)]
  extern crate alloc;
  // ... global_allocator + alloc_error_handler tanımları
  
  // Kalır:
  #![no_std]
  #![no_main]
  ```
- `src/kernel/mod.rs` veya `src/lib.rs` (varsa): `pub mod sandbox;` line sil
- **`src/common/config.rs:83` (FIX-3, Codex)**: `pub const WASM_HEAP_SIZE: usize = 4194304;` SİL
- **`src/verify.rs:687-688` (FIX-3 follow)**: `mem_budget_within_ram` proof body'sinde `WASM_HEAP_SIZE` referansını kaldır; `total = kernel_total + task_total` (wasm_heap = 0 olduğu için sadece terimi at)
- **`src/kernel/policy/mod.rs` (FIX-I, Codex)**: `WasmTrap = 2` → `TaskFault = 2` rename
- **`src/kernel/scheduler/mod.rs::handle_task_fault`**: `PolicyEvent::WasmTrap` → `PolicyEvent::TaskFault`
- **`src/arch/trap.rs`**: `PolicyEvent::WasmTrap` referansı varsa update

**Test (RED → GREEN sıralaması):**
- G3 başı `cargo build --release` muhtemelen FAIL (sandbox + WasmTrap referansları kırık)
- Tüm sandbox referansları + rename çözüldükten sonra `cargo build --release` PASS

**Adım adım**:
1. `src/sandbox/` klasörü `rm -rf`
2. `src/kernel/mod.rs` veya `src/lib.rs`'den `pub mod sandbox;` referansı kaldır
3. `src/main.rs`'den alloc-related lines kaldır
4. `src/common/config.rs:83` `WASM_HEAP_SIZE` sil
5. `src/verify.rs:687-688` proof body update
6. `src/kernel/policy/mod.rs` enum `WasmTrap → TaskFault` rename
7. `src/kernel/scheduler/mod.rs` + `src/arch/trap.rs` enum referansları update
8. Diğer modüllerde sandbox/allocator/WasmTrap import kullanan satırları temizle:
   ```bash
   grep -rE "use crate::sandbox|::WasmTrap|alloc::|extern crate alloc" src/
   ```
9. `cargo build --release` → PASS
10. `cargo build --release --features self-test` → muhtemelen FAIL (test_wasm fn sandbox referansı) — G6'da çözülecek

**Geçici workaround G3 sonu:** Eğer self-test build fail ediyorsa, G3 GREEN gate'i sadece production `cargo build` PASS olabilir. Self-test G6 sonrası GREEN.

**U-27.5 cross-isolation invariant koruma**: G3 sonu `make run-cross-isolation` çalıştır — `[OK]` marker hala görünür, çünkü `decide_action(TaskFault, ...)` davranışı `decide_action(WasmTrap, ...)` ile aynı (event=2 numerik değer aynı). G7 sonrası tam doğrulama.

### G4 — WASM-tied Kani proofs sil (30dk)

**Dosya:** `src/verify.rs`

G3'te `sandbox::verification` mod otomatik gitti. `src/verify.rs`'de KALAN WASM-related proof'ları temizle:
- `proof_12_compute_wcet_ordering` (varsa) — COMPUTE_* sabitleri zaten yok, body empty veya orphan
- WCET_COMPUTE_* assertion'lar (Proof 4 body'sinde olabilir, U-22.5 G4'te silindi — verify)

**Test:**
- `cargo kani` → derler + tüm proof'lar PASS
- Sayı 213 → ~200 (delta -13'ten -15'e, sandbox + verify temizliği)

### G5 — Linker script .wasm_arena removal (20dk) [Codex FIX-D düzeltme]

**Dosya:** `sipahi.ld`

Sil (line ~91-95):
```ld
/* WASM arena — Wasmi interpreter M-mode'da erişir */
.wasm_arena (NOLOAD) : ALIGN(4096) {
    __wasm_arena_start = .;
    *(.wasm_arena)
    __wasm_arena_end = .;
} > RAM
```

Yorumda WASM mention'ları tarihsel context'e taşı:
- Üst yorum bloğu (line 12-17 area) `.wasm_arena (RW) 4MB ...` satırı sil
- Header comment "WASM arena: PMP match yok → U-mode DENY, M-mode erişir (spec)" sil
- `__clear_end` yorumu update: "BSS + kernel_stack + task_stacks hepsini sıfırlar" (wasm_arena kaldırıldı)

**ÖNEMLİ (Codex fix):** `_end` ve `__clear_end` linker location counter'ın o noktadaki değeri. `.wasm_arena (NOLOAD)` 4MB ilerletiyordu — silince hem `_end` hem `__clear_end` **~4MB küçülür**. Sonuç:
- Boot clear loop (`__bss_start..__clear_end`) süresi azalır (~binlerce cycle tasarruf)
- `boot.S` clear loop kod değişikliği gerekmez — symbol re-resolve eder
- `ASSERT(_end <= __native_task_base)` korunur — _end küçüldü, native_task_base 0x80600000 sabit

**Test:**
- `cargo build --release` → PASS
- `objdump -h target/.../sipahi | grep wasm_arena` → **boş** (kalmadı, codex fix gate)
- `nm target/.../sipahi | grep _end` → eski adresten ~4MB küçük
- `nm target/.../sipahi | grep __clear_end` → eski adresten ~4MB küçük
- Boot smoke `timeout 30 make run` → NF/FATAL-free (boot init OK)

### G6 — Self-test Sprint 12 WASM block sil (30dk)

**Dosyalar:** `src/tests/mod.rs`

Sil:
- `fn test_wasm()` (Sprint 12 block, ~50 satır)
- `run_all()` içinde `test_wasm()` çağrısı
- WASM ile ilgili imports

**Test (GREEN):**
- `cargo build --release --features self-test` → PASS
- `timeout 30 make run-self-test 2>&1 | grep "Sprint 12"` → boş (kaldırıldı)
- `timeout 30 make run-self-test 2>&1 | grep "ALL TESTS PASSED"` → marker

### G7 — config.rs COMPUTE_* yorum temizliği (10dk)

**Dosya:** `src/common/config.rs`

U-22.5 G2'de COMPUTE_* / WCET_COMPUTE_* sabitleri silindi ama yorumlar (line 184-240) "tarihsel benchmark" olarak bırakılmıştı. v2.0'da yorumlar da kalkar (kernel'da no_ttwasm semantic).

Sil:
- Line 184: `// U-22.5 G2: COMPUTE_* ID sabitleri silindi (4 sabit).`
- Line 236-240: WCET_COMPUTE_* yorumları
- Line 250 civarı: yer değişikliği yorumu

**Test:**
- `cargo build --release` → PASS
- `grep -c "COMPUTE_" src/common/config.rs` → 0

### G8 — wasm-sandbox feature + check-cfg + coverage cleanup (15dk)

**Dosya:** `Cargo.toml`

```toml
# Önce:
wasm-sandbox = ["dep:wasmi"]
self-test = ["test-keys", "trace", "debug-boot", "wasm-sandbox"]
wasmi = { version = "=1.0.9", default-features = false, features = ["prefer-btree-collections"], optional = true }
unexpected_cfgs = { ..., values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "wasm-sandbox", "sntm", "sntm-safe", "cross-isolation-demo") }

# Sonra:
# wasm-sandbox feature SİLİNDİ (U-29 v2.0)
self-test = ["test-keys", "trace", "debug-boot"]
# wasmi dep SİLİNDİ
unexpected_cfgs = { ..., values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "sntm", "sntm-safe", "cross-isolation-demo") }
```

**coverage.toml:**
- `[feature.wasm-sandbox]` entry sil
- 15 feature → 14 feature (sayı düşer)
- coverage gate symmetric kontrol

**Test:**
- `cargo build --release` → PASS
- `make check` → clippy `-D warnings` PASS (check-cfg uyarısı yok)
- `bash scripts/check_coverage.sh` → 14F + 14R PASS

### G8.5 — CI + scripts cleanup (Codex FIX-J, 25dk)

**Dosya 1: `.github/workflows/ci.yml`** (line 482-494 area)

**Önce** (mevcut guard — section küçük olmalı):
```yaml
- name: Check .wasm_arena absent in production (G8 verification)
  run: |
    ARENA_SIZE=$(... | awk '/\.wasm_arena/ {print $7; exit}')
    if [ "$SIZE_DEC" -le 64 ]; then
      echo "PASS: .wasm_arena absent or empty in production"
    ...
```

**Sonra** (v2.0 invariant — section HİÇ olmamalı + wasmi symbol HİÇ olmamalı):
```yaml
- name: Check no WASM artifacts in production (v2.0 invariant)
  run: |
    BIN=target/riscv64imac-unknown-none-elf/release/sipahi
    # 1. .wasm_arena section bulunmamalı
    if riscv64-linux-gnu-readelf -S $BIN | grep -q '\.wasm_arena'; then
      echo "FAIL: .wasm_arena section found (v2.0 invariant violated)"
      exit 1
    fi
    # 2. wasmi symbol bulunmamalı
    if riscv64-linux-gnu-nm $BIN 2>/dev/null | grep -qi 'wasmi'; then
      echo "FAIL: wasmi symbol found in production binary"
      exit 1
    fi
    echo "PASS: no WASM artifacts in production binary"
```

**Dosya 2: `scripts/feature_matrix.sh`** (line 23-25 area)

Sil:
```bash
"fast-crypto,fast-sign,test-keys,wasm-sandbox"
"fast-crypto,fast-sign,test-keys,wasm-sandbox,v2-hal"
```

Header yorum güncelle (line 3):
```bash
# G5 (v2-hal), G6 (production-otp), G25 (entropy check)
# NOT: G8 (wasm-sandbox) U-29 v2.0'da kaldırıldı — wasm-sandbox feature artık yok.
```

**Dosya 3: `scripts/u19_remove_blanket.sh`** (line 15)

`src/sandbox/mod.rs` satırı:
- **Seçenek (öneri)**: script başına banner ekle + sandbox/mod.rs satırını yorumla:
  ```bash
  # ARCHIVE — U-19 historical sweep (blanket #[allow] removal).
  # post-U-29: src/sandbox/ silindi, ilgili satır yorum.
  # Bu script artık çalıştırılamaz; gelecek benzer sweep'lerde örnek olarak korunur.
  
  for file in \
      ...
      # src/sandbox/mod.rs  # U-29: sandbox silindi
      ...
  ```

**Test:**
- `bash scripts/feature_matrix.sh --dry-run` (varsa) → wasm-sandbox kombinasyonu yok
- `bash scripts/u19_remove_blanket.sh` runtime executed değil (archive)
- CI guard manuel inspect: `grep -A20 "no WASM artifacts" .github/workflows/ci.yml`

### G9 — Doc senkronu (45dk) [Codex FIX-H genişletme: 8 dosya]

**Dosya 1: `README.md`**
- Kani proof sayısı `213` → güncelle (G10 sonra net sayı belli)
- "WASM sandbox" satırları "v1.0'da vardı, v2.0'da kaldırıldı (no_alloc doctrine)" tarihsel context

**Dosya 2: `ARCHITECTURE.md`**
- `src/sandbox/` bölümü sil
- alloc / global_allocator bölümü sil
- SNTM tasks/ bölümü v2.0 vurgusu
- Module layout diagram update (sandbox node kaldırıldı)

**Dosya 3: `CHANGELOG.md`**
- Yeni section: `## [2.0.0] - YYYY-MM-DD` (G10 sonu net tarih)
- Added: ed25519-compact migration, PolicyEvent::TaskFault rename, no_alloc kernel
- Removed: WASM (wasmi, sandbox/, wasm_arena, COMPUTE_*, WASM_HEAP_SIZE, alloc, global_allocator, ~13 Kani proof, Sprint 12 self-test, wasm-sandbox feature, PolicyEvent::WasmTrap)
- Changed: self-test feature listesi (wasm-sandbox kalktı), Kani sayısı 213→~200, _end/__clear_end ~4MB küçüldü (boot clear süresi azaldı)

**Dosya 4: `STRUCTURE.md`** (Codex fix)
- `src/sandbox/` dir listesi kaldır
- module map güncelle (alloc/global_allocator yok)
- v2.0 SNTM pure native task model vurgusu

**Dosya 5: `docs/sipahi_context.md`** (Codex fix)
- WASM mention'lar tarihsel notlara dönüştür ("v1.0'da WASM sandbox vardı, v2.0'da SNTM ile tamamen değişti")
- Runtime "WASM aktif" tarz cümle bırakma

**Dosya 6: `docs/sipahi_features_en.md`** (Codex fix)
- "WASM sandbox" feature listesinden sil veya "(removed in v2.0)" annotation
- v2.0 cleanup row ekle (no_alloc, ed25519-compact)

**Dosya 7: `docs/sipahi_features_tr.md`** (Codex fix)
- aynı (Türkçe karşılık)

**Dosya 8: `SIPAHI_SNTM_DESIGN.md`** (Codex fix)
- §11.7 doc senkron tablosu **v1.5 row → v2.0 row** transition complete
- Implementation Readiness section (line ~2685): U-29 status `Hazır ✓` → `Tamamlandı ✓`
- v2.0 final closure note

**Tarihsel context kuralı (FIX-H carry):** 8 dosyada **tarihsel WASM mention yasak değil** — "v1.0'da vardı, v2.0'da kaldırıldı" tarz historical notlar OK. **Runtime claim'leri yasak** — "WASM sandbox aktif çalışıyor" tarz cümleler kalmaz.

### G10 — Verification battery + final report (45dk)

Sıralı:
1. `cargo kani` → **~200/~200 PASS** (delta -13 ile -15 arası; exact sayı kaydet)
2. `bash Tla+/run_tlc.sh` → **8/8 PASS** (SipahiSNTM 138 states, değişmez)
3. `timeout 30 make run-self-test` → ALL TESTS PASSED (Sprint 12 WASM yok, U-27 + U-27.5 testler korunur)
4. `timeout 120 make run` → 600+ TICK NF/FATAL/POLICY-free
5. `timeout 30 make run-cross-isolation` → 4-gate PASS (U-27.5 invariant 15 korunur)
6. `bash scripts/check_coverage.sh` → **14F + 14R PASS** (wasm-sandbox feature kalktı)
7. `make check` → clippy `-D warnings` PASS
8. `bash scripts/sntm_sprint_gate.sh` → PASS

**No-go regression guard (zorunlu, Codex FIX-K kapsamlı tarama):**

7 ayrı kategori, **her biri ayrı pass-or-fail**. Tarihsel doc mention'lar bu guard'ı geçer (CHANGELOG.md + SIPAHI_SNTM_DESIGN.md §11.5 hariç).

```bash
# ─── KATEGORI 1: src/ alloc removal ───
! grep -rE "extern crate alloc|#\[global_allocator\]|#\[alloc_error_handler\]" src/
! grep -rE "Vec<|Box<|String::|format!" src/

# ─── KATEGORI 2: src/ WASM runtime references ───
! grep -riE "wasmi|wasm_arena|WASM_HEAP_SIZE|COMPUTE_|dispatch_compute|WasmTrap" src/
test ! -d src/sandbox

# ─── KATEGORI 3: Cargo.toml ───
! grep -E "wasmi|wasm-sandbox" Cargo.toml
! grep -E "ed25519-dalek|ed25519_dalek" Cargo.toml
[ "$(grep -c 'ed25519-compact' Cargo.toml)" = "1" ]   # exact 1 occurrence
# Exact pin format (Codex FIX-A): =X.Y.Z, NOT placeholder
! grep -E 'ed25519-compact.*=2\.x\.x' Cargo.toml

# ─── KATEGORI 4: sipahi.ld ───
! grep -F ".wasm_arena" sipahi.ld
! grep -iE "wasm_arena|wasmi" sipahi.ld
# _end + __clear_end ~4MB küçülmeli (build sonra objdump ile verify)

# ─── KATEGORI 5: scripts/ ───
! grep -rE "wasm-sandbox" scripts/feature_matrix.sh
# u19_remove_blanket.sh sandbox satırı yorum veya silinmiş
if grep -E "src/sandbox" scripts/u19_remove_blanket.sh; then
    grep -E "^\s*#.*src/sandbox|ARCHIVE.*U-29" scripts/u19_remove_blanket.sh   # yorum/banner
fi

# ─── KATEGORI 6: .github/ CI ───
# .wasm_arena guard inverted veya "no WASM artifacts" guard'a dönüştürülmüş
grep -E "no WASM artifacts|v2.0 invariant" .github/workflows/ci.yml
! grep -E "\.wasm_arena.*<=.*64" .github/workflows/ci.yml   # eski guard kalmadı

# ─── KATEGORI 7: docs/ + top-level docs ───
# Tarihsel mention OK ama runtime claim YOK
! grep -iE "WASM sandbox.*aktif|wasmi.*running" \
    docs/sipahi_context.md \
    docs/sipahi_features_en.md \
    docs/sipahi_features_tr.md \
    STRUCTURE.md \
    README.md \
    ARCHITECTURE.md

# ─── KATEGORI 8: coverage.toml ───
! grep -E "\[feature\.wasm-sandbox\]" coverage.toml

# ─── KATEGORI 9: U-27/U-27.5 invariant korunması ───
[ "$(grep -c 'cross-isolation-demo' Cargo.toml)" = "2" ]   # feature + check-cfg
grep -c "task_world" sipahi.toml | grep -v "^0$"            # > 0
test -x scripts/check_cross_isolation.sh
# Cross-isolation gate (smoke) — PolicyEvent::TaskFault rename sonrası hala 4-gate PASS
timeout 30 make run-cross-isolation 2>&1 | grep -q "PASS:"

# ─── KATEGORI 10: PolicyEvent rename complete ───
! grep -rE "PolicyEvent::WasmTrap|WasmTrap\s*=" src/
grep -rE "PolicyEvent::TaskFault" src/ | wc -l   # >= 1 (rename'in en az 1 kullanıcısı)
```

**Tüm 10 kategori GREEN olmalı.** Herhangi biri RED → G10 başarısız, root cause investigation.

### Final report template

```markdown
## Sprint U-29 — Final Report (v2.0)

### Completed
- G0: baseline audit (U-27.5 v1.5.1 clean)
- G1: ed25519-dalek → ed25519-compact migration (Cargo.toml + secure_boot.rs)
- G2: ed25519 self-test verify (RFC8032 TV1 + tampered + wrong key)
- G3: WASM source removal — src/sandbox/ klasörü tamamen sil + main.rs alloc cleanup
- G4: WASM-tied Kani proofs sil (verify.rs orphan + sandbox autoremoved)
- G5: sipahi.ld .wasm_arena section sil (4MB NOLOAD reserve kalktı)
- G6: Self-test Sprint 12 WASM block sil + test_wasm() çağrısı
- G7: config.rs COMPUTE_* / WCET_COMPUTE_* yorum temizliği
- G8: wasm-sandbox feature + check-cfg list cleanup
- G9: README.md + ARCHITECTURE.md + CHANGELOG.md senkron

### Verification metrics
- Kani: 213 → **~200 PASS** (delta -13 ile -15: WASM proofs autoremoved + verify.rs cleanup)
- TLA+: 8/8 PASS (no delta — runtime spec değişmedi)
- Self-test: ALL TESTS PASSED (Sprint 12 WASM block YOK, U-27/U-27.5 korunur)
- Production smoke: 120s NF/FATAL/POLICY-free
- Cross-isolation gate: 4/4 PASS (U-27.5 invariant 15 korunur)
- Coverage: 14F + 14R (wasm-sandbox feature kalktı)
- Clippy: -D warnings PASS

### Invariant audit
- U-27.5 1..15: hepsi korundu
- U-29 #16 (kernel no_std + no_alloc): grep boş ✓
- U-29 #17 (WASM compile-out absolute): grep boş + dir yok ✓
- U-29 #18 (ed25519-compact RFC8032 TV1): 3 self-test [OK] ✓

### Surface reduction
- Kernel kaynak: ~700 satır silindi (sandbox/ + main.rs alloc bloğu + self-test WASM)
- Dependency: 2 → 1 crate (ed25519-dalek + wasmi → ed25519-compact)
- Feature flag: 12 → 11 (wasm-sandbox kalktı)
- Kani proof: 213 → ~200 (-13 WASM-tied)
- coverage.toml: 15F → 14F

### Commit önerisi (NO auto-commit)
sprint-u29: v2.0 — WASM removal + ed25519-compact migration

- ed25519-dalek → ed25519-compact (no_std + no_alloc, RFC8032 wire same)
- src/sandbox/ klasörü tamamen sil (~700 satır)
- main.rs alloc cleanup: extern crate alloc + global_allocator +
  alloc_error_handler + feature(alloc_error_handler) hepsi kalktı
- wasmi optional dep sil, wasm-sandbox feature sil
- WASM-tied Kani proofs sil (~13: COMPUTE_*, dispatch_compute,
  allocator wrapping_add, LEB128, float reject, WASM exec path)
- sipahi.ld .wasm_arena (4MB NOLOAD) section sil
- Self-test Sprint 12 WASM block sil + test_wasm() çağrısı
- config.rs COMPUTE_* / WCET_COMPUTE_* yorum temizliği
- self-test feature: ["test-keys", "trace", "debug-boot"] (wasm-sandbox kalktı)
- check-cfg list update
- coverage.toml [feature.wasm-sandbox] sil
- README + ARCHITECTURE + CHANGELOG senkron (Kani 213→~200, WASM tarihsel)

Kernel artık pure no_std + no_alloc. SNTM v1.5 → v2.0 closure.

Kani ~200/~200, TLA 8/8, smoke 120s clean, coverage 14F/14R,
cross-isolation 4/4 PASS, sntm gate PASS, clippy clean.

### Tag önerisi
**v2.0.0** — SNTM v2.0 final: WASM removed + ed25519-compact + no_alloc kernel.
```

---

## 5. Doctrine Reminder

- **NO auto-commit** — her commit için ayrı onay iste
- **Test-first RED→GREEN** — implementation öncesi test yaz
- **Migration sıralaması KRİTİK** — ed25519-compact ÖNCE (G1-G2), WASM SONRA (G3+)
- **U-27 + U-27.5 invariants korunur** — cross-isolation gate + 14 invariant + script gate dokunulmaz
- **NO destructive git** — force push, reset, branch-D YOK
- **§18.7 3-yorum rule** — yeni `unsafe fn` için VERIFIES/CALLS/FAILS-IF yorumları (10-line window)

---

## 6. Audit (Kontrol)

| Soru | Cevap |
|------|-------|
| ed25519-compact API farkı kanıtlandı mı? | FIX-A'da mental model; G1.0'da `cargo search` ile REAL exact pin + `cargo doc` ile API doğrula; placeholder `=2.x.x` YASAK; migration patlarsa STOP+RAPORLA |
| Migration sırası neden ed25519 ÖNCE? | FIX-B: ed25519-dalek alloc tüketicisi; #[global_allocator] silmeden compact'a geçilmeli |
| linker script .wasm_arena silince _end değişir mi? | **EVET, ~4MB küçülür** (Codex FIX-D düzeltme): NOLOAD section da location counter ilerletir; `__clear_end` ve `_end` `.wasm_arena` SONRASINDA tanımlı; silince ikisi de küçülür → boot clear süresi azalır (~binlerce cycle tasarruf); ASSERT korunur |
| PolicyEvent::WasmTrap kalırsa WASM tam silinmiş olur mu? | HAYIR (Codex FIX-I): G3'te `TaskFault` rename zorunlu — enum semantic'i de WASM-bağımsız olmalı |
| WASM_HEAP_SIZE sabiti unutuldu mu? | HAYIR (Codex FIX-3): G3'te `src/common/config.rs:83` + `src/verify.rs:687-688` cleanup |
| CI/script kalıntıları? | Codex FIX-J: G8.5'te ci.yml guard invert + feature_matrix.sh combo sil + u19_remove_blanket.sh archive banner |
| Doc senkronu kapsamı? | 8 dosya (Codex FIX-H): README + ARCHITECTURE + CHANGELOG + STRUCTURE + 3 docs/* + SIPAHI_SNTM_DESIGN; tarihsel mention OK, runtime claim YASAK |
| Self-test test_wasm() silinince diğer test'ler etkilenir mi? | HAYIR — bağımsız test block; G6 sonu run_all() listesinden çağrı kaldırılır |
| Kani sayısı kesin -13 mi? | FIX-E + design §11.7: 5 verify.rs WASM proof + 2 allocator + 3 LEB128 + 2 float reject + ~5 WASM exec = ~17 ama bazıları zaten U-22.5'te silindi; net delta -13 ile -15 arası, G10 net sayar |
| U-27.5 cross-isolation invariant'ı korunur mu? | EVET — alloc/WASM cleanup PMP trap path'ini etkilemez; `decide_action` event=2 davranışı `TaskFault` rename sonrası aynı (numerik değer same); G10 KATEGORI 9 doğrular |
| Production unaffected guard? | EVET — WASM zaten production'da compile-out idi (U-22 G8); v2.0'da kaynak da silinir + ci.yml `no WASM artifacts` guard yakalar |
| No-go grep kapsamı yeterli mi? | Codex FIX-K: 10 kategori (src + Cargo.toml + sipahi.ld + scripts + .github + docs + coverage.toml + invariant + rename + alloc). Her biri ayrı pass-fail; tarihsel doc CHANGELOG/SNTM_DESIGN istisna |
| Yeni Kani proof / TLA+ invariant var mı? | YOK — U-29 pure cleanup, yeni soyutlama eklenmez |

---

## 7. Hazır mı? (Codex pre-review post-fix)

**Evet.** Codex'in 7 maddesi prompt'a entegre edildi:

1. ✓ **sipahi.ld FIX-D düzeltildi** — `.wasm_arena` silinince `_end` ve `__clear_end` ~4MB küçülür (NOLOAD section location counter ilerletir); boot clear süresi azalır
2. ✓ **PolicyEvent::WasmTrap → TaskFault rename** (FIX-I yeni section) — G3'te 5 ref güncellendi; decide_action davranışı invariant
3. ✓ **WASM_HEAP_SIZE explicit silme** (FIX-3 yeni) — G3'te config.rs:83 + verify.rs:687-688
4. ✓ **CI + script cleanup** (FIX-J yeni G8.5 task) — ci.yml guard invert, feature_matrix combo sil, u19_remove_blanket archive banner
5. ✓ **Doc senkronu 8 dosya** (FIX-H genişletildi) — STRUCTURE + 3 docs/* + SIPAHI_SNTM_DESIGN dahil
6. ✓ **ed25519-compact REAL exact pin** (FIX-A güçlendirildi) — G1.0 cargo search + cargo doc validate; placeholder YASAK; patlarsa STOP+RAPORLA
7. ✓ **No-go grep 10 kategori** (FIX-K yeni) — src + Cargo + sipahi.ld + scripts + .github + docs + coverage + invariant + rename + alloc; her biri ayrı pass-fail

Mini-cleanup sprint, ~1 gün scope (G3 + G8.5 + G9 genişlemesi ile ~1.5 güne çıkabilir). 11 G-task (G0..G10 + G8.5):
- Kernel surface ~700 satır azalır
- Dependency tree -1 crate (wasmi tamamen, ed25519-dalek → compact)
- Feature flag -1 (wasm-sandbox)
- Kani proof -13 ile -15
- Pure no_std + no_alloc doctrine compliance
- `_end` ~4MB küçülür → boot clear süresi azalır
- PolicyEvent semantic'i WASM-bağımsız (TaskFault rename)

**Başlama gate'i:**
- [ ] Kullanıcı onayı
- [ ] U-27.5 commit + v1.5.1 tag (önkoşul)
- [ ] Working tree clean
- [ ] G0 audit PASS (Kani 213, TLA 8/8, smoke, cross-isolation 4-gate)
- [ ] G1.0 ed25519-compact REAL version pin tespit edildi (placeholder YASAK)
