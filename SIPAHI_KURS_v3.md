# SİPAHİ — İÇİNDEN GEÇEN KURS

> Sen Sipahi'yi yaptın ama Sipahi'nin içinden geçmedin.
> Bu kurs seni Sipahi'nin içinden geçirecek.
> Bitirdiğinde birisi "bu satır ne yapıyor?" diye sorduğunda bileceksin.

---

## BÖLÜM 1: SİPAHİ'NİN KALP ATIŞI

Güç düğmesine basıyorsun. İşlemci uyanıyor. Hiçbir şey bilmiyor — bellekte ne var bilmiyor, kim olduğunu bilmiyor. Tek bildiği şey: "0x80000000 adresindeki komutu çalıştır." Bu adres donanıma gömülü, değiştirilemez. İşlemci o adrese bakıyor ve Sipahi'nin hikayesi başlıyor.

### 1.1 — İlk Nefes: boot.S

İlk çalışan dosya `src/arch/boot.S`. Bu dosya assembly — Rust değil. Neden? Çünkü Rust çalışmak için bir ortam istiyor: stack lazım, bellekte temiz alan lazım. Hiçbiri yok. boot.S bu ortamı hazırlıyor.

İşlemci uyanıyor, boot.S'in `_start` etiketine bakıyor ve şu adımları yapıyor:

**Adım 1 — "Ben kimim?"**

```asm
csrr    a0, mhartid     # "Ben hangi çekirdekteyim?" diye soruyor
bnez    a0, .park        # Eğer çekirdek 0 değilsem dur, bekle
```

Düşün ki bir fabrikada 4 işçi aynı anda uyanıyor. Ama işi sadece 1 kişi yapacak — işçi 0. Diğerleri "ben 0 değilim" deyip köşeye çekilip bekliyorlar. Sipahi tek çekirdekli çalışıyor, sadece hart 0 devam ediyor.

**Adım 2 — "Çalışma masam nerede?"**

```asm
la      sp, __stack_top      # Stack'in tepesini sp register'ına yükle
csrw    mscratch, sp         # Aynı adresi mscratch'a da yaz (trap handler için)
```

Stack'i hatırla — tabak yığını. İşlemci hesap yaparken ara sonuçları buraya koyacak. `__stack_top` linker script'ten gelen bir adres — bellekte kernel stack'in nerede bittiğini gösteriyor. sp register'ı artık bu adresi gösteriyor.

mscratch'a da aynı adresi yazıyor — bu U-9 sprint'inde eklendi. Neden? Birazdan trap handler'da göreceksin. Şimdilik "yedek anahtar" gibi düşün.

**Adım 3 — "Masayı temizle"**

```asm
la      a0, __bss_start      # BSS bölümünün başlangıcı
la      a1, __clear_end      # Temizlenecek alanın sonu
.bss_loop:
    bgeu    a0, a1, .bss_done    # Bitirdik mi?
    sd      zero, (a0)           # Bu adrese 0 yaz
    addi    a0, a0, 8            # 8 byte ilerle
    j       .bss_loop            # Tekrarla
.bss_done:
```

BSS bölümü "başlangıç değeri verilmemiş global değişkenler" bölümü. İçinde çöp veri var — bellekte daha önce ne varsa o kalmış. Bu döngü her şeyi sıfırlıyor. `__bss_start`'tan `__clear_end`'e kadar her 8 byte'a 0 yazıyor.

Neden `__clear_end` ve `__bss_end` değil? Çünkü U-5 sprint'inde task stack'leri ve WASM arena'yı da BSS'in sonuna koyduk. Hepsinin sıfırlanması lazım.

**Adım 4 — "Rust'a geç"**

```asm
call    rust_main
```

Bu satır assembly'nin bittiği, Rust'ın başladığı yer. `rust_main` fonksiyonu `src/main.rs`'de tanımlı. Artık Rust dünyasındayız.

### 1.2 — Rust Uyanıyor: rust_main()

`src/main.rs` içindeki `rust_main()` fonksiyonu çalışıyor. Bu fonksiyon Sipahi'nin orkestra şefi — her şeyi doğru sırada başlatıyor.

```
rust_main()
├── Banner yazdır ("Sipahi Microkernel v0.1.0")
├── boot::init()        ← Bütün altyapıyı kur
├── tests::run_all()    ← Testleri çalıştır, fail varsa dur
└── boot::start()       ← İlk task'ı başlat, geri dönüş yok
```

### 1.3 — Altyapı Kurulumu: boot::init()

`src/boot.rs` içindeki `init()` fonksiyonu çağrılıyor. Bu fonksiyon sırayla şunları yapıyor:

**1. Trap vektörünü ayarla**

```rust
arch::csr::write_mtvec(arch::trap_entry as usize);
```

İşlemciye "bir interrupt veya hata olursa şu adrese git" diyor. `trap_entry` = trap.S dosyasındaki fonksiyon. Artık herhangi bir kesme olursa işlemci nereye gideceğini biliyor.

Bu neden İLK yapılıyor? Çünkü bundan sonraki adımlarda bir hata olursa (PMP yanlış ayarlanırsa, bellek erişim hatası olursa) işlemci trap handler'a gidebilmeli. Trap vektörü ayarlanmamışsa hata olduğunda işlemci ne yapacağını bilemez ve çöker.

**2. PMP'yi kur (bellek koruması)**

```rust
kernel::memory::init_pmp();
```

PMP = Physical Memory Protection. Bu fonksiyon bellekteki bölgeleri çizgilerle ayırıyor ve her bölgeye izin kuralları koyuyor:

```
Bölge 0-1: Sipahi'nin kodu    → Oku + Çalıştır (yaz yasak!)
Bölge 2-3: Sabit veriler      → Sadece oku
Bölge 4-5: Değişken veriler   → Oku + Yaz (çalıştırma yasak!)
Bölge 6-7: UART cihazı        → Oku + Yaz
Bölge 8:   Task stack'i       → Her task için ayrı ayarlanacak
Bölge 9-15: BOŞ               → Her şey yasak
```

Her bölge L-bit (Lock) ile kilitleniyor. L-bit koyulduğunda o bölgenin ayarları **bir daha değiştirilemez** — işlemciyi yeniden başlatana kadar. Bu donanım kilidi, yazılımla açılamaz.

Ayrıca bu fonksiyon "shadow register" denen kopyalar tutuyor. Her tick'te gerçek PMP register'ları ile shadow kopyalar karşılaştırılacak — eğer biri değişmişse kozmik ışın veya saldırı var demektir.

**3. Blackbox'ı başlat**

```rust
ipc::blackbox::init();
```

Kara kutuyu hazırlıyor. 8KB'lık dairesel bir bellek alanı — her güvenlik olayı buraya yazılacak. Güç kesilse bile veri korunacak (pilli bellek).

**4. Capability anahtarını yükle**

```rust
#[cfg(feature = "test-keys")]
{
    let mac_key = [0x5Au8; 32];
    broker::provision_key(&mac_key);
}
```

Capability sistemi BLAKE3 adlı bir algoritma ile token'ları imzalıyor. İmzalamak için gizli bir anahtar lazım. Test modunda bu anahtar sabit bir değer (0x5A tekrarı). Production'da HSM veya OTP fuse'dan gelecek.

`#[cfg(feature = "test-keys")]` ne demek? "Bu kod sadece test-keys özelliği açıksa derlensin." Production build'de bu blok tamamen yok — derleyici onu görmezden geliyor.

**5. Secure boot kontrolü**

```rust
#[cfg(feature = "test-keys")]
{
    if !secure_boot_check(...) {
        loop { unsafe { core::arch::asm!("wfi"); } }  // Dur, ilerleme
    }
}
```

Ed25519 dijital imza ile kernel'ın bütünlüğünü kontrol ediyor. İmza tutmazsa kernel durur — hiç task başlatmaz.

**6. Task'ları oluştur**

```rust
scheduler::create_task(task_a_config);
scheduler::create_task(task_b_config);
```

Her task için bir yapı oluşturuluyor: öncelik, bütçe, periyot, DAL seviyesi, stack adresi. Task'lar henüz çalışmıyor — sadece tanımlandılar.

### 1.4 — Sınav: tests::run_all()

Task'lar başlamadan önce kernel kendini test ediyor. Bu Power-On Self Test (POST) — uçaklarda da aynısı var, motor çalıştırılmadan önce tüm sistemler kontrol edilir.

POST şunları kontrol ediyor:
- CRC32 motoru çalışıyor mu? (bilinen girdi ile bilinen çıktıyı karşılaştır)
- PMP shadow register'lar tutarlı mı?
- Policy engine doğru karar veriyor mu? (PMP hatası → Shutdown mu dönüyor?)
- mstatus CSR erişilebilir mi?
- mtvec ayarlanmış mı? (0 değilse OK)
- BLAKE3 deterministik mi? (aynı girdi iki kez → aynı çıktı mı?)
- Ed25519 imza doğrulama çalışıyor mu?

Sonra integration testler çalışıyor: PMP NAPOT, policy engine, capability broker, IPC, WASM sandbox, blackbox, fault injection.

Eğer herhangi bir test başarısız olursa: `TEST_FAIL_COUNT > 0` → kernel durur. `wfi` (Wait For Interrupt) döngüsüne girer, hiç task başlatmaz. Boot HALTED.

Eğer tüm testler geçerse: `[TEST] ★★★ ALL TESTS PASSED ★★★`

### 1.5 — İlk Kalp Atışı: boot::start()

```rust
csr::enable_timer_interrupt();    // Timer kesmesini aç
clint::init_timer();              // İlk alarm kur (10ms sonra)
scheduler::start_first_task();    // İlk task'a atla — geri dönüş YOK
```

`start_first_task()` şunu yapıyor:

1. mscratch'a kernel stack adresini yaz (trap handler kullanacak)
2. mepc'ye ilk task'ın başlangıç adresini yaz
3. mstatus'u U-mode'a ayarla (yetki düşür)
4. sp'yi task'ın stack'ine ayarla
5. `mret` — M-mode'dan U-mode'a geç, task çalışmaya başlar

Bu noktadan itibaren kernel geri plana çekilir. Task çalışıyor. 10ms sonra timer interrupt gelecek ve Sipahi'nin kalbi atacak.

### 1.6 — Kalp Atışı: Timer Interrupt → Schedule

10 milisaniye geçti. CLINT cihazı timer interrupt gönderiyor. İşlemci ne yapıyorsa bırakıyor ve `trap_entry`'e atlıyor.

**trap.S — Kapı açılıyor**

```asm
csrrw   sp, mscratch, sp      # Atomic swap: sp ↔ mscratch
```

Bu tek satır çok şey yapıyor. İşlemci U-mode'da task'ın stack'ini kullanıyordu. Bu satır sp'yi kernel stack'le değiştiriyor, eski sp'yi (task'ın stack'i) mscratch'a koyuyor. Artık trap handler kernel stack'te çalışıyor — task'ın stack'ine dokunmuyor.

```asm
beqz    sp, .nested_fault      # sp = 0 ise → nested trap, dur
addi    sp, sp, -272           # 272 byte'lık çerçeve oluştur
```

Sonra 16 register'ı bu çerçeveye kaydediyor (ra, t0-t6, a0-a7), mscratch'tan user_sp'yi okuyup çerçevenin 256. byte'ına koyuyor, mscratch'ı sıfırlıyor (nested trap detection için).

mcause ve mepc CSR'larını okuyor — "neden geldim" ve "nerede kaldım" bilgisi. Sonra Rust'taki `trap_handler` fonksiyonunu çağırıyor.

**trap.rs — Beyin çalışıyor**

Rust'taki trap_handler mcause'a bakıyor:

```
mcause = 0x8000000000000007 → Timer interrupt (en üst bit 1 = interrupt, alt 7 = timer)
```

Timer interrupt! Scheduler'ı çağırıyor.

**scheduler — Karar verme zamanı**

`schedule()` fonksiyonu 4 fazda çalışıyor:

**Faz 1 — Saatleri ilerlet:** Her task'ın periyot sayacını 1 artır. Periyot dolmuşsa task'ı Ready yap ve bütçesini yenile.

**Faz 1.5 — Nöbetçiyi kontrol et:** Watchdog sayacını artır. IPC gönderme sayacını sıfırla. Watchdog limiti aşılmışsa → PolicyEvent tetikle.

**Faz 2 — Bütçeyi düş:** Şu an çalışan task'ın bütçesinden 1 düş. Bütçe bittiyse → Suspended yap.

**Faz 3 — En iyisini seç:** Tüm Ready task'lar arasında en yüksek öncelikliyi bul. Bu `select_highest_priority()` fonksiyonu — Kani ile kanıtlanmış pure fonksiyon.

**Faz 4 — Geçiş yap:** Yeni task farklıysa PMP Entry 8'i yeni task'ın stack'ine ayarla, `switch_context` çağır.

**context.S — Kapılar değişiyor**

```asm
la      t0, __stack_top
ld      t1, -16(t0)           # Eski task'ın user_sp'sini trap frame'den oku
sd      t1, 8(a0)             # TaskContext'e kaydet
```

Eski task'ın callee-saved register'larını (s0-s11) ve mepc/mstatus'u kaydediyor. Yeni task'ın register'larını yüklüyor. Yeni task'ın user_sp'sini trap frame'e yazıyor.

**trap.S — Kapı kapanıyor**

Trap handler bitiyor. Register'lar geri yükleniyor. mscratch restore ediliyor. `mret` — M-mode'dan U-mode'a geri dön. Yeni task çalışmaya başlıyor.

Bu döngü her 10ms'de tekrarlanıyor. Sipahi'nin kalp atışı bu.

---

## BÖLÜM 2: RUST'I SİPAHİ'DE NASIL KULLANIYORUZ

Bu bölüm generic Rust kursu DEĞİL. Sipahi'de karşılaştığın her Rust pattern'ini anlatıyor.

### 2.1 — SingleHartCell: Sipahi'nin Kasası

Sipahi'de `static mut` (global değiştirilebilir değişken) yasak. Neden? Rust bunu güvensiz buluyor — iki thread aynı anda yazabilir. Ama Sipahi tek çekirdekli, thread yok. Yine de Rust şikayet ediyor.

Çözüm: SingleHartCell. Bir kasa gibi düşün — içine bir değer koyuyorsun, almak için özel fonksiyon çağırıyorsun.

```rust
static TASK_COUNT: SingleHartCell<usize> = SingleHartCell::new(0);

// Okumak için:
let count = unsafe { *TASK_COUNT.get() };

// Yazmak için:
unsafe { *TASK_COUNT.get_mut() = 3; }
```

Neden unsafe? Rust diyor ki "ben garanti edemem, tek çekirdek olduğunu sen biliyorsun, sorumluluk sende." Ve her unsafe bloğun yanına SAFETY yorumu yazıyoruz — "neden bu güvenli?" açıklaması.

### 2.2 — #[cfg] ile Koşullu Derleme

Sipahi'de bazı kodlar sadece belirli durumlarda derleniyor:

```rust
#[cfg(feature = "test-keys")]        // Sadece test modunda
#[cfg(feature = "debug-boot")]       // Sadece debug modunda
#[cfg(feature = "trace")]            // Sadece trace açıkken
#[cfg(not(kani))]                    // Kani çalışırken DEĞİL
```

Bu "koşullu derleme" — derleyici bu satırları sadece koşul doğruysa derliyor. Yanlışsa o kod binary'de bile yok. Production build'de trace kodları hiç derlenmez → sıfır overhead.

### 2.3 — #[repr(C)] ile Bellek Düzeni Kontrolü

Rust normalde struct'ın field'larını istediği gibi sıralar ve araya boşluk koyabilir. Ama donanımla konuşurken bellek düzeninin kesin olması lazım.

```rust
#[repr(C)]
pub struct BlackboxRecord {
    pub magic:   [u8; 4],     // offset 0
    pub version: u16,          // offset 4
    pub _pad:    [u8; 2],      // offset 6 (explicit boşluk)
    pub seq:     u32,          // offset 8
    // ...
}
```

`#[repr(C)]` = "bu struct'ı C dili gibi sırala, boşluk bırakma kuralları C'ninki gibi olsun." Böylece struct'ın bellekte tam olarak nerede ne olduğunu biliyoruz.

### 2.4 — core::hint::black_box — Derleyiciyi Kandırmak

LLVM derleyicisi çok akıllı — gereksiz bulduğu kodu siliyor. Ama bazen "gereksiz" gördüğü şey aslında güvenlik için gerekli.

```rust
let r = decide_action(event, rc, dal);
core::hint::black_box(r)    // "bu değeri kullandım, silme!"
```

`black_box` derleyiciye "bu değer önemli, optimize etme" diyor. Lockstep'te iki kez hesaplama yapıyoruz — derleyici "ikisi aynı sonuç, birini siliyim" diyebilir. black_box bunu engelliyor.

### 2.5 — #[inline(never)] — Fonksiyonu Ayrı Tut

Derleyici küçük fonksiyonları çağıran yere kopyalar (inline). Ama lockstep'te iki ayrı fonksiyon çağrısı olmasını istiyoruz.

```rust
#[inline(never)]
fn decide_action_fenced(event: u8, rc: u8, dal: u8) -> FailureMode {
    let r = decide_action(event, rc, dal);
    core::hint::black_box(r)
}
```

`#[inline(never)]` = "bu fonksiyonu kopyalama, ayrı tut." Böylece binary'de iki ayrı `jalr` (fonksiyon çağrısı) instruction'ı oluyor — objdump ile doğruladık.

### 2.6 — const fn — Derleme Zamanında Hesaplama

```rust
pub const fn crc32(data: &[u8]) -> u32 {
    // ... hesaplama ...
}
```

`const fn` = bu fonksiyon derleme zamanında da çalışabilir. Sipahi'de CRC32 hesaplaması hem derleme zamanında (compile-time assert'lerde) hem de çalışma zamanında kullanılıyor.

### 2.7 — Saturating Arithmetic — Taşma Yok

```rust
counter = counter.saturating_add(1);   // MAX'taysa MAX kalır, taşmaz
budget = budget.saturating_sub(1);     // 0'daysa 0 kalır, eksi olmaz
```

Normal toplama taşabilir: 255 + 1 = 0 (u8'de). Bu bir bug olur. `saturating_add` taşma yerine MAX değerde kalır. Sipahi'nin "no panic" doktrininin parçası.

### 2.8 — Volatile — Donanımla Konuşmak

```rust
unsafe {
    core::ptr::read_volatile(0x10000000 as *const u8)   // UART'tan oku
    core::ptr::write_volatile(0x10000000 as *mut u8, b'A')  // UART'a yaz
}
```

Normal bellek okuması/yazması derleyici tarafından optimize edilebilir — "bu değeri zaten okudum, tekrar okumama gerek yok" diyebilir. Ama donanım register'ları her okunduğunda farklı değer döndürebilir. `volatile` = "her seferinde gerçekten oku/yaz, optimize etme."

---

## BÖLÜM 3: SİPAHİ'NİN 8 GÜVENLİK KATMANI

Sipahi'nin güvenlik felsefesi: "Güvenme, doğrula — her katmanda, her tick'te, her cycle'da."

Her katman diğer katmanlardan bağımsız çalışıyor. Birisi kırılsa bile diğerleri hâlâ koruyor. Tüm katmanları aynı anda kırmak için 8 farklı mekanizmayı simultaneously bypass etmen lazım.

### Katman 1 — Demir Duvar: PMP

PMP donanım seviyesinde bellek koruması. İşlemcinin kendi devresinde — yazılımla atlanamaz.

Nasıl çalışıyor: Bellekteki her bölgeye "kim ne yapabilir" kuralları koyuluyor. Bir task başka bir task'ın bölgesine erişmeye kalkarsa işlemci StoreAccessFault veya LoadAccessFault fırlatıyor — trap handler yakalıyor, task izole ediliyor.

L-bit (Lock) ile kurallar kilitleniyor. Kilitlendikten sonra işlemci resetlenmeden açılamaz. M-mode bile (en yetkili mod) kilitli bölgeleri değiştiremez.

### Katman 2 — Dijital Anahtar: Capability Token

Her task'ın ne yapabileceğini belirleyen dijital anahtar. Otel kartı gibi — üzerinde "hangi odaya, ne işe, ne zamana kadar" yazıyor.

Token'ın bütünlüğü BLAKE3 keyed hash ile korunuyor. Kernel gizli bir anahtarla hash hesaplıyor, token'a koyuyor. Task token'ı değiştirirse hash tutmaz → DENY.

Cache sistemi var — son 4 doğrulanmış token'ı saklıyor. Cache hit constant-time (sabit süreli) — branch yok, timing attack imkansız.

### Katman 3 — İkiz Hesap: Policy Lockstep

Kozmik ışın bir bit'i çevirirse ne olur? Yanlış karar. Çözüm: her kritik kararı iki kez hesapla, sonuçları karşılaştır.

```
action1 = decide_action(event, count, dal)
action2 = decide_action(event, count, dal)
if action1 != action2 → Shutdown (bir şeyler çok yanlış)
```

`#[inline(never)]` + `black_box` ile derleyicinin iki hesaplamayı birleştirmesi engelleniyor. Binary'de iki ayrı `jalr` instruction'ı var — objdump ile doğrulandı.

### Katman 4 — Gece Bekçisi: Windowed Watchdog

Her task belirli aralıklarla "ben hâlâ canlıyım" demeli. İki sınır var:

Üst sınır: 100 tick (1 saniye) içinde yield etmezse → task donmuş, policy tetikle.
Alt sınır: 3 tick'ten önce yield ederse → task çok hızlı, kontrol akışı bozuk.

Normal watchdog sadece üst sınır kontrol eder. Sipahi alt sınır da kontrol ediyor — ISO 26262 uyumlu.

### Katman 5 — Güvenlik Kamerası: Blackbox Flight Recorder

Her güvenlik olayı 8KB dairesel bir belleğe kaydediliyor. Her kayıtta:
- Ne zaman oldu (tick)
- Ne oldu (event türü)
- Kim yaptı (task ID)
- CRC32 (kaydın bozulmadığını doğrulama)

Güç kesilse bile veri korunuyor. Hiçbir task blackbox'ı silemez — PMP korumalı.

### Katman 6 — Kimlik Kontrolü: Secure Boot

Boot sırasında kernel'ın bütünlüğü Ed25519 dijital imza ile kontrol ediliyor. İmza tutmazsa kernel hiç başlamıyor.

### Katman 7 — Kum Havuzu: WASM Sandbox

DAL-C/D task'ları WASM sandbox'ta çalışıyor. Sandbox'ın 3 koruması:
- Fuel metering: her instruction yakıt harcar, yakıt bitince durur
- Float rejection: float opcode'lar yükleme zamanında reddediliyor
- Linear memory: task sadece kendi bellek alanına erişir

Task çökse bile sandbox dışına çıkamaz — flight control etkilenmez.

### Katman 8 — Adres Temizliği: Kernel Pointer Sanitization

Syscall sonucu task'a dönerken kernel adresi sızdırmamak için kontrol yapılıyor. Dönüş değeri kernel bellek aralığındaysa `E_INTERNAL` dönüyor.

---

## BÖLÜM 4: SİPAHİ'NİN HER MODÜLÜ

### src/arch/ — Donanımla Konuşan Katman

Bu klasör donanıma en yakın kod. Assembly dosyaları ve donanım register'larıyla konuşan Rust kodu burada.

`boot.S` — İlk açılış kodu. Stack kurar, BSS sıfırlar, Rust'a geçer.
`trap.S` — Interrupt geldiğinde register'ları kaydeder/yükler. mscratch swap burada.
`context.S` — Task'lar arası geçişte register'ları kaydeder/yükler.
`uart.rs` — UART seri port. Ekrana karakter yazmak için.
`pmp.rs` — PMP register'larını programlama. `write_per_task_napot()` wrapper'ı burada.
`csr.rs` — CSR register'larını okuma/yazma fonksiyonları.
`clint.rs` — Timer yönetimi. `schedule_next_tick()` burada.
`trap.rs` — Trap handler'ın Rust tarafı. mcause'a bakıp yönlendirme yapıyor.

### src/hal/ — Donanım Soyutlama

`device.rs` — HalDevice trait. Donanım cihazları için ortak arayüz.
`iopmp.rs` — I/O cihaz koruması (şimdilik devre dışı).
`secure_boot.rs` — Ed25519 imza doğrulama.
`key.rs` — Kriptografik anahtar yönetimi.

### src/kernel/ — Çekirdek Mekanizmalar

`scheduler/mod.rs` — Sipahi'nin beyni. `schedule()` fonksiyonu, task yönetimi, context switch çağrısı, budget enforcement, watchdog, policy action uygulama — hepsi burada.

`capability/mod.rs` — Token yapısı, `ct_eq_16` constant-time karşılaştırma.
`capability/cache.rs` — 4 slot'luk token önbelleği. Branch-free lookup.
`capability/broker.rs` — Token oluşturma, imzalama, doğrulama. `validate_full()` burada.

`policy/mod.rs` — 5+1 mod failure policy engine. `decide_action()` pure fonksiyon (Kani ile kanıtlanmış). `apply_policy()` lockstep wrapper.

`syscall/dispatch.rs` — 5 syscall handler. Task'ın `ecall` ile istediği işlemi yönlendiriyor.

`memory/mod.rs` — PMP kurulumu ve shadow register doğrulama. `verify_pmp_integrity()` her tick'te çağrılıyor.

### src/ipc/ — İletişim

`mod.rs` — SPSC (Single Producer, Single Consumer) lock-free ring buffer. 8 kanal, 16 slot, 64 byte mesaj.
`blackbox.rs` — Flight recorder. 8KB dairesel buffer, CRC32 korumalı.

### src/sandbox/ — WASM Kum Havuzu

`mod.rs` — Wasmi interpreter entegrasyonu. Modül yükleme, float tarama, fuel metering.
`allocator.rs` — Bump allocator. WASM modülü için bellek ayırma. Epoch reset ile sıfırlama.

### src/common/ — Paylaşılan Altyapı

`config.rs` — Tüm sabitler. WCET hedefleri, bellek adresleri, task limitleri.
`types.rs` — Ortak tip tanımları.
`error.rs` — Hata kodları.
`crypto/` — BLAKE3 ve CRC32 implementasyonları.
`fmt.rs` — `print_u32`, `print_hex` gibi formatlama fonksiyonları (core::fmt kullanmadan).
`sync.rs` — SingleHartCell tanımı.

### src/verify.rs — Formal Verification

67 Kani harness'ın ana dosyası. Diğer modüllerde de harness'lar var — toplam 188.

---

## BÖLÜM 5: FORMAL VERIFICATION — KANITLAMA NEDİR

### Testle Kanıt Arasındaki Fark

Test: "Bu fonksiyona 1, 2, 3, 100, 255 verdim, hepsi doğru çıktı."
Kanıt: "Bu fonksiyona VERİLEBİLECEK HER DEĞER için doğru çıktığını matematiksel olarak kanıtladım."

Test 5 durumu kontrol ediyor. Kanıt 2^32 = 4 milyar durumu kontrol ediyor (u32 için). 6. durumda hata olabilir — test yakalamaz, kanıt yakalar.

### Kani Nasıl Çalışıyor

Kani Rust kodunu matematiksel formüllere çeviriyor. Sonra CBMC adlı bir araç tüm olası girdi kombinasyonlarını tarıyor.

```rust
#[kani::proof]
fn nonce_replay_is_rejected() {
    let token_nonce: u32 = kani::any();     // herhangi bir değer
    let last_nonce: u32 = kani::any();       // herhangi bir değer
    kani::assume(token_nonce <= last_nonce);  // koşul: eski veya eşit
    assert!(!is_nonce_valid(token_nonce, last_nonce));  // reddedilmeli
}
```

`kani::any()` = "herhangi bir değer." Kani 0'dan 4 milyara kadar her u32 kombinasyonunu deniyor. Eğer assert başarısız olabilecek bir kombinasyon varsa → FAIL rapor ediyor. Yoksa → PASS.

### 188 Harness Ne Anlama Geliyor

Sipahi'de 188 Kani harness var. Üç kategoride:

**88 sembolik proof** — gerçek formal verification. `kani::any()` ile geniş girdi alanı taranıyor. Scheduler seçimi, policy kararları, capability doğrulama, PMP konfigürasyonu.

**65 concrete proof** — belirli değerlerle test. CRC32 bilinen vektör, IPC roundtrip, cache insert/lookup.

**35 compile-time assertion** — derleme zamanında zaten bilinen şeyler. Enum boyutları, sabit değerler. Kani olmadan da doğru.

### TLA+ Ne

TLA+ sistem seviyesinde davranış modelleme dili. Kani tek fonksiyonu kanıtlar — TLA+ tüm sistemin davranışını modelliyor.

Sipahi'de 7 TLA+ spec var, hepsi verified:
- SipahiIPC — mesaj gönderme/alma protokolü
- SipahiWatchdog — watchdog tetikleme mantığı
- SipahiCapability — token doğrulama state machine
- SipahiScheduler — task seçimi ve bütçe yönetimi
- SipahiDegradeRecover — degrade moda giriş/çıkış
- SipahiBudgetFairness — DAL-A task'ların açlığa uğramaması
- SipahiPolicy — policy escalation ve livelock freedom

---

## BÖLÜM 6: BUG HİKAYELERİ

Bu bölüm Sipahi'de bulunan ve düzeltilen bug'ları anlatıyor. Her bug bir ders.

### Bug 1 — PMP Entry 5 Herkesin Evine Açık (Sprint U-5'te düzeltildi)

İlk analiz buldu: PMP Entry 5 `.data` bölgesini koruyor ama `.task_stacks` de `.data`'dan sonra, aynı bölgede. Yani her task her task'ın stack'ine erişebiliyordu.

Düzeltme: Linker script'te `.task_stacks` ve `.wasm_arena` bölümleri Entry 5 sınırının DIŞINA çıkarıldı. `__pmp_data_end` sınırı eklendi. Task stack'ler artık PMP match yok → U-mode default deny.

Ders: PMP bölge sınırları linker script ile belirleniyor. Linker script değişirse güvenlik kırılabilir.

### Bug 2 — Capability Cache Başkasının Anahtarını Kabul Ediyor (Sprint U-4'te düzeltildi)

Cache'te token doğrulanırken task_id kontrol edilmiyordu. Task A'nın token'ı cache'e giriyordu, Task B aynı resource için sorduğunda cache hit dönüyordu — yetki kontrolü bypass.

Düzeltme: Cache entry'ye `owner_task_id` eklendi. Lookup'ta `entry.owner_task_id == caller_task_id` kontrolü eklendi. Branch-free, constant-time.

Ders: Cache optimizasyonu güvenlik kontrolünü atlamamalı.

### Bug 3 — Lockstep Derleyici Tarafından Silinebilir (Sprint U-4'te düzeltildi)

Policy kararını iki kez hesaplıyorduk ama LLVM "iki çağrı aynı sonuç, birini siliyim" diyebilir (CSE — Common Subexpression Elimination).

Düzeltme: `#[inline(never)]` + `core::hint::black_box`. Binary'de iki ayrı `jalr` olduğu objdump ile doğrulandı.

Ders: Güvenlik mekanizmaları derleyici optimizasyonlarına karşı korunmalı.

### Bug 4 — Trap Handler Yanlış Stack'te (Sprint U-9'da düzeltildi)

En büyük bug. Trap handler M-mode'a geçtiğinde sp hâlâ user task'ın sp'siydi. Kötü niyetli task ecall'dan önce sp'yi başka task'ın stack adresine set edebilir → trap handler M-mode'da o stack'e 256 byte yazar → cross-task memory corruption.

Düzeltme: `csrrw sp, mscratch, sp` — trap entry'de atomic swap. Trap handler artık kernel stack'te çalışıyor. Nested trap detection eklendi.

Ders: M-mode PMP'den muaf. PMP izolasyonu sadece U-mode'u koruyor. M-mode'da çalışan trap handler farklı korumalara ihtiyaç duyuyor.

### Bug 5 — MAC Key Herkese Açık (Sprint U-9'da düzeltildi)

Capability MAC anahtarı `[0x5A; 32]` her build'de — production dahil — aynı. Açık kaynak projede herkes anahtarı görüyor.

Düzeltme: `#[cfg(feature = "test-keys")]` ile gate'lendi. Production build'de key yüklenmiyor → capability sistemi kapalı (key yoksa token doğrulama her zaman false).

Ders: Test verileri production'a sızmamalı.

---

## BÖLÜM 7: KOD OKUMA PRATİĞİ

### Pratik 1 — trap.S entry (ilk 7 satır)

```asm
trap_entry:
    csrrw   sp, mscratch, sp
    beqz    sp, .nested_fault
    addi    sp, sp, -272
    sd      t0, 8(sp)
    csrr    t0, mscratch
    sd      t0, 256(sp)
    csrw    mscratch, zero
```

Satır 1: sp ile mscratch'ı değiştir. sp artık kernel stack, mscratch artık user sp.
Satır 2: sp = 0 ise nested trap (trap içinde trap). Park et.
Satır 3: 272 byte çerçeve oluştur (sp'yi 272 aşağı it).
Satır 4: t0'ı çerçevenin 8. byte'ına kaydet (henüz bozulmamış user değeri).
Satır 5: mscratch'tan user sp'yi oku (t0'a koy).
Satır 6: user sp'yi çerçevenin 256. byte'ına kaydet.
Satır 7: mscratch'ı sıfırla — bir sonraki trap gelirse sp = 0 olacak → nested trap yakalanacak.

### Pratik 2 — decide_action (policy kararı)

```rust
pub const fn decide_action(event: u8, restart_count: u8, dal: u8) -> FailureMode {
    match event {
        0 => { /* BudgetExhausted */
            if restart_count == 0 { FailureMode::Restart }
            else { FailureMode::Degrade }
        }
        // ...
        5 => FailureMode::Shutdown,  // PmpIntegrityFail → her zaman Shutdown
        // ...
    }
}
```

Bu fonksiyon `const fn` — derleme zamanında da çalışabilir. Pure fonksiyon — side effect yok, sadece girdi alıp çıktı veriyor. Kani ile kanıtlanmış — her olası event/restart_count/dal kombinasyonu için doğru sonuç döndüğü garanti.

### Pratik 3 — cache lookup (constant-time)

```rust
let now = get_tick();
let mut found: u8 = 0;
while i < CACHE_SLOTS {
    let e = &self.entries[i];
    let is_infinite = (e.expires == 0) as u8;
    let not_expired = (now <= e.expires as u64) as u8;
    let expiry_ok = is_infinite | not_expired;
    let hit = (e.valid as u8) & (e.owner_task_id == task_id) as u8
            & (e.resource == resource) as u8
            & (e.action == action) as u8
            & expiry_ok;
    found |= hit;
    i += 1;
}
found != 0
```

Bu kod constant-time — hangi slot'ta bulunursa bulunsun, hep 4 slot taranıyor. `if` yok, early return yok. Tüm karşılaştırmalar `as u8` ile 0/1'e dönüştürülüyor, `&` ile birleştiriliyor. Timing side-channel attack imkansız.

---

## SİPAHİ SÖZLÜĞÜ

**bare metal** — işletim sistemi olmadan doğrudan donanım üzerinde çalışma
**boot** — bilgisayarın açılıp ilk kodun çalışması
**BSS** — başlangıç değeri verilmemiş global değişkenler bölümü
**budget** — bir task'a ayrılan CPU cycle sayısı
**capability** — bir task'ın ne yapabileceğini belirleyen dijital anahtar
**cfg** — koşullu derleme (conditional compilation)
**CLINT** — RISC-V timer donanımı
**const fn** — derleme zamanında da çalışabilen fonksiyon
**context switch** — bir task'tan diğerine geçiş
**CRC32** — veri bütünlüğü kontrol algoritması
**CSR** — işlemcinin kontrol register'ları
**DAL** — Design Assurance Level (güvenlik seviyesi: A en yüksek)
**determinism** — aynı girdi her zaman aynı çıktı, aynı sürede
**ecall** — U-mode'dan kernel'a istek gönderme komutu
**formal verification** — matematiksel olarak hata olmadığını kanıtlama
**fuel metering** — WASM'da her komutun yakıt harcaması
**hart** — RISC-V'de işlemci çekirdeği
**IPC** — task'lar arası iletişim
**Kani** — Rust için formal verification aracı
**L-bit** — PMP kilitleme biti (reset'e kadar açılamaz)
**lockstep** — aynı hesabı iki kez yapıp sonuçları karşılaştırma
**M-mode** — en yetkili çalışma modu (kernel burada)
**mcause** — interrupt/exception nedenini tutan CSR
**mepc** — interrupt anında işlemcinin nerede olduğunu tutan CSR
**mret** — M-mode'dan geri dönüş (U-mode'a geçiş)
**mscratch** — trap handler için yedek register
**NAPOT** — Naturally Aligned Power-Of-Two (PMP bölge boyutu)
**no_std** — standart kütüphane kullanmadan çalışma
**PMP** — Physical Memory Protection (donanım bellek koruması)
**policy engine** — hata olduğunda ne yapılacağına karar veren sistem
**POST** — Power-On Self Test (açılış sınavı)
**saturating** — taşma yerine MAX/MIN değerde kalma
**scheduler** — hangi task'ın çalışacağına karar veren sistem
**shadow register** — donanım register'larının yazılım kopyası
**SingleHartCell** — tek çekirdek için güvenli global değişken sarmalayıcı
**SPSC** — Single Producer Single Consumer (tek yazar, tek okuyucu)
**syscall** — task'ın kernel'dan hizmet istemesi
**task** — bir iş parçası, bir program
**TLA+** — sistem davranışı modelleme dili
**trap** — interrupt veya exception sonucu işlemcinin yaptığı atlama
**U-mode** — en az yetkili çalışma modu (task'lar burada)
**unsafe** — Rust'ın güvenlik garantilerini geçici olarak askıya alma
**volatile** — derleyici optimizasyonunu engelleyen bellek erişimi
**WASM** — WebAssembly, taşınabilir sandbox formatı
**watchdog** — task'ların canlılığını kontrol eden mekanizma
**WCET** — Worst Case Execution Time (en kötü durum çalışma süresi)
**W^X** — Write XOR Execute (bir bölge ya yazılabilir ya çalıştırılabilir, ikisi birden değil)
