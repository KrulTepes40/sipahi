# Sipahi v1.0 — Claude Code Comprehensive Audit

> Read-only audit. Hiçbir kaynak dosya değiştirilmedi.
> Tarih: 2026-05-01
> Audit hedefi: AMCI multi-hart geçişi öncesi son kapsamlı denetim.
> Yöntem: 3 pass'te tüm `src/`, linker, CI, doküman, TLA+, deny.toml, Kani harness'leri end-to-end okundu.

---

## Birleşik Özet

| Pass | Kapsam | CRITICAL | HIGH | MEDIUM | LOW | INFO |
|---|---|---|---|---|---|---|
| 1+2 | Pass 1 broad + Pass 2 detailed (kritik path dosyaları) | 0 | 2 | 7 | 5 | 4 |
| 3 | Kalan 18 dosya derin okuma + senior-bar kalite analizi | 0 | 1 | 4 | 7 | — |
| **Toplam** | **42 dosya, 9.5K LOC** | **0** | **3** | **11** | **12** | **4** |

**CRITICAL bulgu yok.** Vanilla PMP'nin MAC_KEY/PMP_SHADOW'u U-mode'a açık bırakması ARCHITECTURE.md §Known Limitations'da explicit kabul edilmiş — yeniden bulgu yapılmadı.

**HIGH bulguların 2'si production deployment'ı doğrudan etkiliyor:**
- H1: POST CLINT/misa kontrolleri WARN-only, HALT olmuyor.
- H2: Default feature build secure boot ve capability MAC key provisioning'i sessizce devre dışı bırakıyor.
- H3: Kani BLAKE3 stub key material'i çıktıya kopyalıyor; "different key → different hash" tipi prooflar trivial geçiyor.

---

## İçindekiler

1. [Pass 1+2 — Genel Audit Raporu](#pass-12--genel-audit-raporu)
2. [Pass 3 — Kalan Dosyalar + Kod Kalitesi Analizi](#pass-3--kalan-dosyalar--kod-kalitesi-analizi)
3. [Birleşik Bulgu Tablosu](#birleşik-bulgu-tablosu)
4. [Önerilen v1.5 / v2.0 Sertleştirme Listesi](#önerilen-v15--v20-sertleştirme-listesi)
5. [Performance Baseline (AMCI Öncesi Referans)](#performance-baseline-amci-öncesi-referans)
6. [Metrik Özet](#metrik-özet)

---

## Pass 1+2 — Genel Audit Raporu

### Özet

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 2 |
| MEDIUM | 7 |
| LOW | 5 |
| INFO | 4 |

Beklenenin aksine CRITICAL bulgu çıkmadı. Vanilla PMP'nin MAC_KEY/PMP_SHADOW'u U-mode'a açık bırakması ARCHITECTURE.md §Known Limitations'da explicit kabul edilmiş — A.0 threat model kuralı gereği yeniden bulgu yapılmadı. HIGH bulguların ikisi de production deployment'ı doğrudan etkiliyor: POST'taki tek WARN-only path ve default feature setinin secure boot / capability MAC anahtarını sessizce devre dışı bırakması.

### Threat Model Kalibrasyonu

A.0'a göre kapsamda olanlar: hostile WASM, kötü niyetli U-mode task, hardware fault injection, side-channel timing. Kapsam dışı: JTAG, rowhammer, Spectre, kernel-içi compromise.

Aşağıdaki konular Known Limitation kapsamında yeniden raporlanmadı:
- Vanilla PMP MAC_KEY/PMP_SHADOW U-mode'dan okunabilir (ARCHITECTURE.md:132-148)
- WCET değerleri estimated, FPGA ölçümü pending (ARCHITECTURE.md:99-108)
- Kani assembly/CSR/crypto kapsam dışı (ARCHITECTURE.md:71-86)
- TLA+ refinement mapping yok (ARCHITECTURE.md:87-97)
- Single-hart only (compile_error guard mevcut)

---

### HIGH Bulgular

#### H1 — POST CLINT timer arızası HALT yerine WARN üretiyor
- **[KAT]** Correctness / Boot
- **[DOS]** [src/tests/mod.rs:566-577](src/tests/mod.rs)
- **[BUL]** POST'taki tüm diğer sağlık kontrolleri (CRC32, PMP, mtvec, BLAKE3, Ed25519, mstatus) `halt_system()` ile boot'u durdurur, ancak CLINT mtime ilerlemiyorsa sadece "WARN: mtime not advancing" yazılır + blackbox kaydı atılır ve boot devam eder.
- **[ETK]** Saldırgan/donanım hatası mtime register'ını dondurursa: timer interrupt tetiklenmez → schedule() çağrılmaz → PMP shadow integrity check de bir daha çalışmaz. Tüm safety mekanizması (watchdog, budget, lockstep PMP verify) sessizce ölü hale gelir. Aynı dosyadaki misa kontrolü (lines 583-604) de aynı asimetriye sahip — non-conforming ISA tespitinde sadece WARN.
- **[ÖNR]** İki kontrolü de `halt_system("[POST] FAIL: CLINT timer dead — HALT")` ile sertleştirin. Grace period gerekiyorsa 3 ardışık örnekleme yapın (`while k < 3 { ... }`) ama nihai sonuç WARN değil HALT olmalı.
- **[KNT]** tests/mod.rs:566-577 üst-orta kısmı ve 583-604 misa bloğu doğrudan okunabilir. CRC32 testinde (lines 477-479) ve PMP testinde (lines 483-486) `halt_system` kullanılırken CLINT'te kullanılmıyor.

#### H2 — Default feature build secure boot + capability MAC key provisioning'i sessizce atlıyor
- **[KAT]** Security / Boot
- **[DOS]** [Cargo.toml:9](Cargo.toml), [src/boot.rs:35-61](src/boot.rs)
- **[BUL]** `default = ["fast-crypto", "fast-sign"]`. `make build` (Makefile:13-14) `--release` ile bu default'u kullanır → `test-keys` feature'ı KAPALI → `provision_key()` (boot.rs:35) ve `secure_boot_check()` (boot.rs:47) tamamen compile-out edilir. Sonuç: production binary boot ettiğinde:
  1. Kernel imza doğrulaması çalışmaz.
  2. KEY_READY = false kalır → `validate_full()` her token için `false` döner (broker.rs:125-127).
  3. cap_invoke fast path `validate_cached()` kullanır; cache hiç doldurulmadığı için tüm cap_invoke E_NO_CAPABILITY ile döner (dispatch.rs:253-296). 3 ardışık fail → CapViolation policy (dispatch.rs:276) → ISOLATE.
- **[ETK]** README.md:13 ve ARCHITECTURE.md:36-46 "Capability-based access control" + "Secure boot" özelliklerini v1.0 listesinde sayar; gerçekte default build'de ikisi de etkisiz. Production deploy yapan biri bu durumun farkında olmadan gemiye binari indirebilir. boot.rs:43 yorumu "fail-closed davranış" diyor — doğru ama kullanıcıya ne ettiği görünmez.
- **[ÖNR]** Üç seçenekten biri: (a) production-keys feature ekle ve default'a koy (HSM/OTP entegrasyonu yokken QEMU test key'leriyle aynı path); (b) test-keys olmadan secure_boot_check çağrısı zorunlu hale getir, public key linker symbol olarak gelsin; (c) boot.rs'te `#[cfg(not(any(feature = "test-keys", feature = "production-otp")))] compile_error!("...")` ekle ki "default-features=false" olmadan build edilemesin. README/ARCHITECTURE'a "v1.0 default build secure boot devre dışıdır" notu da ekleyin.
- **[KNT]** `make build` çıktısında provision_key cağrısı yok (boot.rs:35 sadece `#[cfg(feature = "test-keys")]` altında). Cargo.toml:9 default'u açık. dispatch.rs:267-294 fail counter mekanizması — ardışık 3 fail isolate.

---

### MEDIUM Bulgular

#### M1 — mcounteren açıkça ayarlanmıyor (U-mode timing side-channel'a açık olabilir)
- **[KAT]** Security / CSR
- **[DOS]** [src/arch/csr.rs](src/arch/csr.rs) (yok), [src/boot.rs](src/boot.rs) (yok)
- **[BUL]** Kod tabanında `mcounteren` yazma yok. RISC-V spec'te bu CSR U-mode'un `rdcycle/rdtime/rdinstret` erişimini kontrol eder. Reset değeri implementation-defined; QEMU virt + bazı bootloader'lar U-mode counter erişimini açık başlatır.
- **[ETK]** Capability MAC validation `ct_eq_16` ile constant-time yapılıyor (broker.rs:197-207) ama U-mode `rdcycle` çekebiliyorsa BLAKE3 keyed_hash sürecindeki cache hit/miss veya MAC mismatch zamanlaması ölçülebilir. ARCHITECTURE.md:36-46'da "side-channel timing attack" tehdit listesinde — kapsam içi.
- **[ÖNR]** boot.rs::init() içine `unsafe { asm!("csrw mcounteren, zero") }` ekleyin. POST'ta da `read_mcounteren() == 0` doğrulayın. Eğer cycle counter'a U-mode erişim gerekiyorsa (genelde değil), açıkça gerekçeleyin.
- **[KNT]** `grep -rn "mcounteren" src/` boş döner. Default reset davranışı QEMU-version'a bağımlı.

#### M2 — medeleg/mideleg açıkça sıfırlanmıyor / doğrulanmıyor
- **[KAT]** Security / CSR
- **[DOS]** [src/boot.rs](src/boot.rs), [src/tests/mod.rs:471-606](src/tests/mod.rs)
- **[BUL]** M-only kernel için (S-mode yok) medeleg/mideleg sıfır olmalı. Kod hiçbir yerde bu CSR'ları yazmıyor veya POST'ta okumuyor. RISC-V spec'inde reset değeri implementation-defined; bootloader veya firmware non-zero set etmişse exception delegation S-mode'a yönlendirilir ve crash/UB.
- **[ETK]** Sipahi'de S-mode yok → delegated trap kayıp olur. mtvec asla çağrılmaz, sistem tamamen ölü kalır (ya da delege edilen exception class tipine göre QEMU'da illegal-state davranışı). Boot ROM pasifsa görünmez ama hardened kernel doctrine açıkça sıfırlamalı + doğrulamalı.
- **[ÖNR]** boot.rs init'te `csrw medeleg, zero; csrw mideleg, zero`. POST'a okuma + assert ekleyin (mtvec=0 kontrolüyle simetrik).
- **[KNT]** Tüm `medeleg|mideleg` arama boş.

#### M3 — write_mtvec mode bit'lerini açıkça maskelemiyor
- **[KAT]** Correctness / CSR
- **[DOS]** [src/arch/csr.rs:81-84](src/arch/csr.rs), [src/boot.rs:15](src/boot.rs)
- **[BUL]** `write_mtvec(addr)` ham `addr`'ı `csrw mtvec, {addr}` ile yazıyor. mtvec[1:0] mode field — 0=direct, 1=vectored, ≥2 reserved. trap_entry .align 4 olduğundan sembol adresi {1:0]=0 (direct), ancak bu implicit. Linker reorganization, symbol re-alignment veya ileride farklı entry point eklenmesi durumunda mode bit'i kazara 1 olabilir.
- **[ETK]** mtvec[1:0]=1 olursa vectored mode aktif ve trap dispatch yanlış adrese atlar. Sistem ya boot edemez ya da silent corruption.
- **[ÖNR]** `pub fn write_mtvec(addr: usize) { let val = (addr & !0x3) | 0; ... }` veya `write_mtvec(addr: usize, mode: MtvecMode)` API. POST'a `read_mtvec() & 0x3 == 0` da ekleyin (şu an sadece `mtvec != 0` test ediliyor — tests/mod.rs:511-515).
- **[KNT]** csr.rs:81 doğrudan yazma; boot.rs:15 trap_entry pointer'ı geçiyor; tests/mod.rs:511 sadece nonzero kontrol.

#### M4 — Linker script sabitleri ↔ config.rs sabitleri compile-time bağlı değil
- **[KAT]** Correctness / Build
- **[DOS]** [sipahi.ld:69](sipahi.ld), [sipahi.ld:84](sipahi.ld), [src/common/config.rs](src/common/config.rs)
- **[BUL]** Linker `. += 16384;` (kernel stack) ve `ALIGN(8192)` (.task_stacks) hardcoded. config.rs::KERNEL_STACK_SIZE=16384, TASK_STACK_SIZE=8192. İki yerde aynı sayı; build.rs veya const_assert ile bağlı değil. WASM_HEAP_SIZE de linker yorumunda referans veriliyor ama doğrulanmıyor.
- **[ETK]** Sprint 13'te 4KB→16KB değişiminde olduğu gibi gelecekte config değişirse linker güncellenmezse stack overflow'lar PMP-corrupted bölgeye yazar. PMP shadow check yakalar (degrade tetiği) ama root cause invisible.
- **[ÖNR]** build.rs ekle: linker script'i template'den üret veya `const _: () = assert!(...)` ile linker symbol address'leri runtime check. Alternatif: config.rs'i .ld'ye `INCLUDE config.ld` ile çek (cargo-binutils desteği gerekli).
- **[KNT]** sipahi.ld:69 ve config.rs:32 ayrı yerlerde aynı sayı; cross-reference yok.

#### M5 — TaskContext (128 byte) layout için compile-time size assertion yok
- **[KAT]** Correctness / FV
- **[DOS]** [src/kernel/scheduler/mod.rs:31-49](src/kernel/scheduler/mod.rs), [src/arch/context.S:23-76](src/arch/context.S)
- **[BUL]** context.S `sd ra, 0(a0); sd s0, 16(a0); ...; sd mstatus, 120(a0)` ile sabit offset'lerle TaskContext'e yazıyor. Yapı 16 alan × 8 byte = 128 byte. Ancak `const _: () = assert!(core::mem::size_of::<TaskContext>() == 128)` benzeri compile-time guarantee yok. config.rs sadece TRAP_FRAME_SIZE-USER_SP_OFFSET ilişkisini doğruluyor (config.rs:56-60).
- **[ETK]** TaskContext'e bir alan eklenirse veya repr(C) padding değişirse, switch_context bitişik task'ın belleğini siler. Cross-task corruption silent.
- **[ÖNR]** scheduler/mod.rs sonuna ekle: `const _: () = assert!(core::mem::size_of::<TaskContext>() == 128); const _: () = assert!(core::mem::offset_of!(TaskContext, sp) == 8);` vb. her kritik offset için.
- **[KNT]** scheduler/mod.rs ve context.S'te TaskContext layout var; size assertion yok.

#### M6 — IPC mesaj integrity (CRC32) opt-in, kernel kanal seviyesinde dayatmıyor
- **[KAT]** Security / IPC
- **[DOS]** [src/ipc/mod.rs:42-62](src/ipc/mod.rs), [src/kernel/syscall/dispatch.rs:304-426](src/kernel/syscall/dispatch.rs)
- **[BUL]** `IpcMessage::set_crc()` U-mode caller'ın görevi. sys_ipc_send mesajı `core::ptr::read_volatile` ile okuyup ring buffer'a kopyalıyor; CRC hesaplanmıyor. sys_ipc_recv'de verify_crc çağrılmıyor — receiver task'ın sorumluluğunda.
- **[ETK]** Hostile producer task corrupted mesaj gönderir; consumer task verify_crc çağırmazsa (genel pattern: kullanım kolaylığı için skip) verisi bozuk şekilde kabul eder. Threat model'de hostile U-mode task var → bu gap. ARCHITECTURE.md:42 "CRC32 IPC integrity" diyor ama kernel-enforce değil.
- **[ÖNR]** İki seçenek: (a) sys_ipc_send mesajı kopyaladıktan sonra kernel `set_crc` çağırsın → sys_ipc_recv `verify_crc` zorunlu, fail E_CORRUPTED. WCET cost ~480c (tick budget içinde). (b) `IPC_REQUIRE_CRC` config flag ile opt-in modu pekiştirin ama default-on.
- **[KNT]** dispatch.rs:339-343 read_volatile kopyala → ch.send. Hiçbir yerde kernel set_crc çağrılmıyor.

#### M7 — Capability validate_full ordering: nonce write MAC verify sonrası — replay window minimal ama belgesiz
- **[KAT]** Security / Capability
- **[DOS]** [src/kernel/capability/broker.rs:118-164](src/kernel/capability/broker.rs)
- **[BUL]** Sıralama: cache→KEY_READY→owner_match→nonce_read→expiry→MAC_compute→ct_eq→nonce_write+cache_insert. MIE=0 single-hart context'inde re-entrancy yok, dolayısıyla TOCTOU yok. Ancak test/HSM yolunda "monoton nonce" iddiası "her başarılı validate sonrası güncellenir" şeklinde dokümante edilmemiş. Belirli bir MAC valid + expired token kombinasyonunda LAST_NONCE güncellenmez (line 153 sadece success path) → aynı MAC ileri tarihli expiry ile gönderilirse yine kabul.
- **[ETK]** Praktik exploit dar: aynı task'ın başarılı bir token'ını ele geçiren attacker, expiry'i ileri attıramaz çünkü MAC header expiry'i içeriyor (header_bytes - token.rs okumadım ama olağan yapı). Risk konseptüel: nonce write semantiği tek satırda belgesiz.
- **[ÖNR]** broker.rs:139-141 yorumuna "Nonce update only on full success — expired-but-MAC-valid path also updates nonce to prevent same-token re-replay after expiry rollover" notu ekleyin VE kodu da öyle düzeltin (expiry fail → nonce update).
- **[KNT]** broker.rs:139-156 tek başarı path'inde nonce update.

---

### LOW Bulgular

#### L1 — Kani harness sayısı dokümanda 200, gerçekte 201
- [README.md:88](README.md), [ARCHITECTURE.md:58](ARCHITECTURE.md), [docs/sipahi_features_*](docs/) → "200"; `grep -rn "kani::proof" src/ | wc -l` → 201. Trivial drift.

#### L2 — Cargo.toml caret dependency versioning, exact pin yok
- [Cargo.toml:22-26](Cargo.toml): `wasmi = "1.0.9"` (caret), `blake3 = "1"` (≥1.0 <2.0), `ed25519-dalek = "2"` (≥2.0 <3.0). Cargo.lock pinli olduğundan reproducible build sağlanır, ancak safety-critical doctrine için `=1.8.4` stili explicit pin daha temiz. CI'da `--locked` continue-on-error olduğu için (ci.yml:67) drift CI tarafından yakalanmaz.

#### L3 — main.rs versiyon banner v1.5 yazıyor, audit hedefi v1.0, Cargo version "0.1.0"
- [src/main.rs:81](src/main.rs): `"  Sipahi Microkernel v1.5"`. Bu audit "v1.0" hedefli, Cargo.toml:3 `version = "0.1.0"`. Ya v1.0/v1.5/0.1.0 üçlüsünü tek noktaya hizalayın, ya banner'ı `env!("CARGO_PKG_VERSION")`'den üretin.

#### L4 — ipc/blackbox.rs ve diğer 14 unsafe modülde "MIE=0 in trap context" SAFETY notu — context'in dışında çağrılabilen path'ler için doğrulama eksik
- broker.rs:172, scheduler.rs:512 vb. SAFETY yorumları "MIE=0 in trap context" diyor. boot sequence'de (init() öncesi mret henüz yok) bu doğru ama compile-time enforce edilmiyor. Yanlış yerden çağrı sessizce derlenir.
- Type-level guard (örn. `TrapToken<'a>` parametre) v2.0 hardening listesi.

#### L5 — Watchdog window_min=3 hardcoded, task-spesifik override yok
- config.rs::WATCHDOG_WINDOW_MIN=3. Budget'ı 1 tick'lik task'lar bu window violation tetikleyebilir. v1.0 task config'i (boot.rs) period_ticks=10 olduğu için sorun değil ama gelecekte ekstrema task eklenirse sürpriz.

---

### INFO

- **I1:** TLA+ specs (7 dosya — SipahiPolicy, SipahiScheduler, SipahiCapability, SipahiIPC, SipahiWatchdog, SipahiDegradeRecover, SipahiBudgetFairness) repo'da mevcut, README "35,770 distinct states" iddia ediyor — TLC log dosyası repo'da yok, manuel tutulan iddia. CI'da TLC step yok.
- **I2:** CI pipeline `--locked` ve `unsafe documentation` kontrollerini `continue-on-error: true` ile bilgilendirici tutuyor (ci.yml:54-78); reproducible build verification (double-build + sha256 compare) yok.
- **I3:** scheduler/mod.rs::schedule() 174 satır (218-391); birden fazla phase content tek fonksiyonda. Doğrulama / readability açısından phase-helper fonksiyonlara bölünmesi önerilir, ancak ardından inline'lanması gerekir (WCET stabilitesi için). Mevcut yapı belgesi açık.
- **I4:** sandbox/mod.rs::has_float_opcodes (lines 241-266) 0xFC prefix sub-opcode ≤0x07 (trunc_sat) için ek scan yapıyor — false negative testi negative regression suite'de yok; "0xFC 0x80 0x01 ..." (LEB128 sub-opcode > 127) edge case beklenmedik tarafa kaçabilir mi? skip_instruction içinde aynı kontrol kapsamlı görünüyor.

---

### Past-Bug Regression Matrix

| Sprint | Düzeltilen Bug | Regression Guard | CI Catch? |
|---|---|---|---|
| U-16 | is_valid_user_ptr tüm ptr kabul | Kani Proof 157 (any_address_default_deny) + tests `test_cross_task_pointer_rejected` | ✓ |
| U-16 | Token owner mismatch | Kani Proof token_owner_mismatch_always_rejected (broker.rs:240) + `test_token_owner_mismatch_neg` | ✓ |
| U-16 | IPC channel default allow | Kani Proof unassigned_channel_denies_any_caller (ipc/mod.rs:246) + `test_ipc_wrong_owner_rejected` | ✓ |
| U-16 | Watchdog Ready task cezalandırma | scheduler.rs:286-310 sadece Running'de artırma + INFO `info_ready_task_watchdog` | ⚠ INFO-only |
| U-16 | schedule() tek task güvenlik skip | scheduler.rs:225-227 Phase 2 sonrası early return; QEMU runtime test'i tek task path'i kapsamıyor | ⚠ |
| U-16 | Allocator wrapping_add | Kani Proof bump_allocator_offsets_never_overlap + `test_allocator_overflow` | ✓ |
| U-17 | Lockstep CSE optimize | policy.rs:142-185 black_box fence — disassembly check `verify-ct-eq.sh` informational | ⚠ |
| U-18 | task_trampoline NF | QEMU self-test NF-marker grep (CI ci.yml:158) | ✓ |
| U-19 | task_trampoline reg leak | context.S:112-127 16 register clear; runtime test yok | ✗ |
| U-19 | helper inline scheduler | Kani Proof 71/95 helper kullanıyor | ✓ |

⚠/✗ olanlar: regression guard yetersiz veya yok → bug reintroduce edilirse sessizce geri gelebilir. U-19 register clear için bir negative test ekleyin (örn. trap'ten dönen U-mode'da a0/t6 baseline değerini göster).

---

### Attack Scenario Walkthrough Sonuçları

**Scenario 1 — WASM sandbox escape:**
1. 0xFC saturating trunc gizleme → sandbox/mod.rs:250-258 yakalar (sub ≤ 0x07 reject) ✓
2. Allocator overflow → checked_add + arena bound (allocator.rs:55-73) ✓
3. Sonsuz loop → fuel metering (sandbox/mod.rs:402, wasmi store.set_fuel) ✓
4. **AÇIK:** Fuel exhaustion'da policy isolation otomatik mi? sandbox/mod.rs:412 SandboxError döner; çağıran tarafın isolate çağırması gerekiyor. Test koduyla doğrulanmış (tests/mod.rs:402-410) ama production WASM dispatch path'i runtime'da gerçek modül çalıştırmıyor — v2.0.

**Scenario 2 — Compromised U-mode task:**
1. Cross-task pointer → task_stack_range (scheduler.rs:790-803) reject ✓
2. Sealed channel impersonation → can_send/can_recv default deny + seal_channels (boot.rs:91-97) ✓
3. **AÇIK:** Boot sırasında IPC seal başarısız olursa boot.rs:96 `halt_system` çağrılıyor — fail-closed ✓. Ancak default features test-keys olmadığı için cap_invoke zaten devre dışı (H2 finding).
4. Illegal instruction privilege escalation → trap.rs:138-140 verify_mpp_is_user_mode (M-mode'a yükselme tespit) ✓

**Scenario 3 — Hardware glitching:**
1. PMP register glitch → schedule() her tick verify_pmp_integrity (memory.rs:200-224) ✓
2. decide_action SEU → policy.rs:162-181 lockstep + black_box fence ✓
3. **AÇIK:** ct_eq_16 black_box derleyici barrier — RISC-V ISA garanti değil, future LLVM upgrade'da hoist edilebilir. Disassembly check informational (M3 paralel).
4. mscratch glitch → trap.S:17 beqz nested fault → "NF" UART → wfi park ✓

---

### Olumlu Bulgular — Senior Göstergeleri (Pass 1+2)

- **Defense-in-depth:** PMP shadow + L-bit + capability owner check + windowed watchdog + lockstep + blackbox + NF UART park
- **Compile-time invariants:** TRAP_FRAME assertion (config.rs:56-60), IPC message size (ipc/mod.rs:294), syscall count (dispatch.rs:465), multi-hart compile_error (sync.rs:6), feature exclusivity (secure_boot.rs:4-8)
- **Conservative defaults:** unassigned IPC channel default deny, KEY_READY=false → fail-closed cap, Dead/Isolated → task_stack_range None, decide_action unknown event → Isolate (policy.rs:131)
- **Helper extraction for FV:** is_selectable_by_scheduler / is_period_reset_eligible / should_watchdog_timeout (scheduler.rs:927-943) hem inline production'da hem Kani harness'inde kullanılıyor — single source of truth
- **Boot fail-closed:** secure_boot fail → halt; IPC seal fail → halt; task creation fail → halt; POST CRC/PMP/mtvec/BLAKE3/Ed25519 fail → halt
- **mscratch invariant:** boot.S → init → trap_entry → restore_regs → start_first_task → task_trampoline tüm path'lerde mscratch=kernel_sp (or 0 if nested) symmetric
- **Lockstep with input black_box:** policy.rs:162-171 hem input hem output black_box fence — CSE saldırısına ek güçlendirme
- **Test fail criteria explicit:** tests/mod.rs:877-889 toplam fail count > 0 → halt; "BOOT HALTED" CI tarafından grep ile yakalanıyor

---

## Pass 3 — Kalan Dosyalar + Kod Kalitesi Analizi

### Çalışma Kapsamı

İlk audit'te kritik path dosyalarını okumuştum (broker, scheduler, syscall, ipc, sandbox, trap, boot, config, memory, pmp, csr, policy, secure_boot, tests). Bu pass'te kalan **18 dosyayı** baştan sona okudum: `verify.rs`, `blackbox.rs`, `token.rs`, `cache.rs`, `fmt.rs`, `error.rs`, `types.rs`, `sync.rs`, `clint.rs`, `uart.rs`, `device.rs`, `iopmp.rs`, `key.rs`, `blake3_impl.rs`, `provider.rs`, `crypto/mod.rs`, `common/mod.rs`, `deny.toml`. Hepsi değişiklik yapılmadan, sadece okundu.

Ayrıca paralel agent'ların döndürdüğü bulguları doğrulamak için kritik claim'ler kod üzerinden tek tek kontrol edildi. Ajan bulgularının ~%60'ı false positive çıktı; doğru çıkanlar yeni bulgu olarak listelendi.

---

### YENİ Bulgular (Pass 1+2'de yakalanmamış, gerçek olanlar)

#### H3 — Kani BLAKE3 stub key material'i çıktıya kopyalıyor; "different key → different hash" türü prooflar trivial olarak geçiyor
- **[KAT]** FV / Doc
- **[DOS]** [src/common/crypto/blake3_impl.rs:55-67](src/common/crypto/blake3_impl.rs), [src/verify.rs:699-725](src/verify.rs)
- **[BUL]** Kani altında `Blake3Provider::keyed_hash` stub'ı `result[i] = key[i]` yapıyor (key'in ilk 16 byte'ını döndürüyor). `blake3_impl.rs:73-75` bu durumu açıkça notlandırmış ve aynı dosyadaki proof'lar `blake3_stub_*` olarak yeniden adlandırılmış. **Ancak** verify.rs:700-712'deki Proof 139 `blake3_different_key_different_hash` ve Proof 140 `blake3_same_input_same_hash` "BLAKE3" ismi taşıyor; gerçekte sadece stub'ın key passthrough davranışını test ediyor. Aynı şekilde Proof 147 (token_mac_field_matches_blake3_output) sadece dizi boyutlarını karşılaştırıyor.
- **[ETK]** Proof sayısı 200/201 olarak ilan ediliyor (kalite kriteri); bu üç proof BLAKE3 cryptographic property iddia eden isimlere sahip ama hiçbir kriptografik özellik kanıtlamıyor — pure passthrough'u test ediyorlar. ARCHITECTURE.md:80-82 zaten "crypto correctness via upstream" diye notlandırıyor ama proof isimleri okuyucuyu yanıltıyor.
- **[ÖNR]** Proof 139'u `stub_passes_through_key_difference` olarak rename + docstring ekleyin: "Stub identity property — NOT cryptographic."
- **[KNT]** blake3_impl.rs:62 `result[i] = key[i]`; verify.rs:711 `assert!(!same)` (key1 ≠ key2 olduğu için stub trivially geçer).

#### M8 — `dispatch_compute` bilinmeyen servis ID'sinde `-1` döner, `compute_mac` short-data hatası da `-1` — caller ayırt edemez
- **[KAT]** Quality / Correctness
- **[DOS]** [src/sandbox/mod.rs:288-296](src/sandbox/mod.rs), [src/sandbox/mod.rs:316-317](src/sandbox/mod.rs)
- **[BUL]** `dispatch_compute` 4 servis tanımlı, default arm `_ => -1` (line 294). `compute_mac` `if data.len() < 32 { return -1; }` (line 317). İki anlamlı farklı hata aynı kod ile dönüyor.
- **[ETK]** WASM modülü "service unknown" ve "MAC short data" arasında ayrım yapamaz, bug ayıklama zor; forensics bilgi kaybı.
- **[ÖNR]** Distinct kodlar: unknown service → -99, short data → -1, NotImplemented → -3 (mevcut). Veya `-> Result<i32, ComputeError>` döndür.
- **[KNT]** İki farklı kod path aynı dönüş değeri.

#### M9 — `device.rs` dead trait abstraction; `UartDevice::write_byte` non-blocking semantiği `uart.rs::putc` bounded-blocking semantiği ile çakışıyor
- **[KAT]** Quality
- **[DOS]** [src/hal/device.rs:18-105](src/hal/device.rs), [src/arch/uart.rs:11-33](src/arch/uart.rs)
- **[BUL]** `uart.rs::putc` 1000-iter THR-empty bekler, sonra char drop. `device.rs::DeviceAccess::write_byte` ilk poll'de hazır değilse `DeviceNotReady` döner. Production code yalnızca `uart::putc` kullanıyor; trait çağrılmıyor, tek implementor `UartDevice` ve hiçbir runtime path'te kullanılmıyor (`#[allow(dead_code)]` line 18). Trait + impl ~50 satır, blanket allow ile dead.
- **[ETK]** v2.0 birinin DeviceAccess'i kullanmaya başlaması durumunda davranış sessizce değişir (drop yerine error). Ölü kod binary footprint ve zihinsel yük.
- **[ÖNR]** Ya HAL trait'ini `#[cfg(feature = "v2-hal")]` arkasına gizleyin, ya da v1.0'da silin. Production binary'sinden çıkarmak v2.0 semantik tasarımına zarar vermez (trait yeniden eklenebilir).
- **[KNT]** device.rs'in tüm trait + impl path'i `#[allow(dead_code)]`; production paths uart.rs::puts/putc kullanıyor.

#### M10 — verify.rs WCET ordering `WCET_TOKEN_CACHE_HIT <= WCET_YIELD` (10 ≤ 10) — borderline tautolojik, FPGA sonrası kırılgan
- **[KAT]** FV
- **[DOS]** [src/verify.rs:76](src/verify.rs)
- **[BUL]** İki sabit de 10c. `<=` eşitlikle geçiyor; bu bir invariant değil, coincidence. WCET_YIELD = 10c FPGA ölçümü sonrası 5c'ye düşerse proof kırılır ama anlamsal regresyon yok.
- **[ETK]** FPGA ölçümünden sonra config recalibration sırasında bu proof patladığında kullanıcı düzgün invariant ihlali sanır.
- **[ÖNR]** Proof'u kaldır veya `assert!(WCET_TOKEN_CACHE_HIT < WCET_TOKEN_VALIDATE)` gibi gerçek invariant'a çevir (cache hit her zaman validate'tan ucuz olmalı — bu meaningful).
- **[KNT]** verify.rs:76 ve config.rs:152 (WCET_YIELD=10) + config.rs:191 (WCET_TOKEN_CACHE_HIT=10).

#### M11 — `deny.toml` "Copyleft yasaklı" yorumu ile gerçek policy uyuşmuyor (deny block boş, sadece allow list var)
- **[KAT]** Doc / Supply chain
- **[DOS]** [deny.toml:13-27](deny.toml)
- **[BUL]** Yorum (line 13): "Copyleft (GPL, MPL-2.0) dep'leri yasaklı". Gerçek `[licenses]` bloğunda sadece `allow = [...]` var, `deny` yok. cargo-deny semantiği: allow listesinde olmayan license'lar configurable behavior'a göre yasaklanır (varsayılan: warn ya da error). Yeni cargo-deny sürümlerinde varsayılan davranış değiştiyse beklenmeyen sonuç çıkabilir.
- **[ETK]** "yasaklı" iddiası açık compile-time enforcement'a değil, allow-list mekanizmasına dayanıyor. Bir transitive dep şu anda allow listesinde olmayan bir license eklerse cargo-deny davranışı sürüm bağımlı.
- **[ÖNR]** Explicit `deny = ["GPL-2.0", "GPL-3.0", "AGPL-3.0", "MPL-2.0"]` ekle veya `unlicensed = "deny"` + `copyleft = "deny"` flag'lerini açıkça yaz.

#### L6 — verify.rs Kani harness'lerde `.unwrap()` kullanımı (`assert!(option.is_some())` + `unwrap()` antipattern)
- **[KAT]** FV / Quality
- **[DOS]** [src/verify.rs:19-21](src/verify.rs), [src/verify.rs:194-200](src/verify.rs), [src/verify.rs:212-216](src/verify.rs)
- **[BUL]** Pattern: `assert!(x.is_some()); let r = x.unwrap();` — Kani altında unwrap None'da panic atar = proof fail. assert + unwrap ikinci kez aynı kontrolü yapıyor.
- **[ETK]** Cosmetic; proof correctness'i etkilemez ama gereksiz iki kontrol.
- **[ÖNR]** `if let Some(r) = x { assert!(r == expected); } else { panic!() }` veya `kani::assume(x.is_some())` + `let r = x.unwrap()`.

#### L7 — verify.rs `for level in &levels` iterator kullanımı, projenin geri kalanı `while i < N` doctrine'ına aykırı
- **[KAT]** Quality / Determinism
- **[DOS]** [src/verify.rs:31](src/verify.rs), [src/verify.rs:95](src/verify.rs), [src/verify.rs:98-102](src/verify.rs)
- **[BUL]** Üç yerde `for x in &arr` veya `for i in 0..n` kullanılıyor. Diğer tüm dosyalar `while i < n { ... ; i += 1; }` pattern'ini takip ediyor (deterministik codegen + clippy hood gerek yok). Style drift.
- **[ETK]** Sıfır runtime etki (Kani harness, production değil), ama style guide consistency.
- **[ÖNR]** Mini cleanup PR — `while i < N` ile değiştir.

#### L8 — `BB_NEXT_SEQ` u32 wrap sınırı 23 yıl ama wrap davranışı belgesiz
- **[KAT]** Quality / Doc
- **[DOS]** [src/ipc/blackbox.rs:142-143](src/ipc/blackbox.rs), [src/ipc/blackbox.rs:260](src/ipc/blackbox.rs)
- **[BUL]** `BB_NEXT_SEQ` `seq.wrapping_add(1)` (line 260). Comment line 142: "u32, ~23 yıl wrap-free @ 6 kayıt/saniye". Ancak yüksek event hızında (örn. WCET ihlali altında saniyede yüzlerce event) wrap çok daha erken gelir. Wrap sonrası post-mortem analyzer aynı seq görür → eski/yeni karışır.
- **[ETK]** Pratikte düşük (ortalama event hızı düşük) ama edge case (sürekli isolate/restart loop) wrap'i hızlandırır.
- **[ÖNR]** seq u64 yap (8KB tampon içinde fazladan 4 byte/record × 128 = 512B; toplam 8.5KB → BLACKBOX_SIZE'ı 8KB'dan 9KB'a güncelle gerekli) veya wrap kontrolü ekle: BB_BOOT_EPOCH gibi BB_SEQ_EPOCH artır.

#### L9 — `fmt.rs` print_u32/u64/hex magic literal `i < 10/20/16` vs buffer boyutu — `buf.len()` daha sağlam
- **[KAT]** Quality
- **[DOS]** [src/common/fmt.rs:19](src/common/fmt.rs), [src/common/fmt.rs:33](src/common/fmt.rs), [src/common/fmt.rs:46](src/common/fmt.rs)
- **[BUL]** `let mut buf = [0u8; 10]; ... while val > 0 && i < 10` — magic literal 10 buffer boyutuyla manual sync. Aynı print_u64 (20), print_hex (16).
- **[ETK]** Buffer boyutu değişirse bound senkronizasyonu unutulabilir. Şu an iki yerde de doğru.
- **[ÖNR]** `while val > 0 && i < buf.len()` — derleyici aynı kodu üretir, daha sağlam.

#### L10 — `uart.rs::putc` 1000-iter bound FPGA hardware'a uygun değil olabilir
- **[KAT]** Correctness / Perf
- **[DOS]** [src/arch/uart.rs:13-30](src/arch/uart.rs)
- **[BUL]** Comment: "1000 iter × 3c = 3000c = 30µs". Gerçek: NS16550A FIFO 16-byte derinlikte; 115200 baud'da byte başına ~87µs. CVA6 100MHz'de 1000 iter ≈ 10µs — UART tek byte göndermeyi bile bitirmeden polling biter, char drop edilir.
- **[ETK]** QEMU'da UART "instant ready" → loop 1 iterde çıkar, sorun yok. FPGA bring-up'ta debug-boot output'u sürekli kaybolacak (sessiz başarısızlık). Production'da debug-boot kapalı olduğu için kritik değil ama developer experience için sürpriz.
- **[ÖNR]** Bound config sabiti yap: `UART_POLL_LIMIT = 100_000` (CVA6 100MHz'de 1ms). Veya FPGA bring-up sonrası ölçümle ayarlanması için TODO.
- **[KNT]** uart.rs:18 yorum 30µs = 5.5µs/byte iddia ediyor; 115200 baud'da gerçek byte time 87µs.

#### L11 — STRUCTURE.md / ARCHITECTURE.md "TLC v2.19, 35,770 distinct states" iddiası repo'da kanıtlı değil
- **[KAT]** Doc
- **[DOS]** Tla+/, ARCHITECTURE.md:59
- **[BUL]** 7 .tla dosyası mevcut (SipahiPolicy, Scheduler, Capability, IPC, Watchdog, DegradeRecover, BudgetFairness). TLC çalıştırma script'i veya .out/.log dosyası repo'da yok. CI'da TLC adımı yok. "35,770 distinct states" sayısı manuel iddia.
- **[ETK]** Dış denetimde reproducible olmayan claim.
- **[ÖNR]** `Tla+/run_tlc.sh` + her spec için TLC çıktısı `Tla+/results/*.out`. CI'a opsiyonel TLC step. Veya iddiayı yumuşat: "specs designed for TLC verification (v1.5'te otomatik run eklenecek)".

#### L12 — `Token::_pad: [u8; 2]` field repr(C) padding'i explicit yapıyor, MAC computation pad'i sıfır kabul ediyor
- **[KAT]** Correctness / Quality
- **[DOS]** [src/kernel/capability/token.rs:36-37](src/kernel/capability/token.rs), [src/kernel/capability/token.rs:66-67](src/kernel/capability/token.rs)
- **[BUL]** `_pad` field'ı struct'ta var, `header_bytes()` içinde h[6]=0, h[7]=0 hardcoded. Bu güvenli ama runtime'da kim _pad'e bir şey yazarsa MAC hâlâ 0 ile hesaplanır → MAC mismatch. `Token::zeroed()` _pad'i 0 başlatıyor, başka path yok.
- **[ETK]** Şu an exploitable değil. Ancak: header_bytes() bytemuck/transmute ile değiştirilirse padding undefined behavior, MAC nondeterministic.
- **[ÖNR]** Compile-time guarantee: `const _: () = assert!(Token::zeroed().header_bytes()[6] == 0 && Token::zeroed().header_bytes()[7] == 0);` veya doc'ta "header_bytes() always overrides _pad bytes — do not transmute Token to bytes for MAC".

---

### Yanlış Pozitif Olarak Reddedilen Ajan Bulguları

İlk pass'te ajanların döndürdüğü 3 "CRITICAL" bulgusu doğrulamada false-positive çıktı:

1. **"cache.rs:59 expiry inversion CRITICAL"** — Branch-free OR gate (`is_infinite | not_expired`) doğru çalışıyor. expires=0 durumunda is_infinite=1 OR'a hakim, sonuç 1. Doğrulanmadı, finding değil.
2. **"UART putc CRITICAL latency injection"** — putc tüm `puts`/`println`'lerden çağrılıyor ama bunların çoğu `#[cfg(feature = "debug-boot")]` veya `#[cfg(feature = "trace")]` gateli; production binary'de aktif değil. Tasarım uygun.
3. **"clint.rs schedule_next_tick HIGH no compiler_fence"** — `core::ptr::read_volatile` zaten compiler-level fence (Rust spec). RISC-V CPU-level reordering MMIO için garanti edilmiyor ama bu ARM-style memory ordering meselesi, RISC-V `mtime`/`mtimecmp` aynı CLINT cycle domain'inde, sıralama otomatik. Finding değil.

Ayrıca M2/M3 (cache LRU vs FIFO) ve "TaskContext yeni alan eklenirse" gibi bulgular tehdit modeline göre geçerli senaryolar değil veya zaten ARCHITECTURE.md'de belgelenmiş.

---

### Genel Kod Kalitesi Analizi

Senior-bar değerlendirmesi: **kod tabanı genel olarak senior bar üstünde**, aşağıdaki konularda pekiştirme yapılabilir.

#### ✓ Güçlü Yönler (Senior+ standardı)

1. **Tek doğruluk kaynağı (Single source of truth):**
   - `is_selectable_by_scheduler`/`is_period_reset_eligible`/`should_watchdog_timeout` helper'ları hem production scheduler'da hem Kani harness'inde kullanılıyor (scheduler/mod.rs:927-943). Drift imkansız.
   - `pack_pmpcfg` const fn — boot, Kani, ve runtime aynı fonksiyonu çağırıyor.
   - `decide_action` pure const fn, lockstep'te de Kani'de de aynı.

2. **Compile-time invariants:**
   - `const _: () = assert!(TRAP_FRAME_SIZE - TRAP_FRAME_USER_SP_OFFSET == 16, ...)` (config.rs:56-60)
   - `const _: () = assert!(core::mem::size_of::<IpcMessage>() == 64);` (ipc/mod.rs:294)
   - `const _: () = assert!(SIGNATURE_SIZE == 2 * OTP_KEY_SIZE);` (key.rs:31)
   - `const _: () = assert!(BLACKBOX_MAX_RECORDS <= 255);` (blackbox.rs:315)
   - `const _: () = assert!(core::mem::size_of::<Token>() == 32);` (token.rs:81)
   - `compile_error!` for mutually exclusive features (secure_boot.rs:4-8) ve multi-hart guard (sync.rs:6).

3. **Volatile macro disiplini (LTO-safe):**
   - `vol_read!`/`vol_write!` macros (ipc/blackbox.rs:160-169) static mut'ı LTO + `opt-level="s"` altında register caching'den koruyor. Bu pattern projede tutarlı uygulanmış.

4. **Defense-in-depth:**
   - PMP shadow + L-bit + per-task NAPOT + watchdog + lockstep + nested fault park.
   - Pointer validation: `is_valid_user_ptr` task_stack_range tabanlı (default deny Dead/Isolated için).
   - Dispatch'te kernel pointer scrubbing (dispatch.rs:226-229): syscall sonucu RAM_BASE arası ise E_INTERNAL döner.

5. **Token kriptografi disiplini:**
   - `header_bytes()` explicit byte construction — endian-agnostik (token.rs:58-77). Memory aliasing/transmute YOK.
   - `ct_eq_16` const-time XOR + black_box (broker.rs:197-207).

6. **`SingleHartCell` pattern:**
   - `static mut` sıfır (`#[allow(dead_code)]` üzerinden compile_error'lar var).
   - Multi-hart compile_error guard (sync.rs:6). Bilinçli single-hart concurrency.

7. **No panic/no alloc/no float discipline:**
   - `panic = "abort"`, `overflow-checks = true`, `lto = true`, `codegen-units = 1` (Cargo.toml). Production'da unwrap/expect 0 (sadece test/Kani'de).

8. **Trap-frame ABI symmetry:**
   - trap.S entry/exit + context.S task_trampoline mscratch invariant'ını koruyor. Yorumlar (especially context.S:78-98) WHY açıklıyor, mekanik akış değil.

#### ⚠ Senior-Bar Altında / Pekiştirme Alanları

1. **Stale comments / Sprint references:**
   - crypto/mod.rs comments: "Sprint 9'da implemente edilecek" diyor ama Sprint 13'te uygulandı.
   - main.rs:81 banner "v1.5" yazıyor; Cargo.toml `version = "0.1.0"`; audit "v1.0".
   - sandbox/mod.rs:281 yorum: "WCET: COPY ~80c · CRC ~120c" ama config.rs::WCET_COMPUTE_CRC = 1500 (Sprint U-15 düzeltmesi sonrası).
   - **Etki:** ~10 farklı stale yorum bulundu. Senior bar: yorumlar kodun gerisinde kalmamalı.

2. **`#[allow(dead_code)]` enflasyonu:**
   - 60+ tekil işaretleme + 3 modül-seviyesi (`config.rs`, `csr.rs`, `sandbox/mod.rs`, `iopmp.rs` — tümü gerekçeli).
   - Çoğu "v2.0 API" veya "Kani harness only" gerekçesi taşıyor; ancak `device.rs` tüm trait'i dead — gerçekten v2.0 fonksiyonu yok, hiç kullanılmıyor (M9).
   - **Senior bar:** dead_code gerçekten "future API" mi yoksa "shipped but unused" mı ayrımı gerekli. Yıkıcı temizlik: gerçekten v2.0'da gerekecek olanları sakla, geri kalanı sil. Şu an karışım.

3. **DRY ihlali — duplicate LEB128:**
   - `read_u32_leb128` (sandbox/mod.rs:64) ve `read_leb128_u32` (sandbox/mod.rs:146) işlevsel olarak aynı. Tek farkları: ilki `bytes` slice alıyor, ikincisi `(code, pos)` çifti alıyor.
   - **Etki:** WASM bytecode parsing tek path'te bozulursa diğeri tutarsız davranabilir.
   - **Fix:** Tek implementasyon + adapter wrapper.

4. **Magic number'lar config dışında — token.rs:**
   - `0x01, 0x02, 0x04, 0x07` action bitleri token.rs:20-26'da named constants, iyi. Ama:
   - `header_bytes()` içinde `h[6] = 0; h[7] = 0;` hardcoded sıfırlar — iyi yorumlu (line 66-67).

5. **`device.rs` dead trait abstraction:**
   - 50+ satır kullanılmayan trait + impl. Üretim binary'sine link aşamasında dead-code-elimination yapsa da kaynak okurluğunu bozuyor.
   - **Senior fix:** v2.0 için tasarım yapacaksak `feature = "v2-hal"` gate'i; yapmayacaksak şimdi sil.

6. **`schedule()` cyclomatic complexity:**
   - schedule() 174 satır, 4 phase + 2 early-return path. WCET için inline kalmalı. Phase'ler private helper fn'lere ayrılabilir, `#[inline(always)]` ile WCET aynı kalır, okurluk artar.
   - **Senior bar:** "Hot path tek fonksiyonda" derken bile bir senior phase'leri named blok comment + closure veya helper'la ayırır. Şu an "Faz 1, 1.5, 2, 3, 4" yorumları akış kontrolünü taşıyor.

7. **`#[inline]` kullanım tutarsızlığı:**
   - csr.rs tüm CSR helper'ları `#[inline(always)]` ✓
   - clint.rs `read_mtime` no inline ✗ (1 instruction read_volatile, kesinlikle inline edilmeli)
   - blackbox.rs `count()`, `current_tick()`, `get_tick()` no inline (tek volatile read'ler)
   - **Etki:** `#[inline(always)]` olmadan LLVM çoğu zaman doğru karar verir ama `lto=true` ile kombinasyonda her yerde aynı kararı vermez. Trap-handler hot path'inde `read_mtime` indirect call olursa scheduler tick WCET artar.
   - **Fix:** Tüm `#[inline(always)]`-uygun MMIO/CSR helper'larını işaretle.

8. **`fmt.rs` print_u32 magic bound (L9 yukarıda):**
   - `i < 10` → `i < buf.len()`. Tek satır değişiklik, gelecek kırılganlığı azaltır.

9. **String table'lar:**
   - `error.rs::SipahiError::as_str()` (14 variant, 14 string) ve `tests/mod.rs` test mesajları (~40 string) hepsi `&'static str`. Tasarım iyi. Ancak test mesajlarındaki Unicode özel karakterler (`✓`, `✗`, `★`) UTF-8 byte'ı UART'a yazıyor — terminal görüntülerse OK ama POST log post-mortem analizde garbage olabilir.
   - **Minor:** ASCII-only `[OK]`/`[FAIL]`/`[***]` daha taşınabilir.

10. **WCET comment vs sabit drift:**
    - sandbox/mod.rs:281: "WCET: COPY ~80c · CRC ~120c · MAC ~350c · MATH ~200c" — CRC değeri config.rs'te 1500 olarak güncellenmiş ama yorum güncellenmemiş.
    - Senior bar: docstring + config sabiti tek doğruluk kaynağı, yorum ile hardcode sayı YASAK.

11. **`task_trampoline` register clear sırasının gizli kuralı:**
    - context.S:107-127 yorumda "t0/t1 yukarıda kullanıldı — temizleme EN SON" yazıyor. Bu doğru ama assembler'da yapılmış manual ordering invariant. Yanlış sıralama silent register leak'a sebep olur.
    - **Senior fix:** Bir daha sonra okuyacak insan için "// MUST clear t0,t1 LAST" yorumu var ama makinece zorlanmıyor. Compile-time invariant zor (asm), en azından Kani'de "after task_trampoline, no caller-saved register holds non-zero kernel value" iddiası çok zor; sadece runtime negative test mümkün — şu an yok.

12. **`broker::sign_token` sadece self-test'de kullanılıyor:**
    - Production'da HSM token üretir, runtime sign_token çağrılmaz. Şu an `#[allow(dead_code)]` ile pass ediyor ve sadece tests/mod.rs'te (test_capability_broker) kullanılıyor. v1.0 production'da capability sistemi tamamen by-pass — bu daha önce H2'de işlendi.

#### Performans Gözlemleri (Gereksiz Cycle)

Hot path'lerde ölçülebilir gereksiz iş:

1. **`broker::validate_full` ordering — MAC compute her zaman çalışıyor:**
   - cache miss path (~400c BLAKE3). Cache miss her zaman tam validation yapar; hızlandırma için "negative cache" (recently rejected token) yok. v1.0 için yeterli; v1.5 cache hit ratio ölçümü sonrası optimize edilebilir.

2. **`schedule()` Phase 1 her tick tüm task'lar üzerinde dönüyor:**
   - 8 task × 1 cycle/task = 8c overhead. Sleeping task yoksa optimize edilebilir ama 8c önemsiz.

3. **`schedule()` Phase 1.5 watchdog reset Ready+Running için ipc_send_count'ı sıfırlıyor:**
   - `if st == TaskState::Running || st == TaskState::Ready` (line 287-289). U-16 fix doğru. Ek if/branch ~1c/task; toplam ~3c (Running+Ready=2-3 task). Önemsiz.

4. **`apply_policy` lockstep 2 kez `decide_action_fenced` çağırıyor:**
   - 2 × match table = ~10c overhead. Defense-in-depth için kabul edilebilir.

5. **`ct_eq_16` her zaman 16 iter — hit miss aynı cycle:**
   - Constant-time bilinçli; perf değil.

6. **`crc32` bit-by-bit, byte başına 8 iter:**
   - 60 byte × 8 iter × 3c = ~1440c. Lookup table 256-entry × 4byte = 1KB ile 8x hızlanma mümkün ama .rodata kullanımı ve cache miss riski. v1.0 trade-off doğru.

7. **`fmt.rs::print_u64` 20 iter bound:**
   - print_u64(0) special case (line 30) erken çıkar, başarılı. print_u64(u64::MAX) tam 20 iter. Önemsiz çünkü debug-boot/trace altında.

8. **`blackbox::log` 18 byte zeroed alloc + manual byte copy:**
   - `BlackboxRecord::zeroed()` her log'da yeni 64-byte stack alloc + memset. Sonra manual `i < n` byte copy. Optimize: `core::ptr::copy_nonoverlapping(data.as_ptr(), rec.data.as_mut_ptr(), n)` sadece data alanını kopyalar, geri kalan zaten zero. Şu an `data.iter().take(n)` semantik olarak doğru ama ~5-10c savings mümkün.
   - Hotness: blackbox.log saniyede on'larca event için ~100c × N. Toplam <1000c/saniye. Optimize değer/maliyet borderline.

9. **`is_valid_user_ptr` `task_stack_range` çağrısı:**
   - Her syscall'da 1 task lookup + bounds check ~20c. WCET budget içinde.

10. **`context.S::switch_context` 14+14 callee-saved + 2 CSR + 2 LA:**
    - Necessary; her save/load ~1c. Toplam ~50c, config.rs::WCET_CONTEXT_SWITCH=80 içinde.

**Sonuç:** Hot path'lerde ölçülebilir gereksiz iş yok. Tek mikro-optimizasyon adayı blackbox::log byte copy (M değer/maliyet). Diğer her şey ya zaten optimize ya da defense-in-depth için kasıtlı.

#### Yazım Kalitesi (Naming, Komentler)

- **İyi:** Sprint U-X yorumları her büyük değişiklikte hangi sprint'te neden yapıldığını belgeliyor. WHY açıklamaları yeterli.
- **İyi:** `// SAFETY:` yorumları her unsafe block'ta var (CI informational check). Yorumlar genelde "single-hart, MIE=0" demek; bu pattern'i bir senior `TrapToken<'a>` veya `MIDisabled` lifetime guard ile compile-time enforce ederdi (tekrar L4'te işlendi).
- **İyi:** Türkçe/İngilizce karışımı tutarlı (kod İngilizce, yorum çoğu Türkçe — proje kuralı görünüyor).
- **Eksik:** Module-level docstring (`//!`) bazı modüllerde 1 satır, bazılarında yok. blackbox.rs ve broker.rs kapsamlı; capability/mod.rs minimal.
- **Eksik:** Compile-time guarantee'lerden bazıları yorum olarak iddia edilmiş ama assertion yok (Token _pad zero-fill, mscratch=kernel_sp invariant, WCET budget'leri). Bunların bir kısmı assembly olduğu için compile-time guard zor.

---

## Birleşik Bulgu Tablosu

| ID | SEV | Kategori | Konu | Pass |
|---|---|---|---|---|
| H1 | HIGH | Boot | POST CLINT/misa WARN-only | 1 |
| H2 | HIGH | Security | Default features secure boot kapalı | 1 |
| H3 | HIGH | FV | Kani BLAKE3 stub key passthrough | 3 (yeni) |
| M1 | MED | CSR | mcounteren unset | 1 |
| M2 | MED | CSR | medeleg/mideleg unset | 1 |
| M3 | MED | CSR | write_mtvec mode bits | 1 |
| M4 | MED | Build | Linker ↔ config no link | 1 |
| M5 | MED | FV | TaskContext size assert eksik | 1 |
| M6 | MED | IPC | CRC opt-in | 1 |
| M7 | MED | Capability | nonce update timing belgesiz | 1 |
| M8 | MED | Quality | dispatch_compute -1 ambiguity | 3 (yeni) |
| M9 | MED | Quality | device.rs dead trait | 3 (yeni) |
| M10 | MED | FV | WCET ordering tautoloji | 3 (yeni) |
| M11 | MED | Doc | deny.toml license drift | 3 (yeni) |
| L1 | LOW | Doc | Kani 200 vs 201 drift | 1 |
| L2 | LOW | Build | Caret deps, exact pin yok | 1 |
| L3 | LOW | Doc | Banner v1.5 vs Cargo 0.1.0 vs audit v1.0 | 1 |
| L4 | LOW | Quality | "MIE=0" SAFETY yorum compile-time enforce edilmiyor | 1 |
| L5 | LOW | Quality | WATCHDOG_WINDOW_MIN=3 hardcoded | 1 |
| L6 | LOW | FV | unwrap antipattern verify.rs | 3 (yeni) |
| L7 | LOW | Style | iterator vs while inconsistency | 3 (yeni) |
| L8 | LOW | Quality | BB_NEXT_SEQ wrap dokümantasyon | 3 (yeni) |
| L9 | LOW | Quality | fmt.rs magic literal vs buf.len() | 3 (yeni) |
| L10 | LOW | HW | uart.rs 1000-iter FPGA bound | 3 (yeni) |
| L11 | LOW | Doc | TLA+ TLC results not in repo | 3 (yeni) |
| L12 | LOW | Quality | Token _pad zero invariant | 3 (yeni) |
| I1 | INFO | Doc | TLC log eksik, "35,770 distinct states" iddiası | 1 |
| I2 | INFO | CI | --locked + unsafe doc continue-on-error | 1 |
| I3 | INFO | Quality | schedule() 174 satır complexity | 1 |
| I4 | INFO | Sandbox | 0xFC LEB128 sub-opcode > 127 edge case | 1 |

**Pass 3 yeni:** 4 MEDIUM + 7 LOW = **11 yeni bulgu** (ilk pass'te yakalanmamış).

---

## Önerilen v1.5 / v2.0 Sertleştirme Listesi

Öncelik sırasına göre:

### v1.0 → v1.0.1 (production-blocker patch)
1. **H1**: POST CLINT + misa kontrolleri WARN→HALT.
2. **H2**: Default features production-ready: test-keys olmadığında compile_error veya production-otp feature ile değiştir.

### v1.5 (sertleştirme paketi, ~300 satır)
3. **M1**: mcounteren=0 (timing side-channel kapatma).
4. **M2**: medeleg/mideleg=0 explicit + POST verify.
5. **M3**: write_mtvec mode bits explicit mask.
6. **M4**: build.rs ile linker ↔ config compile-time bağlama.
7. **M5**: TaskContext size + offset_of const_assert.
8. **M6**: IPC CRC kernel-side enforce (send compute + recv verify).
9. **M7**: validate_full nonce update semantik dokümantasyon + expired-but-valid path düzeltmesi.
10. **H3 + M10**: Kani proof renaming (stub_*) + WCET ordering tautoloji temizliği.

### v2.0 / Hardening
11. **Smepmp / .secure_data carve-out**: ARCHITECTURE.md:146 zaten v1.5 roadmap; MAC_KEY/PMP_SHADOW/LAST_NONCE U-mode-deny.
12. **Type-level trap-context guard**: SingleHartCell::get() yerine `get(t: &TrapToken)` ile compile-time MIE=0 invariant.
13. **CI extensions**: TLC step ekle, --locked enforce, double-build determinism check.
14. **M8, M9, M11**: dispatch_compute distinct error codes; device.rs trait feature-gate veya silme; deny.toml explicit deny block.
15. **L6-L12 cleanup**: tek mini PR ile (~50 satır).

### Past-bug regression test gap'leri (öncelik düşük ama gerekli)
- U-19 task_trampoline register clear için negative test ekleyin.
- U-16 schedule() tek-task path için runtime test ekleyin.
- U-16 Ready watchdog için INFO yerine assertion'lı test.

---

## Performance Baseline (AMCI Öncesi Referans)

QEMU TCG cycle counter modu instruction count → gerçek WCET değil. AMCI öncesi göreceli karşılaştırma için:

| Path | Estimated WCET (config.rs) | Notes |
|---|---|---|
| trap_entry | 80c | mscratch swap + 16 reg save + CSR read |
| trap_handler dispatch | 80c | timer or ecall path |
| schedule() full | 350c | PMP verify + 4 phases + context switch |
| context switch | 80c | 14 callee-saved + 2 CSR save/restore |
| cap_invoke (cache hit) | 25c | 4-slot scan + ct_eq_16 |
| ipc_send | 60c | bounds + ptr validate + ring write |
| ipc_recv | 40c | bounds + ring read + ptr write |
| token validate_full | 400c | BLAKE3 keyed_hash + ct_eq + cache insert |

**Multi-hart sonrası regression detection:** her path için actual rdcycle ölçümü WCET_LAST'a düşüyor (dispatch.rs:128-138). check_wcet_limits (dispatch.rs:165-196) self-test build'de runtime check yapıyor; CI'da informational ("WCET regression check" tests/mod.rs:288-296). AMCI sonrası bu sayıların aynı sırada kalması beklenir.

---

## Build Determinism

CI'da double-build + sha256 karşılaştırma yok (ci.yml). `make build` sonrası tek pas hash:
- Cargo.toml: `lto = true`, `codegen-units = 1`, `opt-level = "s"` → deterministic codegen flag'leri set.
- rust-toolchain.toml: nightly-2026-03-01 pinli.
- Cargo.lock: tüm deps exact pin.

Beklenen sonuç: reproducible. Doğrulanmadı (read-only audit). v1.5'te CI'a `make build && sha256sum > b1; cargo clean; make build && sha256sum > b2; diff` adımı eklenmesi önerilir.

---

## Metrik Özet

| Metric | Value |
|---|---|
| LOC | 9,132 Rust + 321 ASM |
| Source files | 39 .rs + 3 .S |
| Kani proofs | **201** (README/docs claim 200 — L1) |
| TLA+ specs | 7 (TLC verification logs not in repo — I1, L11) |
| Compile-time asserts | 8+ const_assert (TRAP_FRAME, IPC_MSG_SIZE, SYSCALL_COUNT, multi-hart guard, feature exclusivity, IPC slots, vb.) |
| Production binary | claimed ~33 KB (not re-measured this run) |
| unsafe blocks | 123 (`grep "unsafe {" src/`); SAFETY-comment coverage advisory in CI |
| Negative tests | 6 + 1 INFO (cross-task ptr, owner, IPC, PMP, blackbox, allocator, watchdog) |
| Dependency count | 3 direct (wasmi, blake3, ed25519-dalek), all caret (L2) |
| Reproducible build | Likely (toolchain + flags pinned), unverified by CI |
| CI gates | 5 jobs: build+clippy, qemu-test, audit, kani-full (master), kani-fast (PR) |
| Watchdog regression guards | scheduler tek-task path runtime test yok (matrix ⚠) |
| Production NF count | 0 (U-18 fix) |
| Sprint count | 14 (Sprint 14 documents say) — drift with main.rs banner v1.5 (L3) |
| Toplam bulgu | 0 CRITICAL · 3 HIGH · 11 MEDIUM · 12 LOW · 4 INFO |

---

## Sonuç — Senior Bar Değerlendirmesi

**Sipahi v1.0 senior+ bar üzerinde** mimari ve disiplin açısından. Compile-time invariants, single source of truth (helper-extracted Kani harness'ler), volatile macro disiplini, fail-closed defaults, defense-in-depth — hepsi yerinde. CRITICAL sınıfı bug bulunmadı.

**Pekiştirme alanları (öncelik sırasına göre):**

1. **HIGH** H1+H2+H3: production deployment ve FV credibility için zorunlu.
2. **MEDIUM** M1-M11: 100-300 satırlık bir sertleştirme PR'ı ile çoğu kapatılabilir, AMCI öncesi yapılırsa multi-hart geçişinde sürpriz minimize edilir.
3. **Stale yorumlar + dead_code temizliği** (M9, INFO): Senior bar için "yorum kodun gerisinde kalmamalı" disiplini şu an ~%80; %95'e çıkarmak küçük bir refactor PR ile mümkün.
4. **`#[inline(always)]` tutarlılığı**: MMIO/CSR helper'lar için tek geçişlik PR (~10 dosya).
5. **Performans**: hot path'lerde ölçülebilir gereksiz iş yok. v1.5'te FPGA WCET ölçümünden sonra `WCET_*` sabitleri yeniden kalibre edilmeli; o zaman M10 (cache_hit ≤ yield tautoloji) ve diğer borderline assertion'lar revize edilir.

**Genel kanaat:** AMCI multi-hart geçişine engel olabilecek 3 HIGH bulgu var (H1, H2, H3). Geri kalanı sertleştirme + cleanup. Kod tabanı, "safety-critical RTOS for RISC-V" iddiasını destekleyecek şekilde inşa edilmiş; senior bar atlanmamış, sadece birkaç köşede pekiştirme bekliyor.
