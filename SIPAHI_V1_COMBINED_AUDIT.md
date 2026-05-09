# Sipahi v1.0 — Birleşik Audit Raporu (Codex + Claude Code)

> **Hedef:** AMCI multi-hart geçişi öncesi son kapsamlı denetim.
> **Yöntem:** İki bağımsız auditor'ün (Codex CLI + Claude Code) Sipahi
> v1.0 üzerinde yaptığı read-only audit'in birleşimi. Hiçbir kaynak
> dosya değiştirilmedi. Çakışan bulgular tek başlık altında ikisinin
> de gözlemleri korunarak konsolide edildi.
>
> **Tarih:** 2026-05-01
> **Audit kapsamı:** 42 src dosyası, sipahi.ld, Cargo.toml, deny.toml,
> .github/workflows/ci.yml, 7 TLA+ spec, README/ARCHITECTURE/STRUCTURE,
> docs/sipahi_features_{tr,en}.md.

---

## Birleşik Özet

| Severity | Codex | Claude Code | Birleşik (deduplicated) |
|---|---|---|---|
| CRITICAL | 0 | 0 | **0** |
| HIGH | 4 | 3 | **6** (1 örtüşme: secure boot) |
| MEDIUM | 6 | 11 | **15** (2 örtüşme: deps, sandbox stale) |
| LOW | 4 | 12 | **16** (1 örtüşme: stale comments) |
| INFO | 0 | 4 | **4** |
| **Toplam** | **14** | **30** | **41** unique bulgu |

**CRITICAL bulgu yok.** Vanilla PMP'nin MAC_KEY/PMP_SHADOW'u U-mode'a
okutması ARCHITECTURE.md §Known Limitations'da explicit kabul edilmiş
— A.0 threat model gereği yeniden bulgu yapılmadı.

**HIGH bulgu kümesi production deployment'ı doğrudan etkiliyor:**
3'ü her iki auditor'de örtüşmemiş tamamen yeni bulgular (UART PMP açık,
SYS_YIELD scheduler tick, unknown exception livelock); 1'i U-19 hardening'in
eksik kaldığı yer (start_first_task register leak); 2'si Claude'a özgü
(POST WARN-only, default features secure boot kapalı); 1'i FV credibility
(BLAKE3 stub passthrough proof'lar).

---

## Threat Model Referansı (her iki audit ortak)

**KAPSAMDA:**
- Hostile WASM modülü (sandbox escape — CRITICAL kapsamında)
- Kötü niyetli U-mode task (privilege escalation — CRITICAL kapsamında)
- Hardware fault injection / glitching (PMP shadow + lockstep defense)
- Compromised dependency (cargo audit + exact pin)
- Side-channel timing attack (ct_eq_16, capability MAC)

**KAPSAM DIŞI (tasarım sınırı, finding değil):**
- Fiziksel JTAG erişimi
- DRAM rowhammer (hardware mitigation gerekli)
- Spectre/Meltdown (CVA6 hedef, M-mode tek context)
- Kernel itself compromised (TCB içinde — root of trust)

**Known Limitation kapsamında (yeniden raporlanmadı):**
- Vanilla PMP MAC_KEY/PMP_SHADOW U-mode'dan okunabilir (ARCHITECTURE.md:132-148)
- WCET değerleri estimated, FPGA ölçümü pending
- Kani assembly/CSR/crypto kapsam dışı
- TLA+ refinement mapping yok
- Single-hart only (compile_error guard mevcut)

---

## CRITICAL Bulgular

**Yok.**

---

## HIGH Bulgular

### H1 — POST CLINT timer ve misa kontrolleri WARN-only, HALT olmuyor
> Kaynak: Claude Code

- **[KAT]** Correctness / Boot
- **[DOS]** [src/tests/mod.rs:566-577](src/tests/mod.rs), [src/tests/mod.rs:583-604](src/tests/mod.rs)
- **[BUL]** POST'taki diğer sağlık kontrolleri (CRC32, PMP, mtvec, BLAKE3, Ed25519, mstatus) `halt_system()` ile boot'u durdurur. Ancak CLINT mtime ilerlemiyorsa sadece "WARN: mtime not advancing" yazılır + blackbox kaydı atılır ve boot devam eder. misa non-conforming ISA için aynı asimetri.
- **[ETK]** Saldırgan/donanım hatası mtime register'ını dondurursa: timer interrupt tetiklenmez → schedule() çağrılmaz → PMP shadow integrity check de bir daha çalışmaz. Tüm safety mekanizması (watchdog, budget, lockstep PMP verify) sessizce ölü hale gelir.
- **[ÖNR]** İki kontrolü de `halt_system("[POST] FAIL: CLINT timer dead — HALT")` ile sertleştirin. Grace period gerekirse 3 ardışık örnekleme yapın ama nihai sonuç WARN değil HALT olmalı.
- **[KNT]** tests/mod.rs:566-577 ve 583-604; CRC32 testinde (lines 477-479) ve PMP testinde (lines 483-486) `halt_system` kullanılırken CLINT'te kullanılmıyor.

### H2 — Default feature build secure boot + capability MAC key provisioning'i sessizce atlıyor
> Kaynak: Claude Code (HIGH) + Codex (MEDIUM) — Claude'un severity'si benimsendi

- **[KAT]** Security / Boot
- **[DOS]** [Cargo.toml:9](Cargo.toml), [src/boot.rs:35-61](src/boot.rs), [src/hal/secure_boot.rs:91](src/hal/secure_boot.rs)
- **[BUL]** `default = ["fast-crypto", "fast-sign"]`. `make build` (Makefile:13-14) `--release` ile bu default'u kullanır → `test-keys` feature'ı KAPALI → `provision_key()` ve `secure_boot_check()` tamamen compile-out edilir. Sonuç: production binary boot ettiğinde:
  1. Kernel imza doğrulaması çalışmaz.
  2. KEY_READY = false kalır → `validate_full()` her token için `false` döner.
  3. cap_invoke fast path `validate_cached()` kullanır; cache hiç doldurulmadığı için tüm cap_invoke E_NO_CAPABILITY ile döner. 3 ardışık fail → CapViolation policy → ISOLATE.

  Ek olarak Codex notu: in-kernel secure boot test-keys ile boş mesajı doğruluyor; production yolunda kernel image/section bütünlük doğrulaması yok.
- **[ETK]** README.md:13 ve ARCHITECTURE.md:36-46 "Capability-based access control" + "Secure boot" özelliklerini v1.0 listesinde sayar; gerçekte default build'de ikisi de etkisiz. Self-test "Secure boot OK" sonucu kernel .text/.rodata/.data bütünlüğünü kanıtlamıyor.
- **[ÖNR]** Üç seçenekten biri:
  (a) `production-keys` feature ekle ve default'a koy (HSM/OTP entegrasyonu yokken QEMU test key'leriyle aynı path);
  (b) `test-keys` olmadan secure_boot_check çağrısı zorunlu hale getir, public key linker symbol olarak gelsin;
  (c) boot.rs'te `#[cfg(not(any(feature = "test-keys", feature = "production-otp")))] compile_error!("...")` ekle.

  README/ARCHITECTURE'a "v1.0 default build secure boot devre dışıdır" notu da ekleyin. Linker-delimited image hash doğrulaması ya da secure boot'un net biçimde external ROM-only kapsamına alınması.
- **[KNT]** `make build` çıktısında provision_key çağrısı yok (boot.rs:35 `#[cfg(feature = "test-keys")]` altında). Cargo.toml:9 default açık. dispatch.rs:267-294 fail counter — ardışık 3 fail isolate.

### H3 — Kani BLAKE3 stub key material'i çıktıya kopyalıyor; "different key → different hash" türü prooflar trivial geçiyor
> Kaynak: Claude Code

- **[KAT]** FV / Doc
- **[DOS]** [src/common/crypto/blake3_impl.rs:55-67](src/common/crypto/blake3_impl.rs), [src/verify.rs:699-725](src/verify.rs)
- **[BUL]** Kani altında `Blake3Provider::keyed_hash` stub'ı `result[i] = key[i]` yapıyor (key'in ilk 16 byte'ını döndürüyor). `blake3_impl.rs:73-75` bu durumu açıkça notlandırmış ve aynı dosyadaki proof'lar `blake3_stub_*` olarak yeniden adlandırılmış. **Ancak** verify.rs:700-712'deki Proof 139 `blake3_different_key_different_hash` ve Proof 140 `blake3_same_input_same_hash` "BLAKE3" ismi taşıyor; gerçekte sadece stub'ın key passthrough davranışını test ediyor. Aynı şekilde Proof 147 (token_mac_field_matches_blake3_output) sadece dizi boyutlarını karşılaştırıyor.
- **[ETK]** Proof sayısı 200/201 olarak ilan ediliyor (kalite kriteri); bu üç proof BLAKE3 cryptographic property iddia eden isimlere sahip ama hiçbir kriptografik özellik kanıtlamıyor — pure passthrough'u test ediyorlar.
- **[ÖNR]** Proof 139'u `stub_passes_through_key_difference` olarak rename + docstring ekleyin: "Stub identity property — NOT cryptographic."
- **[KNT]** blake3_impl.rs:62 `result[i] = key[i]`; verify.rs:711 `assert!(!same)` (key1 ≠ key2 olduğu için stub trivially geçer).

### H4 — UART PMP Entry 7 U-mode'a R+W açık; hostile task syscall/capability bypass ile UART'a doğrudan yazabilir
> Kaynak: Codex

- **[KAT]** Security
- **[DOS]** [src/kernel/memory/mod.rs:107](src/kernel/memory/mod.rs), [src/kernel/memory/mod.rs:113](src/kernel/memory/mod.rs)
- **[BUL]** PMP Entry 7 UART MMIO bölgesini `R | W | L` ile açıyor. Hostile U-mode task syscall/capability/policy katmanlarını bypass ederek 0x1000_0000 UART MMIO'ya doğrudan yazabilir.
- **[ETK]** U-mode task UART gate, rate limiter, policy katmanlarını bypass ederek output flood ve timing DoS yapar. Production'da UART output sadece terminal halt için açıktı; bu erişim threat modeli ihlal eder.
- **[ÖNR]** UART'i PMP eşleşmesi dışında bırakıp M-mode'un unmatched erişimini kullanın (RISC-V spec §3.7.1 U-mode unmatched DENY) veya Smepmp ile M-only MMIO tanımlayın.
- **[KNT]** nl -ba src/kernel/memory/mod.rs → Entry 7 PMP_TOR | PMP_R | PMP_W | PMP_L; UART_BASE=0x1000_0000 src/common/config.rs:71.

### H5 — SYS_YIELD doğrudan schedule() çağırıyor; her yield blackbox tick + IPC sayaç reset üretiyor
> Kaynak: Codex

- **[KAT]** Runtime / Security
- **[DOS]** [src/kernel/syscall/dispatch.rs:431](src/kernel/syscall/dispatch.rs), [src/kernel/scheduler/mod.rs:230](src/kernel/scheduler/mod.rs), [src/kernel/scheduler/mod.rs:286](src/kernel/scheduler/mod.rs)
- **[BUL]** SYS_YIELD doğrudan `schedule()` çağırıyor. `schedule()` her çağrıda blackbox tick artırıyor ve IPC per-tick sayaçlarını sıfırlıyor.
- **[ETK]** U-mode task `yield` spam ile token expiry/cooldown zamanını şişirir ve IPC rate limiter'ı tick dışı resetleyerek DoS yüzeyi açar. Compromised U-mode task threat'i kapsamında.
- **[ÖNR]** Timer tick path ile voluntary-yield scheduling'i ayırın; tick/cooldown/rate reset sadece timer interrupt'ta yapılsın. `schedule_timer_tick()` ve `schedule_yield()` ayrı fonksiyonlar olabilir.
- **[KNT]** dispatch.rs satır 428-435 + scheduler.rs satır 229-289.

### H6 — Unknown exception path sadece debug loglayıp 0 dönüyor; trap.S sadece ecall için mepc += 4 yapar → faulting instruction'a livelock
> Kaynak: Codex

- **[KAT]** Runtime / Correctness
- **[DOS]** [src/arch/trap.rs:214](src/arch/trap.rs), [src/arch/trap.S:57](src/arch/trap.S)
- **[BUL]** Unknown exception path (mcause ≠ ecall_u/ecall_m/timer/illegal/load_fault/store_fault) sadece debug loglayıp 0 döndürüyor. trap.S yalnız ecall için mepc += 4 yaptığı için faulting instruction'a geri dönülür.
- **[ETK]** U-mode breakpoint, misaligned access, unsupported exception sürekli aynı PC'ye dönerek local trap livelock / DoS üretir. Hostile task bunu bilerek tetikleyebilir.
- **[ÖNR]** Bilinmeyen U-mode exception'ları fail-closed `handle_task_fault()` veya isolate path'ine yönlendirin.
- **[KNT]** trap.S satır 57-66 sadece mcause=8/11 advance ediyor; trap.rs satır 214-224 `_ => 0` default arm.

### H7 — task_trampoline caller-saved register clear var; start_first_task() doğrudan mret ediyor, aynı scrub yok
> Kaynak: Codex

- **[KAT]** Security
- **[DOS]** [src/kernel/scheduler/mod.rs:477](src/kernel/scheduler/mod.rs), [src/arch/context.S:107](src/arch/context.S)
- **[BUL]** U-19 hardening'inde `task_trampoline` 16 caller-saved register temizliyor (context.S:107-128). Ancak ilk U-mode geçişi `start_first_task()` doğrudan `csrw...; mv sp; mret` ile gerçekleşiyor ve aynı scrub yok.
- **[ETK]** İlk task kernel register kalıntılarını, pointer/değerleri ve call-site state'ini U-mode'da görebilir. U-19 register-leak hardening ilk task için tamamlanmamış.
- **[ÖNR]** `start_first_task()` içinde ra/a0-a7/t0-t6 temizleyin veya ortak trampoline yolunu kullanın (mret öncesi register clear bloğu refactor edilebilir).
- **[KNT]** context.S satır 107-128 register clear var; scheduler/mod.rs satır 477-488 `csrw...; mv sp; mret` ile scrub yok.

---

## MEDIUM Bulgular

### M1 — mcounteren açıkça ayarlanmıyor (U-mode timing side-channel'a açık olabilir)
> Kaynak: Claude Code

- **[KAT]** Security / CSR
- **[DOS]** [src/arch/csr.rs](src/arch/csr.rs), [src/boot.rs](src/boot.rs)
- **[BUL]** Kod tabanında `mcounteren` yazma yok. RISC-V spec'te bu CSR U-mode'un `rdcycle/rdtime/rdinstret` erişimini kontrol eder. Reset değeri implementation-defined; QEMU virt + bazı bootloader'lar U-mode counter erişimini açık başlatır.
- **[ETK]** Capability MAC validation `ct_eq_16` ile constant-time yapılıyor ama U-mode `rdcycle` çekebiliyorsa BLAKE3 keyed_hash sürecindeki cache hit/miss veya MAC mismatch zamanlaması ölçülebilir. ARCHITECTURE.md threat list'inde "side-channel timing attack" mevcut — kapsam içi.
- **[ÖNR]** boot.rs::init() içine `unsafe { asm!("csrw mcounteren, zero") }` ekleyin. POST'ta da `read_mcounteren() == 0` doğrulayın.
- **[KNT]** `grep -rn "mcounteren" src/` boş döner.

### M2 — medeleg/mideleg açıkça sıfırlanmıyor / doğrulanmıyor
> Kaynak: Claude Code

- **[KAT]** Security / CSR
- **[DOS]** [src/boot.rs](src/boot.rs), [src/tests/mod.rs:471-606](src/tests/mod.rs)
- **[BUL]** M-only kernel için (S-mode yok) medeleg/mideleg sıfır olmalı. Kod hiçbir yerde bu CSR'ları yazmıyor veya POST'ta okumuyor. Reset değeri implementation-defined; bootloader veya firmware non-zero set etmişse exception delegation S-mode'a yönlendirilir → crash/UB.
- **[ETK]** Sipahi'de S-mode yok → delegated trap kayıp olur. mtvec asla çağrılmaz, sistem tamamen ölü kalır.
- **[ÖNR]** boot.rs init'te `csrw medeleg, zero; csrw mideleg, zero`. POST'a okuma + assert ekleyin (mtvec=0 kontrolüyle simetrik).
- **[KNT]** Tüm `medeleg|mideleg` arama boş.

### M3 — write_mtvec mode bit'lerini açıkça maskelemiyor
> Kaynak: Claude Code

- **[KAT]** Correctness / CSR
- **[DOS]** [src/arch/csr.rs:81-84](src/arch/csr.rs), [src/boot.rs:15](src/boot.rs)
- **[BUL]** `write_mtvec(addr)` ham `addr`'ı yazıyor. mtvec[1:0] mode field — 0=direct, 1=vectored, ≥2 reserved. trap_entry .align 4 olduğundan sembol adresi {1:0]=0 (direct), ancak bu implicit. Linker reorganization, symbol re-alignment veya ileride farklı entry point eklenmesi durumunda mode bit'i kazara 1 olabilir.
- **[ETK]** mtvec[1:0]=1 olursa vectored mode aktif ve trap dispatch yanlış adrese atlar. Sistem ya boot edemez ya silent corruption.
- **[ÖNR]** `pub fn write_mtvec(addr: usize) { let val = (addr & !0x3) | 0; ... }`. POST'a `read_mtvec() & 0x3 == 0` da ekleyin.
- **[KNT]** csr.rs:81 doğrudan yazma; tests/mod.rs:511 sadece nonzero kontrol.

### M4 — Linker script sabitleri ↔ config.rs sabitleri compile-time bağlı değil
> Kaynak: Claude Code

- **[KAT]** Correctness / Build
- **[DOS]** [sipahi.ld:69](sipahi.ld), [sipahi.ld:84](sipahi.ld), [src/common/config.rs](src/common/config.rs)
- **[BUL]** Linker `. += 16384;` (kernel stack) ve `ALIGN(8192)` (.task_stacks) hardcoded. config.rs::KERNEL_STACK_SIZE=16384, TASK_STACK_SIZE=8192. İki yerde aynı sayı; build.rs veya const_assert ile bağlı değil. WASM_HEAP_SIZE de linker yorumunda referans veriliyor ama doğrulanmıyor.
- **[ETK]** Sprint 13'te 4KB→16KB değişiminde olduğu gibi gelecekte config değişir, linker güncellenmezse stack overflow'lar PMP-corrupted bölgeye yazar.
- **[ÖNR]** build.rs ekle: linker script'i template'den üret. Alternatif: const _: () = assert! ile linker symbol address'leri runtime check.

### M5 — TaskContext (128 byte) layout için compile-time size assertion yok
> Kaynak: Claude Code

- **[KAT]** Correctness / FV
- **[DOS]** [src/kernel/scheduler/mod.rs:31-49](src/kernel/scheduler/mod.rs), [src/arch/context.S:23-76](src/arch/context.S)
- **[BUL]** context.S `sd ra, 0(a0); sd s0, 16(a0); ...; sd mstatus, 120(a0)` ile sabit offset'lerle TaskContext'e yazıyor. Yapı 16 alan × 8 byte = 128 byte. Ancak `const _: () = assert!(core::mem::size_of::<TaskContext>() == 128)` benzeri compile-time guarantee yok.
- **[ETK]** TaskContext'e bir alan eklenirse veya repr(C) padding değişirse, switch_context bitişik task'ın belleğini siler. Cross-task corruption silent.
- **[ÖNR]** scheduler/mod.rs sonuna ekle: `const _: () = assert!(core::mem::size_of::<TaskContext>() == 128); const _: () = assert!(core::mem::offset_of!(TaskContext, sp) == 8);` her kritik offset için.

### M6 — IPC mesaj integrity (CRC32) opt-in, kernel kanal seviyesinde dayatmıyor
> Kaynak: Claude Code

- **[KAT]** Security / IPC
- **[DOS]** [src/ipc/mod.rs:42-62](src/ipc/mod.rs), [src/kernel/syscall/dispatch.rs:304-426](src/kernel/syscall/dispatch.rs)
- **[BUL]** `IpcMessage::set_crc()` U-mode caller'ın görevi. sys_ipc_send mesajı `core::ptr::read_volatile` ile okuyup ring buffer'a kopyalıyor; CRC hesaplanmıyor. sys_ipc_recv'de verify_crc çağrılmıyor.
- **[ETK]** Hostile producer task corrupted mesaj gönderir; consumer task verify_crc çağırmazsa verisi bozuk şekilde kabul eder. Threat model'de hostile U-mode task var → bu gap.
- **[ÖNR]** İki seçenek: (a) sys_ipc_send mesajı kopyaladıktan sonra kernel `set_crc` çağırsın → sys_ipc_recv `verify_crc` zorunlu, fail E_CORRUPTED. WCET cost ~480c. (b) `IPC_REQUIRE_CRC` config flag default-on.

### M7 — Capability validate_full ordering: nonce write MAC verify sonrası — replay window minimal ama belgesiz
> Kaynak: Claude Code

- **[KAT]** Security / Capability
- **[DOS]** [src/kernel/capability/broker.rs:118-164](src/kernel/capability/broker.rs)
- **[BUL]** Sıralama: cache→KEY_READY→owner_match→nonce_read→expiry→MAC_compute→ct_eq→nonce_write+cache_insert. MIE=0 single-hart context'inde re-entrancy yok, dolayısıyla TOCTOU yok. Ancak belirli MAC valid + expired token kombinasyonunda LAST_NONCE güncellenmez (line 153 sadece success path) → aynı MAC ileri tarihli expiry ile gönderilirse yine kabul.
- **[ETK]** Praktik exploit dar (MAC header expiry'i içeriyor); risk konseptüel: nonce write semantiği belgesiz.
- **[ÖNR]** broker.rs:139-141 yorumuna "Nonce update only on full success — expired-but-MAC-valid path also updates nonce to prevent same-token re-replay after expiry rollover" notu ekleyin VE kodu öyle düzeltin (expiry fail → nonce update).

### M8 — `dispatch_compute` bilinmeyen servis ID'sinde `-1` döner, `compute_mac` short-data hatası da `-1` — caller ayırt edemez
> Kaynak: Claude Code

- **[KAT]** Quality / Correctness
- **[DOS]** [src/sandbox/mod.rs:288-296](src/sandbox/mod.rs), [src/sandbox/mod.rs:316-317](src/sandbox/mod.rs)
- **[BUL]** `dispatch_compute` 4 servis tanımlı, default arm `_ => -1`. `compute_mac` `if data.len() < 32 { return -1; }`. İki anlamlı farklı hata aynı kod ile dönüyor.
- **[ETK]** WASM modülü "service unknown" ve "MAC short data" arasında ayrım yapamaz, debug zor; forensics bilgi kaybı.
- **[ÖNR]** Distinct kodlar: unknown service → -99, short data → -1, NotImplemented → -3. Veya `Result<i32, ComputeError>`.

### M9 — `device.rs` dead trait abstraction; semantik çakışma uart.rs::putc ile
> Kaynak: Claude Code

- **[KAT]** Quality
- **[DOS]** [src/hal/device.rs:18-105](src/hal/device.rs), [src/arch/uart.rs:11-33](src/arch/uart.rs)
- **[BUL]** `uart.rs::putc` 1000-iter THR-empty bekler, sonra char drop. `device.rs::DeviceAccess::write_byte` ilk poll'de hazır değilse `DeviceNotReady` döner. Production code yalnızca `uart::putc` kullanıyor; trait çağrılmıyor, tek implementor `UartDevice` ve hiçbir runtime path'te kullanılmıyor.
- **[ETK]** v2.0'da birinin DeviceAccess'i kullanmaya başlaması durumunda davranış sessizce değişir (drop yerine error). Ölü kod binary footprint ve zihinsel yük.
- **[ÖNR]** Ya HAL trait'ini `#[cfg(feature = "v2-hal")]` arkasına gizleyin, ya v1.0'da silin.

### M10 — verify.rs WCET ordering `WCET_TOKEN_CACHE_HIT <= WCET_YIELD` (10 ≤ 10) — borderline tautoloji, FPGA sonrası kırılgan
> Kaynak: Claude Code

- **[KAT]** FV
- **[DOS]** [src/verify.rs:76](src/verify.rs)
- **[BUL]** İki sabit de 10c. `<=` eşitlikle geçiyor; bu bir invariant değil, coincidence. WCET_YIELD = 10c FPGA ölçümü sonrası 5c'ye düşerse proof kırılır ama anlamsal regresyon yok.
- **[ETK]** FPGA ölçümünden sonra config recalibration sırasında bu proof patladığında kullanıcı düzgün invariant ihlali sanır.
- **[ÖNR]** Proof'u kaldır veya `assert!(WCET_TOKEN_CACHE_HIT < WCET_TOKEN_VALIDATE)` gibi gerçek invariant'a çevir.

### M11 — `deny.toml` "Copyleft yasaklı" yorumu ile gerçek policy uyuşmuyor
> Kaynak: Claude Code

- **[KAT]** Doc / Supply chain
- **[DOS]** [deny.toml:13-27](deny.toml)
- **[BUL]** Yorum: "Copyleft (GPL, MPL-2.0) dep'leri yasaklı". Gerçek `[licenses]` bloğunda sadece `allow = [...]` var, `deny` yok. cargo-deny semantiği: allow listesinde olmayan license'lar configurable behavior'a göre yasaklanır (varsayılan: warn ya da error). Yeni cargo-deny sürümlerinde varsayılan değiştiyse beklenmeyen sonuç çıkabilir.
- **[ETK]** "Yasaklı" iddiası açık compile-time enforcement'a değil, allow-list mekanizmasına dayanıyor.
- **[ÖNR]** Explicit `deny = ["GPL-2.0", "GPL-3.0", "AGPL-3.0", "MPL-2.0"]` ekle veya `unlicensed = "deny"` + `copyleft = "deny"` flag'lerini açıkça yaz.

### M12 — QEMU CI ALL TESTS PASSED görünce QEMU öldürüyor; post-test scheduler/NF regression ve production smoke kontrol edilmiyor
> Kaynak: Codex

- **[KAT]** CI
- **[DOS]** [.github/workflows/ci.yml:132](.github/workflows/ci.yml), [.github/workflows/ci.yml:158](.github/workflows/ci.yml)
- **[BUL]** QEMU CI PASS gördüğünde QEMU öldürüyor; testler geçtikten hemen sonra olabilecek scheduler/NF regression ve production run kontrol edilmiyor. Final checks yalnızca `BOOT HALTED` arıyor; `^NF$` grep'i yok.
- **[ETK]** U-18 nested-fault regression'ı testler geçtikten hemen sonra olursa CI kaçırır. Production binary smoke test CI'da yok.
- **[ÖNR]** PASS sonrası kısa scheduler soak (10-15s) yapın, `^NF$` grep'i ekleyin, ayrı production `make run` smoke job'u ekleyin.
- **[KNT]** CI lines 132-140 PASS polling + kill; final checks only BOOT HALTED, NF yok.

### M13 — `make kani` Kani 0.67.0 ile bozuk: `--all-harnesses` unsupported
> Kaynak: Codex

- **[KAT]** FV / CI
- **[DOS]** [Makefile:55](Makefile)
- **[BUL]** `make kani` mevcut Kani 0.67.0 ile çalışmıyor: `error: unexpected argument '--all-harnesses' found`.
- **[ETK]** Yerel formal verification hedefi false-fail veriyor; kullanıcı Makefile'a güvenirse FV çalıştırılamaz.
- **[ÖNR]** Makefile hedefini CI ile aynı şekilde `cargo kani` (flag'siz) yapın.
- **[KNT]** `make kani` → error: unexpected argument.

### M14 — Production WASM sandbox surface drift: loader/execute call-site yok, .wasm_arena = 0 byte
> Kaynak: Codex

- **[KAT]** Runtime / Doc / Security
- **[DOS]** [src/sandbox/mod.rs:352](src/sandbox/mod.rs), [src/tests/mod.rs:386](src/tests/mod.rs), [sipahi.ld:91](sipahi.ld)
- **[BUL]** Production build'de WASM sandbox için loader/execute call-site yok ve .wasm_arena section boyutu 0; hostile-WASM threat model'i yalnız self-test/demo yolunda egzersiz ediliyor.
- **[ETK]** AMCI öncesi "WASM sandbox escape" kontrolleri production binary'de gerçek bir runtime yolunu temsil etmiyor; güvenlik iddiası ile çalışan binary arasında drift oluşuyor.
- **[ÖNR]** v1.0'da WASM production surface isteniyorsa capability korumalı loader/syscall yolu ekleyin ve arena/runtime'i release'te `KEEP`/used hale getirin; istenmiyorsa dokümanlarda WASM'i self-test/prototype kapsamına alın.
- **[KNT]** rg WasmSandbox → yalnız tests/mod.rs ve sandbox/mod.rs; cargo objdump --release --section-headers → .wasm_arena 00000000.

### M15 — ct_eq_16 LTO ile inline olunca disassembly script "Manual review needed" deyip exit 0; CI continue-on-error
> Kaynak: Codex

- **[KAT]** FV / Security
- **[DOS]** [scripts/verify-ct-eq.sh:24](scripts/verify-ct-eq.sh), [.github/workflows/ci.yml:71](.github/workflows/ci.yml)
- **[BUL]** ct_eq_16 LTO ile inline olunca script "Manual review needed" deyip exit 0 veriyor; CI'da `continue-on-error: true` ile informational.
- **[ETK]** Timing side-channel regression otomatik yakalanmıyor.
- **[ÖNR]** Call-site disassembly için branch pattern gate'i ekleyin (objdump call-site etrafında branch yokluğunu doğrulayın) ve job'u release-blocking yapın.
- **[KNT]** `cargo nm --release | grep ct_eq_16` boş; `bash scripts/verify-ct-eq.sh` → manual review mesajı.

---

## LOW Bulgular

### L1 — Kani harness sayısı dokümanda 200, gerçekte 201
> Kaynak: Claude Code

- [README.md:88](README.md), [ARCHITECTURE.md:58](ARCHITECTURE.md), docs/sipahi_features_*.md → "200"; `grep -rn "kani::proof" src/ | wc -l` → 201. Trivial drift.

### L2 — Cargo.toml caret dependency versioning, exact pin yok
> Kaynak: Claude Code (LOW) + Codex (MEDIUM) — Codex severity ile MEDIUM bağlamı korundu

- [Cargo.toml:22-26](Cargo.toml): `wasmi = "1.0.9"` (caret), `blake3 = "1"` (≥1.0 <2.0), `ed25519-dalek = "2"` (≥2.0 <3.0). Cargo.lock pinli olduğundan reproducible build sağlanır, ancak safety-critical doctrine için `=1.8.4` stili explicit pin daha temiz. CI'da `--locked` continue-on-error olduğu için drift CI tarafından yakalanmaz. Gating `cargo build --locked` da kullanılmıyor.

### L3 — main.rs versiyon banner v1.5 yazıyor, audit hedefi v1.0, Cargo version "0.1.0"
> Kaynak: Claude Code

- [src/main.rs:81](src/main.rs): `"Sipahi Microkernel v1.5"`. Bu audit "v1.0" hedefli, Cargo.toml:3 `version = "0.1.0"`. Üçü tek noktaya hizalansın veya banner `env!("CARGO_PKG_VERSION")` ile üretilsin.

### L4 — "MIE=0 in trap context" SAFETY notu — context dışında çağrılabilen path'ler için doğrulama eksik
> Kaynak: Claude Code

- broker.rs:172, scheduler.rs:512 vb. SAFETY yorumları "MIE=0 in trap context" diyor. Boot sequence'de (init() öncesi mret henüz yok) bu doğru ama compile-time enforce edilmiyor. Yanlış yerden çağrı sessizce derlenir. Type-level guard (örn. `TrapToken<'a>` parametre) v2.0 hardening listesi.

### L5 — Watchdog window_min=3 hardcoded, task-spesifik override yok
> Kaynak: Claude Code

- config.rs::WATCHDOG_WINDOW_MIN=3. Budget'ı 1 tick'lik task'lar bu window violation tetikleyebilir. v1.0 task config'i (boot.rs) period_ticks=10 olduğu için sorun değil ama gelecekte ekstrema task eklenirse sürpriz.

### L6 — verify.rs Kani harness'lerde `.unwrap()` antipattern (`assert!(option.is_some())` + `unwrap()`)
> Kaynak: Claude Code

- [src/verify.rs:19-21](src/verify.rs), [src/verify.rs:194-200](src/verify.rs). Pattern: `assert!(x.is_some()); let r = x.unwrap();`. Cosmetic; gereksiz iki kontrol. Fix: `kani::assume(x.is_some())` + `unwrap`.

### L7 — verify.rs `for` iterator kullanımı, projenin geri kalanı `while i < N` doctrine'ına aykırı
> Kaynak: Claude Code

- [src/verify.rs:31](src/verify.rs), [src/verify.rs:95](src/verify.rs). Üç yerde `for x in &arr` kullanılıyor. Diğer tüm dosyalar `while i < n { ... ; i += 1; }` pattern'ini takip ediyor. Style drift; mini cleanup PR ile düzeltilebilir.

### L8 — `BB_NEXT_SEQ` u32 wrap sınırı 23 yıl ama wrap davranışı belgesiz
> Kaynak: Claude Code

- [src/ipc/blackbox.rs:142-143](src/ipc/blackbox.rs), [src/ipc/blackbox.rs:260](src/ipc/blackbox.rs). Yüksek event hızında (saniyede yüzlerce) wrap çok daha erken gelir; post-mortem analyzer aynı seq görür → eski/yeni karışır.
- ÖNR: seq u64 yap (BLACKBOX_SIZE 8KB→9KB) veya BB_SEQ_EPOCH artır.

### L9 — `fmt.rs` print_u32/u64/hex magic literal `i < 10/20/16` vs buffer boyutu
> Kaynak: Claude Code

- [src/common/fmt.rs:19](src/common/fmt.rs), [src/common/fmt.rs:33](src/common/fmt.rs), [src/common/fmt.rs:46](src/common/fmt.rs). `let mut buf = [0u8; 10]; ... while val > 0 && i < 10` — magic literal buffer boyutuyla manual sync. Fix: `i < buf.len()`.

### L10 — `uart.rs::putc` 1000-iter bound FPGA hardware'a uygun değil olabilir
> Kaynak: Claude Code

- [src/arch/uart.rs:13-30](src/arch/uart.rs). Yorum "1000 iter × 3c = 3000c = 30µs" diyor. Gerçek: 115200 baud'da byte başına ~87µs. CVA6 100MHz'de 1000 iter ≈ 10µs — FPGA'da char drop. QEMU "instant ready" olduğundan QEMU testi yakalamaz.
- ÖNR: `UART_POLL_LIMIT` config sabiti yapın; FPGA bring-up sonrası ölçümle ayarlayın.

### L11 — STRUCTURE/ARCHITECTURE "TLC v2.19, 35,770 distinct states" iddiası repo'da kanıtlı değil
> Kaynak: Claude Code

- 7 .tla dosyası mevcut; TLC çalıştırma script'i veya .out/.log dosyası repo'da yok. CI'da TLC adımı yok. "35,770 distinct states" sayısı manuel iddia.
- ÖNR: `Tla+/run_tlc.sh` + her spec için `Tla+/results/*.out`. CI'a opsiyonel TLC step. Veya iddiayı yumuşat.

### L12 — `Token::_pad: [u8; 2]` repr(C) padding, MAC computation pad'i sıfır kabul ediyor
> Kaynak: Claude Code

- [src/kernel/capability/token.rs:36-37](src/kernel/capability/token.rs), [src/kernel/capability/token.rs:66-67](src/kernel/capability/token.rs). `_pad` field'ı struct'ta var, `header_bytes()` h[6]=0/h[7]=0 hardcoded. Şu an exploitable değil; transmute/bytemuck ile değiştirilirse padding undefined behavior, MAC nondeterministic.
- ÖNR: `const _: () = assert!(Token::zeroed().header_bytes()[6] == 0 && [7] == 0)` veya doc.

### L13 — Blackbox doküman seq:2B/data:46B diyor; kod seq:u32 (4B), data:42B
> Kaynak: Codex

- **[KAT]** Doc
- **[DOS]** [docs/sipahi_features_tr.md:265](docs/sipahi_features_tr.md), [src/ipc/blackbox.rs:68](src/ipc/blackbox.rs)
- **[BUL]** Doküman blackbox seq alanını 2B ve data alanını 46B diyor; kod `seq: u32` (4B) ve `data: [u8; 42]`.
- **[ETK]** Post-mortem parser yazacak kişi yanlış layout kullanabilir.
- **[ÖNR]** TR/EN feature docs'u SEQ:4, DATA:42 olarak güncelleyin.
- **[KNT]** blackbox.rs satır 64-73 gerçek byte layout'u gösteriyor.

### L14 — Linker script `/DISCARD/` tanımlamıyor; production ELF'te `.eh_frame` 0x460 byte kalıyor
> Kaynak: Codex

- **[KAT]** Quality / Linker
- **[DOS]** [sipahi.ld:30](sipahi.ld)
- **[BUL]** Linker script /DISCARD/ tanımlamıyor; production ELF'te `.eh_frame` 0x460 (1120) byte kalıyor.
- **[ETK]** no_std/release yüzeyi ve binary boyutu gereksiz büyüyor.
- **[ÖNR]** `/DISCARD/ : { *(.eh_frame) *(.got) *(.eh_frame_hdr) }` ekleyin.
- **[KNT]** `cargo objdump --release -- --section-headers` → `.eh_frame 00000460`.

### L15 — sandbox/mod.rs WASM yorum/metrikleri stale (64KB ve CRC ~120c)
> Kaynak: Codex (örtüşüyor: Claude Code "stale comments" — ayrı listelendi)

- **[KAT]** Quality / Doc
- **[DOS]** [src/sandbox/mod.rs:16](src/sandbox/mod.rs), [src/sandbox/mod.rs:39](src/sandbox/mod.rs), [src/sandbox/mod.rs:281](src/sandbox/mod.rs), [src/common/config.rs:65](src/common/config.rs)
- **[BUL]** sandbox.rs yorumu "64KB" ve "CRC ~120c" diyor; config gerçekte WASM_HEAP_SIZE=4MB ve CRC WCET=1500c.
- **[ETK]** Kod okuyan kişi modül boyut limiti ve WCET maliyetini 30x/12x yanlış yorumlayabilir.
- **[ÖNR]** Tek kaynak olarak config.rs sabitlerini referanslayan yorum kullanın; proof isim/yorumlarında sabit sayı yerine `WASM_HEAP_SIZE` yazın.

### L16 — TLA+ Scheduler header "starvation freedom" verify edildiğini söylüyor; cfg ise StarvationFreedom'i bilinçli devre dışı bırakıyor
> Kaynak: Codex

- **[KAT]** FV / Doc
- **[DOS]** [Tla+/SipahiScheduler.tla:5](Tla+/SipahiScheduler.tla), [Tla+/SipahiScheduler.cfg:17](Tla+/SipahiScheduler.cfg)
- **[BUL]** Scheduler TLA+ header'ı "starvation freedom" verify edildiğini söylüyor, cfg ise `StarvationFreedom`'i bilinçli olarak devre dışı bırakıyor (fixed-priority preemptive'de tutmaz — DAL-A her zaman çalışır, DAL-D starve edilebilir, design intent).
- **[ETK]** Formal verification kapsamında yanlış güven oluşur; DAL-D starvation tasarım kabulünün üzeri kapanabilir.
- **[ÖNR]** Header'ı "priority correctness/state invariants; starvation intentionally not verified for fixed-priority policy" diye daraltın.
- **[KNT]** Scheduler.cfg satır 17-24 StarvationFreedom'in verify edilmediğini açıkça not ediyor ve PROPERTY listesine almıyor.

---

## INFO

### I1 — TLA+ specs repo'da, "35,770 distinct states" iddiası manuel
> Kaynak: Claude Code

7 .tla dosyası mevcut. TLC log dosyası repo'da yok. CI'da TLC step yok. (L11 ile birleşik konu.)

### I2 — CI `--locked` ve `unsafe documentation` kontrolleri continue-on-error
> Kaynak: Claude Code

[ci.yml:54-78](.github/workflows/ci.yml). Reproducible build verification (double-build + sha256 compare) yok. (M12 ile birleşik konu.)

### I3 — `schedule()` 174 satır, 4 phase + 2 early-return path
> Kaynak: Claude Code

WCET için inline kalmalı. Phase'ler `#[inline(always)]` ile private helper fn'lere ayrılabilir, WCET aynı kalır, okurluk artar.

### I4 — `has_float_opcodes` 0xFC sub-opcode > 127 (LEB128) edge case
> Kaynak: Claude Code

[src/sandbox/mod.rs:241-266](src/sandbox/mod.rs). `0xFC 0x80 0x01 ...` (LEB128 sub-opcode > 127) edge case beklenmedik tarafa kaçabilir mi? `skip_instruction` içinde aynı kontrol kapsamlı görünüyor.

---

## Past-Bug Regression Matrix (her iki audit konsolide)

| Sprint | Düzeltilen Bug | Regression Guard | CI Catch? | Audit Notu |
|---|---|---|---|---|
| U-16 | `is_valid_user_ptr` tüm ptr kabul | Kani Proof 157 + `test_cross_task_pointer_rejected` | ✓ | Codex+Claude OK |
| U-16 | Token owner mismatch | Kani `token_owner_mismatch_always_rejected` + self-test | ✓ | Codex+Claude OK |
| U-16 | IPC default allow | Kani `unassigned_channel_denies_any_caller` + wrong-owner test | ✓ | Codex+Claude OK |
| U-16 | Watchdog Ready task cezalandırma | scheduler.rs:286-310 sadece Running'de + INFO `info_ready_task_watchdog` | ⚠ INFO-only | Claude: assertion'lı test gerekli |
| U-16 | schedule() tek task güvenlik skip | scheduler.rs:225-227 Phase 2 sonrası early return; QEMU runtime test'i tek task path'i kapsamıyor | ⚠ | Codex+Claude: kısmi |
| U-16 | Allocator wrapping_add | Kani `bump_allocator_offsets_never_overlap` + `test_allocator_overflow` | ✓ | OK |
| U-17 | Lockstep CSE optimize | policy.rs black_box fence — disassembly check `verify-ct-eq.sh` informational | ⚠ | M15 paralel |
| U-18 | task_trampoline NF | QEMU self-test NF-marker grep yok (M12); Codex `^NF$` post-grep önerdi | ✗→⚠ | Codex: CI'da yok, M12 ile bağlı |
| U-19 | task_trampoline reg leak | context.S:112-127 16 register clear; runtime test yok | ✗ | Codex+Claude: yok; ayrıca H7 (start_first_task aynı scrub yok) |
| U-19 | helper inline scheduler | Kani Proof 71/95 helper kullanıyor | ✓ | OK |

⚠/✗ olanlar: regression guard yetersiz veya yok → bug reintroduce edilirse sessizce geri gelebilir.

---

## Attack Scenario Walkthrough Sonuçları

### Scenario 1 — WASM sandbox escape

1. **0xFC saturating trunc gizleme** → sandbox/mod.rs:250-258 yakalar (sub ≤ 0x07 reject) ✓
2. **Allocator overflow** → checked_add + arena bound ✓
3. **Sonsuz loop** → fuel metering ✓
4. **AÇIK (Codex M14):** Production build'de WASM loader/execute call-site yok, .wasm_arena = 0 byte → hostile-WASM senaryosu yalnız self-test'te egzersiz ediliyor.
5. **AÇIK (Claude):** Fuel exhaustion'da policy isolation otomatik mi? sandbox/mod.rs:412 SandboxError döner; çağıran tarafın isolate çağırması gerekiyor. v2.0.

### Scenario 2 — Compromised U-mode task

1. **Cross-task pointer** → task_stack_range reject ✓
2. **Sealed channel impersonation** → can_send/can_recv default deny + seal_channels ✓
3. **Boot sırasında IPC seal başarısız** → halt_system fail-closed ✓ (default features test-keys olmadığı için cap_invoke zaten devre dışı — H2)
4. **Illegal instruction privilege escalation** → trap.rs verify_mpp_is_user_mode (M-mode'a yükselme tespit) ✓
5. **AÇIK (Codex H4):** UART MMIO PMP entry 7 R+W açık → policy/rate bypass mümkün
6. **AÇIK (Codex H5):** SYS_YIELD direct schedule() → tick/cooldown/IPC reset spam ile DoS
7. **AÇIK (Codex H6):** Unknown exception → trap livelock DoS
8. **AÇIK (Codex H7):** start_first_task ilk U-mode geçişte register leak

### Scenario 3 — Hardware glitching

1. **PMP register glitch** → schedule() her tick verify_pmp_integrity ✓
2. **decide_action SEU** → policy.rs lockstep + black_box fence ✓
3. **AÇIK (Codex M15 + Claude L4):** ct_eq_16 black_box derleyici barrier — RISC-V ISA garanti değil, future LLVM upgrade'da hoist edilebilir. Disassembly check informational.
4. **mscratch glitch** → trap.S beqz nested fault → "NF" UART → wfi park ✓
5. **AÇIK (Codex M12):** CI'da `^NF$` post-test grep yok → glitch sonrası NF regression görünmez

---

## Olumlu Bulgular — Senior Göstergeleri

> Her iki audit'in olumlu bulguları konsolide; çakışan başlıklar tek satır.

### Single source of truth
- `is_selectable_by_scheduler` / `is_period_reset_eligible` / `should_watchdog_timeout` helper'ları hem production scheduler'da hem Kani harness'inde kullanılıyor (scheduler/mod.rs:927-943). Drift imkansız.
- `pack_pmpcfg` const fn — boot, Kani, runtime aynı.
- `decide_action` pure const fn, lockstep'te de Kani'de de aynı.

### Compile-time invariants
- `const _: () = assert!(TRAP_FRAME_SIZE - TRAP_FRAME_USER_SP_OFFSET == 16, ...)` (config.rs:56-60)
- IpcMessage size=64, SIGNATURE_SIZE = 2*OTP_KEY_SIZE, BLACKBOX_MAX_RECORDS<=255, Token size=32
- `compile_error!` for mutually exclusive features ve multi-hart guard

### Volatile macro disiplini (LTO-safe)
- `vol_read!`/`vol_write!` macros static mut'ı LTO + `opt-level="s"` altında register caching'den koruyor

### Defense-in-depth
- PMP shadow + L-bit + per-task NAPOT + watchdog + lockstep + nested fault park
- Pointer validation: `is_valid_user_ptr` task_stack_range tabanlı (default deny Dead/Isolated)
- Dispatch'te kernel pointer scrubbing: syscall sonucu RAM_BASE arası ise E_INTERNAL döner

### Token kriptografi disiplini
- `header_bytes()` explicit byte construction — endian-agnostik. Memory aliasing/transmute YOK.
- `ct_eq_16` const-time XOR + black_box fence

### `SingleHartCell` pattern
- `static mut` sıfır
- Multi-hart compile_error guard

### No panic/no alloc/no float discipline
- `panic = "abort"`, `overflow-checks = true`, `lto = true`, `codegen-units = 1`
- Production'da unwrap/expect 0 (sadece test/Kani'de)

### Boot fail-closed
- secure_boot fail → halt; IPC seal fail → halt; task creation fail → halt; POST CRC/PMP/mtvec/BLAKE3/Ed25519 fail → halt

### Trap-frame ABI symmetry
- trap.S entry/exit + context.S task_trampoline mscratch invariant'ını tüm path'lerde koruyor
- Yorumlar (özellikle context.S:78-98) WHY açıklıyor, mekanik akış değil

### Lockstep with input black_box
- policy.rs:162-171 hem input hem output black_box fence — CSE saldırısına ek güçlendirme

### Test fail criteria explicit
- tests/mod.rs:877-889 toplam fail count > 0 → halt; "BOOT HALTED" CI tarafından grep ile yakalanıyor

### Reproducible build (Codex doğruladı)
- `make clean && make build` iki kez byte-identical sonuç verdi
- SHA256: `b29ac5374d5a1931d241220e8de9aa3becd3537c0c8acbfd1860d70ec99783a4`
- Production binary: 33,536 byte

### Build & test runtime (Codex doğruladı)
- `make build`, `make check`, `cargo audit`, `cargo deny check`, `cargo kani`, 7/7 TLA+ PASS
- QEMU self-test: `ALL TESTS PASSED`, 12 PASS, 0 FAIL; `NF` marker yok

---

## Kod Kalitesi — Senior Bar Değerlendirmesi (Claude Code Pass 3)

### Senior bar üzerinde olanlar
1. Tek doğruluk kaynağı (yukarıda detaylandı)
2. Compile-time invariants
3. Volatile macro disiplini
4. Defense-in-depth
5. Token kriptografi disiplini
6. SingleHartCell pattern
7. No panic/alloc/float discipline
8. Trap-frame ABI symmetry

### Pekiştirme alanları (senior bar altında değil ama geliştirilebilir)

1. **Stale comments / Sprint references** — ~10 farklı stale yorum bulundu. main.rs banner v1.5 vs Cargo 0.1.0 vs audit v1.0; sandbox/mod.rs:281 "WCET CRC ~120c" config 1500. (L3 + L15)
2. **`#[allow(dead_code)]` enflasyonu** — 60+ tekil + 3 modül-seviyesi. Çoğu rasyoneli var ama device.rs tüm trait dead (M9). Senior bar: gerçekten "future API" mi yoksa "shipped but unused" mı ayrımı netleşmeli.
3. **DRY ihlali — duplicate LEB128:** `read_u32_leb128` (sandbox/mod.rs:64) ve `read_leb128_u32` (sandbox/mod.rs:146) işlevsel olarak aynı. Tek implementasyon + adapter wrapper gerekli.
4. **`schedule()` cyclomatic complexity** — 174 satır, 4 phase. WCET için inline kalmalı. Phase'ler private `#[inline(always)]` helper fn'lere ayrılabilir. (I3)
5. **`#[inline]` kullanım tutarsızlığı** — csr.rs tüm CSR helper'ları `#[inline(always)]` ✓; clint.rs `read_mtime` no inline; blackbox.rs `count()`, `current_tick()`, `get_tick()` no inline. Trap-handler hot path'inde indirect call olursa scheduler tick WCET artar.
6. **String table'lar Unicode** — test mesajlarındaki `✓`, `✗`, `★` UTF-8 byte'ı UART'a yazıyor; terminal görüntülerse OK ama POST log post-mortem analizde garbage olabilir. ASCII-only `[OK]`/`[FAIL]` daha taşınabilir.
7. **WCET comment vs sabit drift** — sandbox/mod.rs:281 yorum config.rs ile drift (L15).
8. **`task_trampoline` register clear sırasının gizli kuralı** — context.S:107-127 yorumda "t0/t1 yukarıda kullanıldı — temizleme EN SON" yazıyor. Doğru ama makinece zorlanmıyor; runtime negative test yok.
9. **`broker::sign_token` sadece self-test'de kullanılıyor** — Production'da HSM token üretir; runtime sign_token çağrılmaz. Şu an `#[allow(dead_code)]`. v1.0 production'da capability sistemi tamamen by-pass (H2).

### Performans Gözlemleri (Claude Code)

Hot path'lerde ölçülebilir gereksiz iş **yok**. Tek mikro-optimizasyon adayı: `blackbox::log` byte copy `core::ptr::copy_nonoverlapping` (~5-10c savings, M değer/maliyet borderline). Diğer her şey ya zaten optimize ya defense-in-depth için kasıtlı.

### Codex Kod Kalitesi Analizi

- 78 tracked dosya envanteri alındı; 42 src dosyası, linker, Cargo/Make/CI, 7 TLA+ spec + cfg, scriptler, docs ve placeholder test modülleri okundu.
- `make check` PASS (cargo clippy `-D warnings`); production yolunda `unwrap()/expect()` yok. Gözlenen `unwrap()` Kani proof içinde, `panic` yalnızca panic handler mesajında.
- Unsafe yoğunluğu 123 blok / 9.1K Rust LOC (~13.5/kLOC); büyük çoğunluk SAFETY yorumlu. Mevcut safety-audit script satır-bazlı olduğu için multi-line SAFETY bloklarını false-positive raporluyor; script CI gate'i olmaya hazır değil.
- Üretim kalitesi genel olarak senior seviyeye yakın: bounded loop tercihleri, fail-closed default'lar, `#[must_use]`, compile-time asserts, tek kaynak config sabitleri ve Kani/TLA bağlantısı iyi.
- TCB minimizasyonu için en büyük kalite borcu: 64 adet `dead_code` allow ve 3 blanket allow (`config.rs`, `csr.rs`, `sandbox/mod.rs`). Çoğunun rasyoneli var; yine de v1.0 release için `sandbox`, `iopmp`, HAL v2.0 ve diagnostic yüzeyini feature-gate etmek binary/TCB okurluğunu iyileştirir.
- `tests/fi/mod.rs`, `tests/integration/mod.rs`, `tests/unit/mod.rs` sadece placeholder yorum içeriyor; gerçek self-test/FI kapsamı `src/tests/mod.rs` içinde. Bu yanlış değil ama repo okuma kalitesini düşürüyor.

---

## Düşük Maliyetli Güvenlik Kazançları (Codex)

- **UART PMP** entry'sini kaldırmak veya M-only yapmak: en yüksek fayda/düşük maliyet; U-mode output flood ve timing bypass'ı kapatır (H4)
- **`schedule_timer_tick()` ile `schedule_yield()` ayrımı**: yield spam'in tick/cooldown/rate-limit state'ini bozmasını kapatır (H5)
- **Unknown exception default'unu `handle_task_fault()`/isolate yapma**: trap livelock'u fail-closed davranışa çevirir (H6)
- **`start_first_task()` register scrub ekleme**: U-19 trampoline hardening'ini ilk task için de tamamlar (H7)
- **Production WASM** ya gerçek loader/capability path ile açılsın ya da feature kapalı/doküman self-test kapsamına çekilsin; gri alan bırakmayın (M14)
- Boot'ta **`mcounteren=0`, `mcountinhibit` politikası ve `medeleg/mideleg=0`** yazımını açık hale getirin; side-channel ve privilege delegation yüzeyini ucuzca sertleştirir (M1, M2)
- CI'ya release-blocking **`cargo objdump` guard'ları** ekleyin: `.eh_frame == 0`, float instruction yok, `ct_eq` call-site branch-free, `.wasm_arena` beklenen moda göre 0 veya 4MB (L14, M15)

---

## Önerilen v1.5 / v2.0 Sertleştirme Listesi

> Öncelik sırasına göre konsolide edildi.

### v1.0 → v1.0.1 (production-blocker patch — 6 HIGH bulgu)
1. **H1**: POST CLINT + misa kontrolleri WARN → HALT
2. **H2**: Default features production-ready (test-keys olmadığında compile_error veya production-otp feature)
3. **H4**: UART PMP entry'i M-only yap (Smepmp veya unmatched)
4. **H5**: SYS_YIELD ile timer tick path'lerini ayır (schedule_timer_tick / schedule_yield)
5. **H6**: Unknown exception → handle_task_fault/isolate (fail-closed)
6. **H7**: start_first_task register scrub (ilk U-mode geçişte de)

### v1.5 (sertleştirme paketi, ~300-400 satır)
7. **M1**: mcounteren=0 (timing side-channel kapatma)
8. **M2**: medeleg/mideleg=0 explicit + POST verify
9. **M3**: write_mtvec mode bits explicit mask
10. **M4**: build.rs ile linker ↔ config compile-time bağlama
11. **M5**: TaskContext size + offset_of const_assert
12. **M6**: IPC CRC kernel-side enforce (send compute + recv verify)
13. **M7**: validate_full nonce update semantik dokümantasyon + expired-but-valid path düzeltmesi
14. **M12**: CI scheduler soak + `^NF$` post-grep + production smoke
15. **M13**: `make kani` `--all-harnesses` flag temizliği (CI ile align)
16. **M14**: WASM production kapsam kararı (loader yolu veya feature gate)
17. **M15**: ct_eq disassembly gating release-blocking
18. **L2**: Exact dependency pins (`=1.x.y` stili)
19. **L14**: `/DISCARD/` linker temizliği (.eh_frame, .got, vb.)
20. **H3 + M10**: Kani proof renaming (stub_*) + WCET ordering tautoloji temizliği

### v2.0 / Hardening
21. **Smepmp / .secure_data carve-out**: MAC_KEY/PMP_SHADOW/LAST_NONCE U-mode-deny
22. **Type-level trap-context guard**: SingleHartCell::get() yerine `get(t: &TrapToken)` ile compile-time MIE=0 invariant (L4)
23. **CI extensions**: TLC step ekle, `--locked` enforce, double-build determinism check (L11, I2)
24. **M8, M9, M11**: dispatch_compute distinct error codes; device.rs trait feature-gate veya silme; deny.toml explicit deny block
25. **L6-L13, L15-L16 cleanup**: tek mini PR (~50-80 satır)

### Past-bug regression test gap'leri (öncelik düşük ama gerekli)
- U-19 task_trampoline register clear için negative test
- U-16 schedule() tek-task path için runtime test
- U-16 Ready watchdog için INFO yerine assertion'lı test
- U-18 NF için CI grep guard (M12 ile bağlı)
- U-17 lockstep CSE için disassembly gate (M15 ile bağlı)

---

## Performance Baseline (AMCI Öncesi Referans)

> QEMU TCG cycle counter modu instruction count → gerçek WCET değil.
> AMCI öncesi göreceli karşılaştırma için kayıt edildi.

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

**Multi-hart sonrası regression detection:** her path için actual rdcycle ölçümü WCET_LAST'a düşüyor (dispatch.rs:128-138). check_wcet_limits self-test build'de runtime check yapıyor. AMCI sonrası bu sayıların aynı sırada kalması beklenir.

**Codex notu:** Self-test WCET ölçümü QEMU TCG'de informational olarak `syscall 0 max=613854 limit=25` aştı; FPGA ölçümü hâlâ gerekli.

---

## Build Determinism

**Codex doğrulama sonucu:**
- `make clean && make build` iki kez byte-identical sonuç verdi
- Reproducible: **EVET**
- Production binary: **33,536 byte**
- SHA256: `b29ac5374d5a1931d241220e8de9aa3becd3537c0c8acbfd1860d70ec99783a4`

**CI gap (M12 + I2):** Double-build + sha256 karşılaştırma adımı CI'da yok. v1.5'te eklenmesi önerilir.

---

## Metrik Özet (her iki audit konsolide)

| Metric | Value | Audit Notu |
|---|---|---|
| LOC | 9,132 Rust + 321 ASM | Codex+Claude eşit ölçtü |
| Source files | 39 .rs + 3 .S | |
| Kani proofs | **201** (README/docs claim 200) | L1 — drift |
| TLA+ specs | 7 (TLC verification logs not in repo) | I1, L11 |
| Compile-time asserts | 8+ const_assert | TRAP_FRAME, IPC_MSG_SIZE, SYSCALL_COUNT, multi-hart guard, feature exclusivity, IPC slots, Token, Blackbox |
| Production binary | 33,536 byte (~33 KB) | Codex doğruladı |
| Binary SHA256 | b29ac537...99783a4 | Codex |
| Reproducible build | EVET (byte-identical) | Codex doğruladı; CI'da otomatik check yok |
| unsafe blocks | 123 | SAFETY-comment coverage advisory in CI |
| unsafe density | ~13.5/kLOC | safety-critical kernel için makul |
| Negative tests | 6 + 1 INFO | cross-task ptr, owner, IPC, PMP, blackbox, allocator, watchdog |
| Dependency count | 3 direct (wasmi, blake3, ed25519-dalek) | tümü caret (L2) |
| CI gates | 5 jobs | clippy+build, qemu-test, audit, kani-full (master), kani-fast (PR) |
| Production NF count | 0 (U-18 fix) | M12: post-test grep yok |
| `dead_code` allow | 64 tekil + 3 blanket | M9 device.rs gerçekten dead |
| `cargo audit` | PASS | 0 CVE |
| `cargo deny` | PASS w/ warnings | M11 license deny block boş |
| `cargo kani` | 200/200 PASS (cargo kani) | M13: `make kani --all-harnesses` broken |
| TLA+ TLC | 7/7 PASS (manuel — repo'da log yok) | L11 |
| Toplam bulgu | 0 CRITICAL · 6 HIGH · 15 MEDIUM · 16 LOW · 4 INFO | 41 unique bulgu |

---

## Sonuç — Senior Bar Değerlendirmesi

**Sipahi v1.0 senior+ bar üzerinde** mimari ve disiplin açısından. Compile-time invariants, single source of truth (helper-extracted Kani harness'ler), volatile macro disiplini, fail-closed defaults, defense-in-depth — hepsi yerinde. **CRITICAL sınıfı bug bulunmadı.**

**AMCI multi-hart geçişine engel olabilecek 6 HIGH bulgu var:**
- H1 (POST WARN→HALT)
- H2 (default features secure boot kapalı)
- H3 (BLAKE3 stub proof'lar misleading)
- H4 (UART PMP açık)
- H5 (SYS_YIELD scheduler tick)
- H6 (unknown exception livelock)
- H7 (start_first_task register leak)

**HIGH'ların 4'ü** (H4, H5, H6, H7) sadece Codex tarafından yakalandı — Claude Code yakalamamıştı. **3'ü** (H1, H2, H3) Claude Code tarafından yakalandı — Codex yakalamamıştı (H2 Codex MEDIUM olarak gördü). İki audit'in birbirini tamamlayıcı niteliği görüldü.

**Geri kalan 15 MEDIUM + 16 LOW** sertleştirme + cleanup kategorisinde; ~400-500 satırlık birkaç PR ile kapatılabilir. AMCI öncesi yapılırsa multi-hart geçişinde sürpriz minimize edilir.

**Genel kanaat:** Kod tabanı, "safety-critical RTOS for RISC-V" iddiasını destekleyecek şekilde inşa edilmiş; senior bar atlanmamış, sadece birkaç köşede pekiştirme bekliyor. AMCI'ye geçmeden önce **6 HIGH bulgunun tamamını** kapatın; MEDIUM'lar paralel olarak v1.5 sertleştirme paketinde toplanabilir.

---

*Birleşik audit raporu — Codex (CODEX_AUDIT.md) + Claude Code (claude code audit.md) bağımsız read-only audit'lerinin konsolidasyonu. Hiçbir kaynak dosya değiştirilmedi.*
