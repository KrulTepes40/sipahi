# Sprint U-27.5 — Cross-task PMP runtime ihlal observe + trap isolate hook

**Hedef:** SNTM-R12 runtime ihlal observasyonu. U-27'de **statik kanıt** (Kani `check_ptr_in_profile_rejects_other_task_region` + sntm-validate cross-task overlap reject + runtime `test_pmp_profiles_disjoint`) tamamlandı. Bu mini-sprint **çalışan ihlal + trap → isolate path'i** ile statik kanıtı runtime ile destekler. Süre: **~2-3 saat** (single-task scope, no new tuning).

**Önkoşul:** U-27 commit + tag (v1.5.0) tamamlanmış. Working tree clean. `cargo kani` 213/213, TLA+ 8/8, self-test ALL TESTS PASSED, production smoke 120s clean.

**Codex pre-review kritik düzeltme (timing bug):** Kernel self-test path'i (`tests::run_all`) **scheduler'dan ÖNCE** çalışır ([src/main.rs:113-114](src/main.rs#L113-L114)):
```rust
boot::init();
#[cfg(feature = "self-test")]
tests::run_all();   // line 113 — scheduler START etmedi
boot::start();      // line 114 — scheduler burada başlar
```
Yani `tests::run_all()` içinden `task_hello = Isolated` kontrolü **anlamsız** (task_hello daha hiç execute edilmedi). U-27.5 doğrulaması **kernel self-test değil, runtime QEMU log grep + trap sonrası in-handler state check** olmalı.

---

## 1. Scope

### IN scope (U-27.5 — kapsam DAR)
1. **`cross-isolation-demo` feature flag** — workspace + task_hello + main kernel Cargo.toml; default-off (opt-in); `unexpected_cfgs.check-cfg` listesine de eklenir (clippy `-D warnings` guard)
2. **task_hello deliberate cross-region write** — cfg(feature) altında task_world.data region'ına `write_volatile(0xAA)`; üretim build'inde compile-out
3. **Trap handler runtime marker + IN-HANDLER state check** — `src/arch/trap.rs` mcause 5|7 path'inde `handle_task_fault()` SONRASI state inspection: task_hello (id=2) Isolated mı + task_world (id=3) Ready/Running mı; SADECE her ikisi de doğruysa `[OK] Cross-task PMP isolation enforced...` marker emit
4. **Script-based runtime verification (NOT kernel self-test)** — `scripts/check_cross_isolation.sh` QEMU log grep gate: marker var + marker sonrası `[TICK]` devam ediyor + NF/FATAL/POLICY SHUTDOWN yok. Codex pre-review timing bug için kernel self-test'e EKLENMEZ.
5. **`make run-cross-isolation` target** — feature ile build + QEMU launch + `check_cross_isolation.sh` invocation
6. **coverage.toml SNTM-R12 update** — `deferred = "runtime_observe"` kaldırılır; `required_tests` listesine **script gate adı** eklenir (`check_cross_isolation.sh`), kernel self-test ismi DEĞIL

### OUT of scope (DEFER kalıyor)
| İtem | Hedef | Sebep |
|------|-------|-------|
| Typed IPC codegen | v1.7 SAFE-2 | SNTM design §17.6 |
| Static cap table | v1.7 SAFE-2 | SNTM design §17.5 |
| Binary verifier | v1.8 SAFE-3 | SNTM design §17.3 |
| Task certificate | v1.8 SAFE-3 | SNTM design §17.4 |
| Stack analyzer | v1.9 SAFE-4 | SNTM design §17.7 |
| FPGA bring-up | U-28 | hardware |
| WASM tamamen sil | U-29 | post-v1.5 cleanup |

---

## 2. Invariants — sprint boyunca BOZULMAYACAK

**U-27 carry-forward (14 invariant):**
1-14: bkz. [SPRINT_U27_PROMPT.md §2](SPRINT_U27_PROMPT.md) — **HEPSI KORUNUR**.

**U-27.5 yeni (1 invariant):**

15. **Cross-task PMP runtime ihlal observe** — task A region'ından task B region'ına store → mcause=7 (StoreAccessFault) trap → `handle_task_fault()` → `isolate_task(A)` → task A state=Isolated, task B state=Ready/Running (next tick'te hala runnable). Production build'de `cross-isolation-demo` feature OFF → trap path hiç tetiklenmez.

**No-go regression guard (zorunlu):**
- Production build'de `cross-isolation-demo` feature aktif ETMEDİĞİNI doğrula (`grep -r "cross-isolation-demo" src/ tasks/ Cargo.toml --include="*.toml"` sadece default-off feature deklarasyonlarını göstermeli; `cfg(feature = "cross-isolation-demo")` ifadesi default build'de compile-out olmalı)
- Production smoke (`make run`) hala 0 FATAL/SHUTDOWN/TRAP — feature OFF varsayılan
- Self-test build (`make run-self-test`) hala ALL TESTS PASSED — cross-isolation feature DAHIL EDİLMEZ (sadece `make run-cross-isolation` ile)

---

## 3. Codex Pre-Review Fix List (anticipate)

### FIX-I — Feature scope ayrımı
`cross-isolation-demo` feature **`self-test`'in part'ı DEĞIL**. Sebep: her self-test çalıştırması task_hello'yu isolate ederse, sonraki self-test'ler bozulur. Ayrı invocation path:
- `make run-self-test` → normal test suite (task_hello çalışır, cross-isolation feature YOK)
- `make run-cross-isolation` → cross-isolation-demo feature aktif, scheduler runtime observation (kernel self-test'e GİRMEZ — Codex timing bug fix)

Workspace Cargo.toml'da:
```toml
cross-isolation-demo = []   # opt-in only, NOT in self-test default features
```

### FIX-O — unexpected_cfgs + check-cfg list extension (Codex fix)
Root `Cargo.toml` `unexpected_cfgs` `check-cfg` listesinde `cross-isolation-demo` feature value'su EKSİK olursa, rustc bilinmeyen cfg uyarısı verir → clippy `-D warnings` patlar.

**Mevcut** ([Cargo.toml:85](Cargo.toml#L85)):
```toml
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)', 'cfg(feature, values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "wasm-sandbox", "sntm", "sntm-safe"))'] }
```

**U-27.5'te güncelle**:
```toml
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)', 'cfg(feature, values("test-keys", "multi-hart", "self-test", "trace", "debug-boot", "production-otp", "v2-hal", "wasm-sandbox", "sntm", "sntm-safe", "cross-isolation-demo"))'] }
```

G1'in build success check'i sadece `unknown feature` error'u değil, **`unexpected cfg condition` uyarısı YOK** olmasını da içerir.

### FIX-J — Trap marker placement + IN-HANDLER state check (Codex güçlendirme)
`src/arch/trap.rs` mcause 5|7 path'inde **isolate işleminin tamamlandığı** kanıtı için marker `handle_task_fault()` çağrısından SONRA emit edilmeli. Sadece "fault tetiklendi" demek **YETERSIZ** — full pipeline (trap → policy → isolate_task) çalıştığını + collateral damage olmadığını kanıt:

```rust
5 | 7 => {
    let fault_addr = crate::arch::csr::read_mtval();
    let attacker = crate::kernel::scheduler::current_task_id();

    #[cfg(feature = "debug-boot")]
    { /* mevcut teşhis */ }

    crate::ipc::blackbox::log(...);
    crate::kernel::scheduler::handle_task_fault();   // mevcut isolate path

    // U-27.5 (Codex): IN-HANDLER state verification — isolate path'in
    // GERÇEKTEN çalıştığını + B unaffected kanıtla. Marker SADECE ikisi
    // de doğruysa emit edilir → trap-fired-but-not-isolated bug yakalanır.
    #[cfg(feature = "cross-isolation-demo")]
    {
        use crate::common::types::TaskState;
        let attacker_state = crate::kernel::scheduler::task_state_for_test(attacker);
        let victim_state   = crate::kernel::scheduler::task_state_for_test(3);  // task_world
        let attacker_isolated = matches!(attacker_state, TaskState::Isolated);
        let victim_runnable   = matches!(victim_state, TaskState::Ready | TaskState::Running);

        if attacker_isolated && victim_runnable {
            crate::arch::uart::puts("[OK] Cross-task PMP isolation enforced: task=");
            crate::common::fmt::print_u64(attacker as u64);
            crate::arch::uart::puts(" attempted=0x");
            crate::common::fmt::print_hex(fault_addr);
            crate::arch::uart::println(" REJECTED");
        } else {
            crate::arch::uart::puts("[FAIL] Cross-task PMP isolation BROKEN: attacker=");
            crate::common::fmt::print_u64(attacker as u64);
            crate::arch::uart::puts(" isolated=");
            crate::common::fmt::print_u64(if attacker_isolated { 1 } else { 0 });
            crate::arch::uart::puts(" victim_runnable=");
            crate::common::fmt::print_u64(if victim_runnable { 1 } else { 0 });
            crate::arch::uart::println("");
        }
    }
    0
}
```

`uart::puts` + `fmt::print_{u64,hex}` use ifadelerinin cfg gate'i `any(debug-boot, cross-isolation-demo)` olacak şekilde genişlet.

**Not:** `task_state_for_test` şu an `#[cfg(feature = "self-test")]` gated. U-27.5'te `#[cfg(any(feature = "self-test", feature = "cross-isolation-demo"))]` yap (kapsam minimum genişleme, scheduler internal accessor).

### FIX-K — task_hello violation pozisyonu
`tasks/task_hello/src/main.rs` `_start` içinde **YIELD'den önce** deliberate write. Aksi halde scheduler context switch sırasında task_world'ün PMP profile aktif olabilir → wrong-task attribution. Doğru sıralama:
```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(feature = "cross-isolation-demo")]
    unsafe {
        // task_world.data: 0x80705000 (sipahi.toml task_world[data] base).
        // task_hello PMP profile (entry 8..11) bu adresi KAPSAMIYOR → STORE fault.
        let target = 0x80705000 as *mut u8;
        core::ptr::write_volatile(target, 0xAA);
        // Unreachable: trap → isolate_task(task_hello).
    }
    main_loop()
}
```

### FIX-L — task_world unaffected gözlem
Test: cross-isolation marker'dan sonra task_world state Ready/Running olmalı. **Heartbeat marker yok** (task_world UART region'ı YOK; PMP'sinde UART entry yok). Verify yöntemleri:
1. **Statik state check** (`task_state_for_test(3)`) — simplest, scheduler internal state
2. **Yield count counter** — task_world `counter += 1` her yield'de; scheduler'da read-only accessor ile counter değerini oku, X tick sonra Y ≠ Y_prev olmalı
3. **TICK timer ilerleme** — `[TICK]` timer ticks devam ediyor (en az 5 tick ihlal sonrası)

**Öneri:** (1) + (3) — statik state check + tick continue grep.

### FIX-M — `cargo build` ile feature check
`make run-cross-isolation` target'ı:
```makefile
run-cross-isolation: build-native
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release \
		--features self-test,cross-isolation-demo $(BUILD_STD)
	$(QEMU) \
		-machine virt -nographic -bios none -m 512M -smp 1 \
		-kernel $(KERNEL)
```

task_hello'nun ALSO feature ile build edilmesi gerekiyor — `scripts/build_native_tasks.sh` `cargo build` çağrısına `--features cross-isolation-demo` propagate edilmeli (cross-isolation make target'ı tetiklediğinde).

**Çözüm:** Environment variable `SIPAHI_CROSS_ISOLATION=1` set, build_native_tasks.sh script bu var'a göre cargo build flag eklesin:
```bash
if [ "${SIPAHI_CROSS_ISOLATION:-0}" = "1" ]; then
    TASK_HELLO_FEATURES="--features cross-isolation-demo"
fi
(cd tasks/task_hello && cargo build --release $TASK_HELLO_FEATURES ...)
```

Makefile `run-cross-isolation` target'ı `SIPAHI_CROSS_ISOLATION=1 bash scripts/build_native_tasks.sh` çağırır.

### FIX-N — coverage.toml deferred kaldırma (Codex script gate fix)
U-27'de:
```toml
[requirement.SNTM-R12]
required_tests = ["test_pmp_profiles_disjoint"]
required_proofs = ["check_ptr_in_profile_rejects_other_task_region",
                   "check_ptr_in_profile_symmetric_isolation"]
deferred = "runtime_observe"
deferred_reason = "..."
deferred_target = "U-27.5"
```

U-27.5'te (Codex fix: kernel self-test ismi DEĞIL, script gate ismi):
```toml
[requirement.SNTM-R12]
description = "Cross-task PMP isolation: task A profile reddedir + manifest overlap reject + runtime PMP_PROFILES disjoint assertion + runtime trap → isolate observe (cross-isolation-demo feature, script gate)"
required_tests = ["test_pmp_profiles_disjoint"]
required_scripts = ["check_cross_isolation.sh"]   # U-27.5: script-based runtime gate
required_proofs = ["check_ptr_in_profile_rejects_other_task_region",
                   "check_ptr_in_profile_symmetric_isolation"]
# deferred + deferred_reason + deferred_target — KALDIR
fault_model = "...mevcut + task A violation sonrası B unaffected değil (collateral damage — check_cross_isolation.sh Gate 3 yakalar), trap handler isolate_task çağrılmadı (mcause 5|7 path missing — IN-HANDLER state check FAIL marker yazar), trap marker emit edilmedi (script Gate 1 fail), ya da scheduler post-trap ilerleyemedi (script Gate 3 [TICK] count fail)"
```

**`check_coverage.sh` script extension (Codex fix):** Mevcut script `required_tests` ve `required_proofs` field'larını okur. `required_scripts` yeni — script `scripts/` altında bulunduğunu doğrulayan extension gerekir. Eğer `check_coverage.sh` schema validator değiştirilmek istenmiyorsa, alternatif: `required_tests = ["check_cross_isolation_script_marker"]` (marker dummy ismi) + deferred=integration_gate ile bypass. **Öneri (temiz):** `check_coverage.sh`'a `required_scripts` field handling ekle (~10 satır kod). Schema bump'a değer.

---

## 4. Görev Planı G0..G6

Test-first RED→GREEN. ~2-3 saat toplam.

### G0 — Pre-sprint baseline audit (10dk)
**Önkoşul:** working tree clean (U-27 commit+tag tamamlanmış).

Audit:
- [ ] `git status --short` boş (U-27 v1.5.0 tagged)
- [ ] `cargo kani` → 213/213 PASS
- [ ] `bash Tla+/run_tlc.sh` → 8/8 PASS
- [ ] `timeout 30 make run-self-test 2>&1 | grep "ALL TESTS PASSED"` → marker
- [ ] `timeout 30 make run` → NF/FATAL/POLICY-free
- [ ] `bash scripts/check_coverage.sh` → 14F + 14R PASS
- [ ] `make check` → clippy clean
- [ ] `bash scripts/sntm_sprint_gate.sh` → PASS

**Mevcut state grep gates (FAIL beklenen — U-27.5'te eklenir):**
- `grep -r "cross-isolation-demo" Cargo.toml` → BOŞ (feature daha eklenmedi)
- `grep "cross-isolation-demo" src/arch/trap.rs` → BOŞ (marker daha eklenmedi)

### G1 — Feature flag scaffolding (15dk) [test-first negative]
**Dosyalar:** root `Cargo.toml`, `tasks/task_hello/Cargo.toml`, `tasks/task_world/Cargo.toml` (sentinel propagation)

**Test (RED):**
- `cargo build --features cross-isolation-demo --target riscv64imac-unknown-none-elf --release` → FAIL ("unknown feature")

**Implement (GREEN):**
- Root `Cargo.toml` `[features]` bölümüne ekle:
  ```toml
  cross-isolation-demo = []   # U-27.5: opt-in cross-task PMP runtime ihlal demo
  ```
- `tasks/task_hello/Cargo.toml` `[features]` bölümüne ekle:
  ```toml
  [features]
  cross-isolation-demo = []
  ```
- task_world Cargo.toml dokunulmaz (sadece task_hello violate edecek).

**Test GREEN:** `cargo build --features cross-isolation-demo` → PASS (henüz kullanılmıyor, no warnings).

### G2 — task_hello deliberate cross-region write (20dk) [test-first negative]
**Dosya:** `tasks/task_hello/src/main.rs`

**Test (RED):**
- `cargo build -p task_hello --features cross-isolation-demo --target riscv64imac-unknown-none-elf --release` → PASS (feature recognized)
- ELF disassembly: `rust-objdump -d target/riscv64imac-unknown-none-elf/release/task_hello | head -20` → 0x80700000 region'ına direkt write instruction var (sb veya store)

**Implement (GREEN):**
- `_start` body'sini değiştir (FIX-K):
  ```rust
  #[no_mangle]
  pub extern "C" fn _start() -> ! {
      // U-27.5: Deliberate cross-region write to demonstrate PMP isolation.
      // task_world.data base = 0x80705000 (sipahi.toml task_world[data]).
      // task_hello PMP profile (entry 8..11) bu adresi KAPSAMIYOR.
      // Beklenen: STORE_ACCESS_FAULT (mcause=7) → trap → isolate_task(task_hello).
      // task_world bu sırada DOKUNULMAZ → next tick'te Ready/Running.
      #[cfg(feature = "cross-isolation-demo")]
      unsafe {
          let target = 0x80705000 as *mut u8;
          core::ptr::write_volatile(target, 0xAA);
          // Bu satıra HİÇBİR ZAMAN ulaşılmaz (trap → isolate).
      }
      main_loop()
  }
  ```

**Doğrulama:**
- `cargo build -p task_hello --features cross-isolation-demo --target riscv64imac-unknown-none-elf --release`
- `rust-objdump -d target/.../task_hello`: store instruction (sb/sw) ve 0x80705000 literal görünür
- `cargo build -p task_hello --release` (feature OFF) → store yok (compile-out)

### G3 — Trap marker emit (20dk) [test-first]
**Dosya:** `src/arch/trap.rs`

**Test (RED — mevcut state):**
- Trap path'inde `cross-isolation-demo` marker output YOK
- `grep "Cross-task PMP isolation enforced" src/arch/trap.rs` → boş

**Implement (GREEN):**
- mcause 5|7 path'inde marker ekle (FIX-J):
  ```rust
  5 | 7 => {
      let fault_addr = crate::arch::csr::read_mtval();
      let task_id = crate::kernel::scheduler::current_task_id();

      #[cfg(feature = "debug-boot")]
      {
          let fault_name = if mcause == 5 { "LoadAccessFault" } else { "StoreAccessFault" };
          uart::puts("[TRAP] "); /* mevcut */
      }

      // U-27.5: Cross-isolation-demo marker (self-test grep gate).
      #[cfg(feature = "cross-isolation-demo")]
      {
          crate::arch::uart::puts("[OK] Cross-task PMP isolation enforced: task=");
          crate::common::fmt::print_u64(task_id as u64);
          crate::arch::uart::puts(" attempted=0x");
          crate::common::fmt::print_hex(fault_addr);
          crate::arch::uart::println(" REJECTED");
      }

      // Mevcut handle_task_fault path (isolate'a yönlendiriyor).
      crate::ipc::blackbox::log(...);
      crate::kernel::scheduler::handle_task_fault();
      0
  }
  ```

- `uart` / `fmt::{print_u64, print_hex}` modüllerinin cfg gate'i `any(feature = "debug-boot", feature = "cross-isolation-demo")` olacak şekilde genişlet (sadece use ifadesinde):
  ```rust
  #[cfg(all(not(kani), any(feature = "debug-boot", feature = "cross-isolation-demo")))]
  use crate::arch::uart;
  #[cfg(all(not(kani), any(feature = "debug-boot", feature = "cross-isolation-demo")))]
  use crate::common::fmt::{print_u64, print_hex};
  ```

**Test GREEN:** `cargo build --features cross-isolation-demo --release` → PASS, no warnings.

### G4 — Build pipeline + Makefile target (15dk)
**Dosyalar:** `scripts/build_native_tasks.sh`, `Makefile`

**Test (RED):**
- `make run-cross-isolation` → make target unknown error

**Implement (GREEN):**

**FIX-M build pipeline:**
- `scripts/build_native_tasks.sh` baş tarafına ekle:
  ```bash
  TASK_HELLO_FEATURES=""
  if [ "${SIPAHI_CROSS_ISOLATION:-0}" = "1" ]; then
      TASK_HELLO_FEATURES="--features cross-isolation-demo"
      echo "[native] task_hello build (cross-isolation-demo ENABLED)"
  fi

  (cd tasks/task_hello && cargo build --release $TASK_HELLO_FEATURES 2>&1 | tail -3)
  ```

**Makefile:**
- Yeni target ekle (Codex fix: kernel `--features self-test` GİRMEZ; sadece `cross-isolation-demo`. Self-test feature scheduler önce `tests::run_all` çalıştırırdı, runtime gözlemini gizler):
  ```makefile
  .PHONY: build run clean check kani debug run-self-test regen-pmp build-native run-cross-isolation

  # U-27.5: Cross-task PMP runtime ihlal demo (opt-in, cross-isolation-demo feature).
  # NOT self-test — kernel tests::run_all scheduler önce çalışırdı; bu demo
  # scheduler runtime observation gerektirir. Log /tmp/u275_xi.log'a yazılır,
  # check_cross_isolation.sh ile doğrulanır.
  run-cross-isolation:
  	SIPAHI_CROSS_ISOLATION=1 bash scripts/build_native_tasks.sh
  	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release \
  		--features cross-isolation-demo $(BUILD_STD)
  	timeout 30 $(QEMU) \
  		-machine virt -nographic -bios none -m 512M -smp 1 \
  		-kernel $(KERNEL) 2>&1 | tee /tmp/u275_xi.log || true
  	bash scripts/check_cross_isolation.sh /tmp/u275_xi.log
  ```

  **Önemli (debug-boot UART):** Production default'ta `debug-boot` feature OFF — `[TICK]` markerları compile-out olur, script Gate 3 (post-marker ticks >= 3) başarısız olur. `cross-isolation-demo` build'i UART output'a ihtiyaç duyar. İki seçenek:
  - A) `make run-cross-isolation` target'ı `--features debug-boot,cross-isolation-demo` build et (TICK markerları aktif)
  - B) trap.rs marker ek olarak `cross-isolation-demo` build'inde TICK markerını da emit et (daha cerrah ama daha çok kod)

  **Öneri: A** (basit, sadece `make run-cross-isolation` target'ı için debug-boot feature aktif; `make run` default production unaffected):
  ```makefile
  run-cross-isolation:
  	SIPAHI_CROSS_ISOLATION=1 bash scripts/build_native_tasks.sh
  	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release \
  		--features cross-isolation-demo,debug-boot $(BUILD_STD)
  	timeout 30 $(QEMU) \
  		-machine virt -nographic -bios none -m 512M -smp 1 \
  		-kernel $(KERNEL) 2>&1 | tee /tmp/u275_xi.log || true
  	bash scripts/check_cross_isolation.sh /tmp/u275_xi.log
  ```

**Test GREEN:**
- `timeout 30 make run-cross-isolation 2>&1 | tee /tmp/u275_xi.log` çalışır
- Log içeriği:
  ```
  [native] task_hello build (cross-isolation-demo ENABLED)
  ...
  [BOOT] Task Hello (native): id=2 prio=6 dal=D budget=500K period=50 (SNTM)
  [BOOT] Task World (native): id=3 prio=7 dal=D budget=500K period=50 (SNTM)
  ...
  [OK] Cross-task PMP isolation enforced: task=2 attempted=0x80705000 REJECTED
  ...
  [TICK] #N ... (timer continues — task_world hala Ready)
  ```

### G5 — Script-based runtime verification (25dk) [Codex pre-review fix]
**Dosya:** `scripts/check_cross_isolation.sh` (yeni)

**Önemli (Codex timing bug fix):** Bu test KERNEL SELF-TEST'E EKLENMEZ — `tests::run_all()` scheduler START etmeden ÇALIŞIR, task_hello daha hiç execute edilmemiş olur, state check anlamsız. Runtime verification dış script ile QEMU log üzerinden yapılır.

**Test (RED):**
- `bash scripts/check_cross_isolation.sh` → script yok / exit nonzero

**Implement (GREEN):**

`scripts/check_cross_isolation.sh`:
```bash
#!/usr/bin/env bash
# U-27.5 SNTM-R12 runtime verification — QEMU log grep gate.
#
# Codex pre-review fix: kernel tests::run_all() scheduler START öncesi
# çalıştığı için kernel self-test ile cross-isolation observation FALSE-PASS/
# FAIL eder. Dış grep gate ile QEMU runtime log üzerinden doğrula.
#
# Beklenen runtime sırası:
#   1. Kernel boot, scheduler START.
#   2. task_hello scheduled → _start çağrısı.
#   3. cfg(cross-isolation-demo) deliberate write 0x80705000 → trap mcause=7.
#   4. trap.rs handle_task_fault → isolate_task(2).
#   5. trap.rs IN-HANDLER state check: task_hello=Isolated, task_world=Ready/Running.
#   6. State check PASS → marker emit: "[OK] Cross-task PMP isolation enforced...".
#   7. Scheduler devam: task_world (prio=7) Ready, [TICK] timer ilerler.
#   8. Hiçbir [FAIL]/FATAL/SHUTDOWN/POLICY DEGRADE/SHUTDOWN markerı yok.
set -euo pipefail
cd "$(dirname "$0")/.."

LOG=${1:-/tmp/u275_xi.log}
if [ ! -f "$LOG" ]; then
    echo "FAIL: log file $LOG yok — make run-cross-isolation ile üret"
    exit 1
fi

PASS_MARKER='[OK] Cross-task PMP isolation enforced'
FAIL_MARKER='[FAIL] Cross-task PMP isolation BROKEN'

# Gate 1: PASS markerı en az 1 kez var.
if ! grep -qF "$PASS_MARKER" "$LOG"; then
    echo "FAIL: '$PASS_MARKER' marker not found in $LOG"
    exit 1
fi

# Gate 2: FAIL markerı YOK.
if grep -qF "$FAIL_MARKER" "$LOG"; then
    echo "FAIL: '$FAIL_MARKER' marker found — isolate path broken"
    grep -F "$FAIL_MARKER" "$LOG" | head -5
    exit 1
fi

# Gate 3: marker sonrası en az 3 [TICK] devam ediyor (task_world unaffected,
# scheduler progress kanıtı; tek başına state check değil, ilerleme da gerek).
POST_MARKER_TICKS=$(awk -v m="$PASS_MARKER" '
    BEGIN { found=0; count=0 }
    found && /\[TICK\]/ { count++ }
    index($0, m) > 0 { found=1 }
    END { print count }
' "$LOG")
if [ "$POST_MARKER_TICKS" -lt 3 ]; then
    echo "FAIL: marker sonrası sadece $POST_MARKER_TICKS [TICK] var (>=3 beklenen)"
    exit 1
fi

# Gate 4: NF / FATAL / POLICY SHUTDOWN markerı YOK.
if grep -qE 'FATAL|\[NF\]|\[POLICY\] SHUTDOWN' "$LOG"; then
    echo "FAIL: panic/halt marker found"
    grep -E 'FATAL|\[NF\]|\[POLICY\] SHUTDOWN' "$LOG" | head -5
    exit 1
fi

echo "PASS: cross-task PMP isolation runtime observed"
echo "  - Marker: '$PASS_MARKER' ✓"
echo "  - Post-marker ticks: $POST_MARKER_TICKS (>=3) ✓"
echo "  - No FATAL / NF / POLICY SHUTDOWN ✓"
exit 0
```

`chmod +x scripts/check_cross_isolation.sh`.

**Test GREEN:**
- `make run-cross-isolation` log üretir → `bash scripts/check_cross_isolation.sh /tmp/u275_xi.log` → `PASS:`
- Negative test (manuel): trap.rs marker emit'i sil → script `FAIL: 'marker not found'`
- Negative test (manuel): handle_task_fault yorum satırı → state check `attacker_isolated=0` → FAIL marker emit edilir → script `FAIL: 'BROKEN' marker found`

**Not:** kernel `src/tests/mod.rs` DOKUNULMAZ. `test_cross_task_isolation_enforced` EKLENMEZ. U-27'deki `test_pmp_profiles_disjoint` (statik kanıt) yerinde kalır.

### G6 — Coverage update + verification battery (20dk)

**FIX-N coverage.toml:**
- SNTM-R12 entry'sinden `deferred = "runtime_observe"`, `deferred_reason`, `deferred_target` SİL
- `required_scripts = ["check_cross_isolation.sh"]` ekle (kernel test ismi DEĞIL)
- `description` güncelle (runtime observe + script gate dahil)
- `fault_model` extend (collateral damage + script Gate 1-4 senaryoları)
- `scripts/check_coverage.sh`'a `required_scripts` field handling ekle (script `scripts/` altında varlığını doğrula)

**Verification battery:**
1. `cargo kani` → **213/213 PASS** (yeni Kani proof eklenmedi, U-27 baseline korunur)
2. `bash Tla+/run_tlc.sh` → **8/8 PASS** (TLA+ değişmedi)
3. `timeout 30 make run-self-test` → ALL TESTS PASSED (cross-isolation feature OFF, U-27 14 self-test korunur, **R12 kernel test'i `test_pmp_profiles_disjoint` PASS** — statik kanıt korunur)
4. `make run-cross-isolation` → log üretir, script otomatik çağrılır:
   ```
   PASS: cross-task PMP isolation runtime observed
     - Marker: '[OK] Cross-task PMP isolation enforced' ✓
     - Post-marker ticks: N (>=3) ✓
     - No FATAL / NF / POLICY SHUTDOWN ✓
   ```
5. `timeout 30 make run` (production) → NF/FATAL/POLICY-free, **`[OK] Cross-task PMP isolation enforced` marker YOK** (cross-isolation-demo feature default-off, compile-out)
6. `bash scripts/check_coverage.sh` → 14F + 14R PASS (R12 deferred kalktı, `required_scripts` doğrulandı)
7. `make check` (clippy `-D warnings`) → PASS (`check-cfg` listesinde `cross-isolation-demo` var, FIX-O)
8. `bash scripts/sntm_sprint_gate.sh` → PASS

**No-go regression guard (Codex hardening):**
```bash
# Production build cross-isolation feature içermez (default-off)
! grep -E '^\s*default\s*=.*"cross-isolation-demo"' Cargo.toml
# self-test feature listesinde de YOK
! grep -E '^\s*self-test\s*=.*"cross-isolation-demo"' Cargo.toml
# Üretim çıktısında marker görünmez
timeout 10 make run 2>&1 | tee /tmp/u275_prod_smoke.log
! grep "Cross-task PMP isolation enforced" /tmp/u275_prod_smoke.log
# Üretimde [FAIL] PASS marker da görünmemeli
! grep "Cross-task PMP isolation BROKEN" /tmp/u275_prod_smoke.log
# Kernel self-test path'inde cross-isolation feature gate YOK (timing bug guard)
! grep -E "cross-isolation-demo" src/tests/mod.rs
```

---

## 5. Doctrine Reminder

- **NO auto-commit** — her commit için ayrı onay iste
- **Test-first RED→GREEN**
- **Production unaffected** — `cross-isolation-demo` default-off, feature compile-out
- **Self-test default unaffected** — `make run-self-test` cross-isolation feature içermez
- **Statik kanıt korunur** — U-27'deki Kani proof'ları + sntm-validate negative test'leri DEĞIŞMEZ
- **NO destructive git** — force push, reset, branch-D YOK

---

## 6. Final Report Template

```markdown
## Sprint U-27.5 — Final Report

### Completed
- G0: baseline audit (U-27 v1.5.0 clean)
- G1: cross-isolation-demo feature flag (Cargo.toml × 2 + check-cfg list, FIX-O)
- G2: task_hello deliberate write — tasks/task_hello/src/main.rs cfg(feature)
- G3: trap.rs mcause 5|7 IN-HANDLER state check + conditional marker (Codex hardening)
- G4: build pipeline (SIPAHI_CROSS_ISOLATION env) + Makefile run-cross-isolation
- G5: scripts/check_cross_isolation.sh — QEMU log grep gate (Codex timing bug fix: NOT kernel self-test)
- G6: coverage.toml SNTM-R12 (deferred kaldırıldı, required_scripts eklendi) + check_coverage.sh schema extension

### Verification metrics
- Kani: 213/213 PASS (no delta)
- TLA+: 8/8 PASS (no delta)
- Self-test (normal `make run-self-test`): ALL TESTS PASSED — cross-isolation-demo OFF, U-27 14 test korunur
- Cross-isolation (`make run-cross-isolation`): script `PASS: cross-task PMP isolation runtime observed` (marker + post-marker ticks ≥ 3 + no FATAL)
- Production smoke: NF/FATAL/POLICY-free, cross-isolation marker YOK
- Coverage: 14F + 14R, SNTM-R12 deferred kalktı, required_scripts schema PASS
- Clippy `-D warnings`: PASS (FIX-O check-cfg list)
- SNTM gate: PASS

### Invariant audit
- U-27 1..14: korunur (kernel self-test path'inde YENİ test EKLENMEDİĞİ için U-27 R11/R14 self-test'leri etkilenmez)
- U-27.5 #15: runtime trap → in-handler state check → marker → script gate GREEN

### Commit önerisi (NO auto-commit)
sprint-u27.5: SNTM-R12 runtime ihlal observe + trap isolate hook
- cross-isolation-demo feature (opt-in, default-off; check-cfg list extended)
- task_hello deliberate write task_world.data (cfg-gated)
- trap.rs mcause 5|7 IN-HANDLER state check (Codex hardening: marker emit
  SADECE attacker=Isolated + victim=Ready/Running ise; aksi halde [FAIL]
  marker emit edilir, script gate yakalar)
- make run-cross-isolation target (--features cross-isolation-demo,debug-boot)
- scripts/check_cross_isolation.sh — QEMU log grep gate, 4 invariant
  (marker var, FAIL marker yok, post-marker [TICK] ≥ 3, no FATAL/NF/POLICY)
- scripts/check_coverage.sh schema extension: required_scripts field
- coverage.toml SNTM-R12 deferred kaldırıldı (runtime observe complete)

NOT: tests/mod.rs DOKUNULMAZ (Codex timing bug fix: kernel self-test
scheduler START öncesi çalışır, runtime observation için uygun değil).

### Tag önerisi
v1.5.1 — SNTM-R12 runtime observe (statik kanıt + runtime observation full)
veya v1.5.0 patch (commit eklenir, tag yenilenmez)
```

---

## 7. Audit (Kontrol)

| Soru | Cevap |
|------|-------|
| U-27 statik kanıtla redundant değil mi? | HAYIR — runtime observasyon kanıt tamamlayıcı: trap path → isolate_task çağrılı + collateral damage YOK |
| Production default'a sızar mı? | HAYIR — cross-isolation-demo feature default-off; cfg gate'leri compile-out (FIX-O check-cfg list ile clippy guard) |
| Self-test bozulur mu? | HAYIR — `make run-self-test` feature içermez; tests/mod.rs DOKUNULMAZ (Codex timing bug fix) |
| Codex'in timing bug uyarısı nasıl çözüldü? | Kernel self-test KALDIRILDI; runtime verification dış script (scripts/check_cross_isolation.sh) ile QEMU log üzerinden yapılır; trap.rs IN-HANDLER state check + conditional marker emit |
| task_hello her zaman isolate olur mu? | EVET, ama SADECE cross-isolation-demo feature ile build edilirse |
| task_world unaffected nasıl kanıtlanır? | trap.rs IN-HANDLER `task_state_for_test(3) == Ready/Running` check + script Gate 3 (post-marker `[TICK]` ≥ 3 = scheduler ilerliyor, task_world preempted veya running) |
| trap.S/context.S değişikliği var mı? | YOK — sadece trap.rs (Rust) IN-HANDLER state check + marker, kernel asm path dokunulmaz |
| Yeni Kani proof var mı? | YOK — U-27'de eklendi (`check_ptr_in_profile_rejects_other_task_region` + `_symmetric`) |
| Yeni TLA+ invariant var mı? | YOK — runtime observasyon spec seviyesinde değişmez (state machine same) |
| coverage.toml schema değişir mi? | EVET — `required_scripts` yeni field, `check_coverage.sh` schema extension (~10 satır) |
| Marker'sız [FAIL] yakalanır mı? | EVET — trap.rs IN-HANDLER state check FAIL ise `[FAIL] Cross-task PMP isolation BROKEN` emit, script Gate 2 yakalar |
| Production marker leak guard? | EVET — `make run` log'unda hem `[OK]` hem `[FAIL]` marker'ı görünmemeli (feature compile-out); G6 no-go regression `! grep` ile kontrol eder |

---

## 8. Hazır mı?

**Evet.** Mini-sprint, single-task scope, no new tuning. 6 G-task ~2-3 saat. U-27 baseline (Kani 213, TLA 8/8, smoke 120s clean) korunur. Yeni invariant #15 runtime observation ile complete.

**Başlama gate'i:**
- [ ] U-27 commit + v1.5.0 tag (kullanıcı onayı)
- [ ] Working tree clean
- [ ] G0 audit PASS
