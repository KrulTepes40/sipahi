# Sipahi Microkernel — Teknik Özellik Dokümanı

**Versiyon:** v1.5 · **Mimari:** RISC-V RV64IMAC · **Dil:** Rust no_std  
**Toplam:** ~8,158 Rust + ~265 ASM satır · 42 kaynak dosya · 188 Kani harness · 7/7 TLA+ verified  
**Felsefe:** Determinizm korunurken maksimum hız. Sıfır heap, sıfır panic, sıfır float.

---

## 1. Temel Tasarım Kararları

### 1.1 Neden RISC-V?

Sipahi, RISC-V RV64IMAC ISA'sını hedefler. ARM veya x86 değil çünkü RISC-V açık kaynak ISA — lisans ücreti yok, özelleştirilebilir. Askeri ve havacılık sistemlerinde yabancı ISA bağımlılığı stratejik risk oluşturur. RISC-V ile tüm donanım zinciri yerel kontrol altında tutulabilir. RV64IMAC profili: 64-bit tamsayı (I), çarpma/bölme (M), atomik (A), sıkıştırılmış komutlar (C). Float extension (F/D) kasıtlı olarak dahil edilmedi — Sipahi'de float yasak, tüm hesaplama Q32.32 fixed-point ile yapılır.

### 1.2 Neden Rust?

C yerine Rust seçildi çünkü Rust'ın ownership sistemi derleme zamanında bellek güvenliği garanti eder. Safety-critical C kodunda bellek hatalarının %70'i use-after-free, buffer overflow ve dangling pointer kaynaklıdır. Rust bunları dil seviyesinde imkansız kılar. `no_std` + `no_alloc` (kernel seviyesi) ile bare-metal ortamda çalışır. `alloc` crate sadece WASM sandbox (Wasmi) için kullanılır, kernel kodu heap tahsisatı yapmaz.

### 1.3 Neden Float Yasak?

IEEE 754 floating-point aritmetiği non-deterministic olabilir — farklı donanımlarda aynı hesaplama farklı sonuç üretebilir (rounding mode, denormalized number handling). Safety-critical'da bu kabul edilemez. Sipahi tüm hesaplamaları Q32.32 fixed-point (`i64`) ile yapar: ±2³¹ aralık, ~2.3×10⁻¹⁰ hassasiyet. WASM modüllerinde float opcode tespit edilirse modül reddedilir — `is_float_opcode()` ile 0x43-0xBF aralığı taranır.

### 1.4 Neden Microkernel?

Monolitik kernel yerine microkernel çünkü saldırı yüzeyi küçük. Kernel sadece scheduler, IPC, capability, policy ve trap handler içerir. WASM sandbox, blackbox, secure boot kernel dışında çalışır. Bir bileşen çökerse kernel ayakta kalır. DO-178C DAL-A sertifikasyonu için küçük, doğrulanabilir kernel şart — 188 Kani harness (88 symbolic proof + 100 concrete/compile-time) ve 7/7 TLA+ spec ile kritik invariant'lar (scheduler seçim doğruluğu, policy escalation, IPC bütünlüğü, bellek güvenliği) formal olarak kanıtlanmış.

---

## 2. Privilege Ayrımı — M-mode / U-mode

### 2.1 Mimari

Sipahi iki privilege seviyesi kullanır. Kernel M-mode'da (Machine mode) çalışır — tüm CSR'lara, PMP register'larına ve MMIO'ya erişim var. Task'lar U-mode'da (User mode) çalışır — CSR erişimi yok, PMP tarafından kısıtlanmış bellek erişimi.

**Neden S-mode (Supervisor) yok?** RISC-V S-mode MMU (virtual memory) gerektirir. Sipahi bare-metal microkernel — sayfa tablosu overhead'i ve TLB flush non-determinism istenmiyor. M/U ayrımı PMP ile fiziksel bellek koruması sağlar, MMU karmaşıklığı olmadan.

### 2.2 U-mode Geçiş Mekanizması

Task oluşturulurken `mstatus.MPP = 00` (U-mode) ve `mstatus.MPIE = 1` (interrupt enable) ayarlanır. `start_first_task()` fonksiyonu `csrw mepc, entry` + `csrw mstatus, val` + `mret` ile U-mode'a düşer. Context switch'te `task_trampoline` (assembly) kullanılır: `switch_context` → `ret` → `task_trampoline` → `mret` → U-mode. Bu trampoline compiler müdahalesi olmadan (prologue/epilogue yok) `mret` çalıştırır.

**Neden assembly trampoline?** Rust fonksiyonu olarak tanımlandığında compiler prologue ekler, LTO farklı davranır. Assembly'de sadece `mret` — tek instruction, sıfır overhead, sıfır belirsizlik.

### 2.3 mstatus.MPP Doğrulaması

Her U-mode ecall sonrası trap handler `mstatus.MPP` bitlerini kontrol eder. MPP ≠ 0 ise (yani task M-mode'a yükselmeye çalışıyorsa) fault injection saldırısı tespit edilmiş demektir — `PRIVILEGE ESCALATION DETECTED → SHUTDOWN`. Bu kontrol M-mode ecall'da (boot testleri) atlanır çünkü boot sırasında MPP=3 doğrudur.

---

## 3. Bellek Koruması — PMP

### 3.1 Bölge Düzeni

RISC-V PMP 16 entry destekler. Sipahi TOR (Top of Range) modunu kullanır çünkü bölge boyutları 2'nin kuvveti değil — NAPOT kullanılamaz. 8 entry kernel bölgeleri için kullanılır, 8 entry task bölgeleri için ayrılmıştır (pmpcfg2, Sprint U-3'te aktif olacak).

| Entry | Bölge | İzin | Açıklama |
|-------|-------|------|----------|
| 0-1 | .text | RX + Lock | Kernel kodu — yazma yasak (W^X) |
| 2-3 | .rodata | R + Lock | Salt okunur veri — yazma/çalıştırma yasak |
| 4-5 | .data+bss+kernel_stack | RW + Lock | Yazılabilir veri + kernel stack (__pmp_data_end sınırı) |
| 6-7 | UART MMIO | RW + Lock | Serial port erişimi |
| 8 | Task stack (NAPOT) | RW | Per-task, context switch'te reprogramlanır |
| 9-15 | Rezerve | — | Gelecek kullanım |

**L-bit (Lock) neden kullanılıyor?** L-bit set edildiğinde PMP kuralları M-mode dahil tüm privilege seviyelerinde zorlanır. Bu, kernel kodunun (.text) yanlışlıkla üzerine yazılmasını M-mode'da bile engeller. U-mode'da PMP eşleşmeyen adresler otomatik reddedilir (RISC-V spec §3.7.1).

### 3.2 PMP Shadow Register

Boot'ta PMP register değerleri `PMP_SHADOW` static'ine kaydedilir. Her scheduler tick'te `read_pmpcfg0()` ile gerçek register okunup shadow ile karşılaştırılır. Uyuşmazlık = fault injection saldırısı → `PmpViolation → SHUTDOWN`. Maliyet: 1 CSR read + 1 compare = O(1), ~5 cycle/tick.

**Neden shadow register?** Donanım fault injection (glitching, laser) ile PMP register'ları bozulabilir. Shadow karşılaştırması bu saldırıyı yazılım seviyesinde tespit eder.

### 3.3 Per-Task PMP (NAPOT)

Task stack'leri `.task_stacks` linker section'ına taşınarak PMP Entry 5 kapsamı dışına çıkarıldı. Bu sayede U-mode task'lar Entry 5 üzerinden tüm RAM'e erişemez — task stacks bölgesinde PMP match olmadığından U-mode implicit DENY (RISC-V spec §3.7.1). Her task kendi stack'ine sadece PMP entry 8 NAPOT ile erişir. NAPOT modu kullanılır — 8KB = 2^13 power-of-2, tek entry yeterli. Her context switch'te entry 8 yeni task'ın stack bölgesine programlanır. Config: R+W, X=0 (W^X), L=0 (kilitlenmez — switch'te değişir). WASM arena da `.wasm_arena` section'a taşındı — U-mode DENY, M-mode erişir (Wasmi interpreter M-mode'da çalışır).

NAPOT task stack koruması QEMU virt machine'de test edilmiştir. QEMU PMP granularity parametresi (G) platform bağımlıdır. CVA6 hedefinde G=0 (4-byte granularity) beklenir. Farklı donanımda G değeri NAPOT minimum bölge boyutunu etkileyebilir — FPGA doğrulaması sırasında kontrol edilmelidir.

---

## 4. Scheduler

### 4.1 Sabit Öncelikli Preemptive Scheduler

Sipahi sabit öncelikli (fixed-priority) preemptive scheduler kullanır. Round-robin veya EDF (Earliest Deadline First) yerine sabit öncelik çünkü WCET analizi daha kolay ve DO-178C sertifikasyonu için tercih edilir. Priority 0 = en yüksek (DAL-A), priority 15 = en düşük (DAL-D).

**select_highest_priority()** fonksiyonu O(N) linear scan yapar (N = MAX_TASKS = 8). Hash table veya priority queue kullanılmadı çünkü 8 task için overhead gereksiz ve worst case = best case = 8 iterasyon. Dallanmasız — her zaman tüm task'ları tarar, constant-time garantisi.

### 4.2 Budget Enforcement

Her task'a CPU bütçesi atanır: DAL-A %40, DAL-B %30, DAL-C %20, DAL-D %10. Budget `saturating_sub(CYCLES_PER_TICK)` ile azaltılır — overflow imkansız. Budget tükenince `BudgetExhausted` policy event tetiklenir.

**Neden saturating_sub?** `wrapping_sub` ile budget 0'ın altına düşüp büyük pozitif sayıya sarılabilir — task sonsuz bütçe kazanır. `saturating_sub` 0'da durur, güvenli.

### 4.3 Period-Based Task Model

Her task'ın period'u var (default 10 tick = 100ms). Period dolduğunda budget yenilenir ve Suspended task Ready'ye geçer. Bu model havacılıkta rate-monotonic scheduling ile uyumlu.

### 4.4 Windowed Watchdog

Her task'ın watchdog sayacı var. Her tick'te artırılır, `sys_yield` veya `watchdog_kick()` ile sıfırlanır. İki yönlü koruma:

**Üst sınır:** `watchdog_counter >= watchdog_limit` → task durdu, yanıt vermiyor → `WatchdogTimeout` policy event. Limit 0 = watchdog devre dışı.

**Alt sınır (Windowed):** `watchdog_counter < watchdog_window_min` durumunda kick gelirse → task çok hızlı çalışıyor, kontrol akışı bozulmuş (sonsuz döngü yerine çok hızlı döngü) → `WatchdogTimeout` policy event. `WATCHDOG_WINDOW_MIN = 3` tick.

**Neden windowed?** Basit watchdog "task durdu mu?" sorar. Windowed watchdog "task DOĞRU HIZDA mı çalışıyor?" sorar. ISO 26262 ve DO-178C'de windowed watchdog zorunlu. Maliyet: +1 compare/kick (~1 cycle).

### 4.5 schedule() Üç Fazlı

Scheduler her tick'te üç faz çalıştırır. Faz 1: period ilerletme + watchdog + IPC rate reset. Faz 2: policy kararlarını uygulama (budget, watchdog). Faz 3: en yüksek öncelikli Ready/Running task'ı seç, context switch.

### 4.6 Context Switch

`switch_context` assembly fonksiyonu 16 register (14 callee-saved + mepc + mstatus) kaydeder/yükler = 128 byte TaskContext. `ret` ile `task_trampoline` → `mret` → U-mode geçişi. Callee-saved convention sayesinde trap handler'da s0-s11 kaydedilmez — Rust calling convention zaten korur.

---

## 5. Syscall Dispatch

### 5.1 O(1) Jump Table

5 syscall için fonksiyon pointer tablosu: `cap_invoke`, `ipc_send`, `ipc_recv`, `yield`, `task_info`. Index bounds check → tek karşılaştırma → direkt atlama. Match/branch yok — O(1) dispatch.

**Neden match yerine tablo?** Match derleyiciye bağlı — jump table veya if-else zinciri üretebilir. Fonksiyon pointer tablosu her zaman O(1), deterministic.

### 5.2 Pointer Doğrulama

`is_valid_user_ptr()` beş katmanlı kontrol uygular: null kontrol, `checked_add` overflow koruması, kernel bellek sınırı (`ptr < kernel_end`), RAM üst sınırı (`end > RAM_END`), 8-byte alignment kontrolü.

**Neden RAM_END kontrolü?** Olmasa 0xFFFF_FFFF_FFFF_0000 gibi saçma adresler geçerli sayılır. PMP engelleyecek ama kernel'ın pointer'ı kabul edip sonra PMP trap'ine düşmesi WCET'i tahmin edilemez kılar.

### 5.3 WCET Ölçümü

Her syscall'da `rdcycle` ile başlangıç/bitiş cycle sayılır, `WCET_MAX` dizisine kaydedilir. `check_wcet_limits()` mevcut WCET'i hedefle karşılaştırır.

**Neden rdcycle?** QEMU TCG'de instruction count döner (gerçek cycle değil). Bu ölçüm göreli karşılaştırma için yeterli, kesin WCET → FPGA'da.

### 5.4 Syscall Sayacı

Her dispatch çağrısında aktif task'ın `syscall_count` alanı `wrapping_add(1)` ile artırılır. Anomali tespiti — bir task anormal sayıda syscall yapıyorsa DoS girişimi olabilir. Maliyet: 1 instruction/syscall.

### 5.5 IPC Rate Limiter

`sys_ipc_send` içinde `check_ipc_rate()` kontrolü. Tick başına `MAX_SENDS_PER_TICK = 16` mesaj limiti. Aşılırsa `E_RATE_LIMITED` dönülür. Her tick'te sayaç sıfırlanır. DoS koruması — kötü niyetli task IPC kanalını flood edemez.

### 5.6 Kernel Pointer Sanitizasyon

Syscall handler dönüş değeri kernel adres aralığındaysa (`RAM_BASE..kernel_end`) `E_INTERNAL` dönülür. Kernel pointer'ı asla U-mode'a sızdırılmaz — info leak koruması. Maliyet: 2 compare/syscall.

### 5.7 Argüman Truncation Koruması

`sys_cap_invoke` içinde `cap > u8::MAX`, `resource > u16::MAX`, `action > u8::MAX` kontrolü. `cap=256 → cap as u8 = 0` gibi sessiz truncation engellenir.

---

## 6. Capability Token Sistemi

### 6.1 Token Yapısı

32-byte `#[repr(C)]` token: id (u8), task_id (u8), resource (u16), action (u8), dal (u8), padding (2B), expires (u32), nonce (u32), MAC (16B). Stack-only, heap yok. Sabit boyut — PMP ile koruma, DMA transfer, serialization kolay.

**Neden 32 byte?** L1 cache line'ın yarısı (64B). İki token bir cache line'a sığar. Daha küçük olsa MAC alanı kısalır (güvenlik düşer), daha büyük olsa cache miss artar.

### 6.2 BLAKE3 Keyed Hash MAC

Token bütünlüğü BLAKE3 keyed hash ile korunur. 32-byte key boot'ta `provision_key()` ile bir kez yazılır. `validate_full()` token header'ından 16-byte MAC hesaplar, token'daki MAC ile constant-time karşılaştırır.

**Neden BLAKE3, neden HMAC-SHA256 değil?** BLAKE3 Rust-native, `no_std` uyumlu, ~350 cycle (SHA-256'dan 3-5x hızlı). Deterministik, timing side-channel korumalı. BLAKE3 portable backend kullanılır (SIMD optimizasyon devre dışı, `default-features = false`). Bu, platform bağımsız deterministic execution sağlar.

**Doğrulama kapsamı:** Kani proof'ları BLAKE3 API memory safety'yi doğrular (Kani stub kullanıyor — key'in ilk 16 byte'ını döndüren trivial impl). Kriptografik doğruluk Kani ile kanıtlanmaz — BLAKE3 crate external audit edilmiştir (Runtime Verification, Stellar Dev Foundation sponsorship, Dec 2025). Ed25519: `ed25519-dalek` 2.x, RUSTSEC-2022-0093 patched. Sipahi'nin sorumluluğu: crate'i doğru çağırmak + input bounds kontrol — bu Kani ile kanıtlanıyor.

### 6.3 4-Slot Constant-Time Cache

`TokenCache` 4 slot sabit zamanlı tarama yapar — her zaman 4 entry karşılaştırılır, erken çıkış yok. Bitwise AND ile hit accumulate, dallanmasız. Cache hit ~10 cycle, full validation ~400 cycle.

**Neden 4 slot?** 8 task × birkaç kaynak = pratikte 4 aktif token yeterli. Hash table kullanılmadı çünkü hash hesaplama non-deterministic cache miss'e neden olabilir. 4-slot linear scan her zaman aynı cycle.

**Neden erken çıkış yok?** Erken çıkış timing side-channel oluşturur. İlk slot'ta bulunan token vs dördüncü slot'ta bulunan token farklı süre alır — saldırgan hangi token'ın cache'te olduğunu ölçebilir.

### 6.4 Cache TTL

Her cache entry'nin `expires` alanı var. `lookup()` sırasında `get_tick() <= expires` kontrolü yapılır. Süresi dolan token cache'ten otomatik düşer. `expires = 0` → sonsuz geçerlilik.

### 6.5 Per-Task Nonce (Replay Guard)

`LAST_NONCE: [u32; MAX_TASKS]` — her task bağımsız monoton artan nonce takip eder. `token.nonce <= last_nonce[task_id]` → replay saldırısı → RED. Tek global nonce yerine per-task çünkü Task A'nın nonce'u Task B'yi etkilememeli.

### 6.6 Token Expiry

`token.expires > 0` ve `get_tick() > expires as u64` → expired token → RED. `get_tick()` epoch-based monotonic u64 döner — 49 günlük u32 wrap sorunu çözülmüş (`BB_BOOT_EPOCH << 32 | BB_TICK`).

### 6.7 ct_eq_16 (Constant-Time Compare)

16-byte MAC karşılaştırması bitwise XOR + OR accumulate ile yapılır. `memcmp` kullanılmaz çünkü ilk farklı byte'ta çıkar — timing side-channel. `ct_eq_16` her zaman 16 byte tarar.

---

## 7. Policy Engine — 5+1 Modlu Arıza Politikası

### 7.1 Tasarım

`decide_action(event, restart_count, dal)` pure fonksiyon — static mut yok, side effect yok. 9 event tipi, 6 FailureMode: Restart, Degrade, Isolate, Failover, Alert, Shutdown. Match tablosu — her yol sabit cycle.

**5+1 mod:** Failover v1.0'da Degrade'e fallback — hot-standby task switch mekanizması v2.0'da planlanıyor. `decide_action → Failover → runtime Degrade` uygular, blackbox kaydında `PolicyFailover` event olarak ayrışır (forensics için).

### 7.2 Escalation Zincirleri

| Event | İlk Tepki | Tekrarlı | Son |
|-------|-----------|----------|-----|
| BudgetExhausted | Restart | MAX_RESTART_BUDGET sonrası | Degrade |
| StackOverflow | Restart | MAX_RESTART_FAULT sonrası | Isolate |
| CapViolation | Isolate | — | — |
| PmpFail | Shutdown | — | — |
| WatchdogTimeout | Failover | MAX_RESTART_WATCHDOG sonrası | Degrade |
| DeadlineMiss | DAL-A→Failover, DAL-D→Isolate | — | — |
| MultiModuleCrash | Shutdown | — | — |
| Bilinmeyen (>8) | Isolate | — | — (fail-safe) |

**Neden Isolate fail-safe default?** Bilinmeyen event → Shutdown çok agresif (sistemi durdurur), Restart çok yumuşak (döngüye girebilir). Isolate sorunu karantinaya alır, sistem çalışmaya devam eder.

**Neden PMP fail her zaman Shutdown?** PMP bütünlük hatası = bellek koruması kırılmış. Sistem güvenilir değil — tek güvenli karar durdurma.

### 7.3 Policy Lockstep (Dual Redundancy)

`apply_policy()` içinde `decide_action()` iki kez çağrılır, sonuçlar karşılaştırılır. Aynı girdi farklı çıktı verirse = cosmic ray, bit flip, veya bellek bozulması → `FailureMode::Shutdown`. Bu olmadan fault injection ile policy engine manipüle edilebilir — Shutdown yerine Restart döndürülerek çökmüş task çalıştırılmaya devam edilebilir.

**Neden dual, neden triple (TMR) değil?** Single-hart sistemde iki çağrı arası bellek bozulması olasılığı astronomik düzeyde düşük. TMR üç çağrı + majority vote gerektirir — WCET'e ~10 cycle ekler. Dual yeterli koruma sağlar, ~5 cycle maliyetle.

### 7.4 Graceful Degradation

`degrade_system()` tetiklendiğinde DAL-C/D task'lar Suspended yapılır ve bütçeleri yarılanır. `DEGRADED` flag set edilir. Her scheduler tick'te `try_recover_from_degrade()` çağrılır: tüm DAL-A/B task'lar sağlıklıysa (hiçbiri Isolated değilse) DAL-C/D task'lar `original_budget` ile yeniden başlatılır.

**Neden bütçe yarılama?** Kurtarma sonrası DAL-C/D task'lar dikkatli modda çalışır — tam bütçeyle hemen yüklenme yerine yarı bütçeyle başlar. Orijinal bütçe `original_budget` alanında saklanır — döngüsel degrade'de bütçe sıfıra düşmez, her kurtarmada orijinale döner.

**Neden otomatik kurtarma?** Manual kurtarma operatör gerektirir — otonom sistemlerde (drone, araç) operatör yoktur. DAL-A/B sağlıklıysa sistem kendini toparlar. AEGIS Safety Island için kritik — Autoware çökerse MRM başlar, Autoware düzelirse AEGIS geri çekilir.

---

## 8. IPC — Lock-Free SPSC Ring Buffer

### 8.1 Tasarım

8 statik `SpscChannel`, her biri 16 slot × 64 byte mesaj. Lock-free — AtomicU16 head/tail, mutex yok. Tek üretici (task A) tek tüketici (task B) modeli. Full → `Err(BufferFull)`, veri ezilmez. Empty → `None`.

**Neden SPSC, neden MPMC değil?** MPMC lock veya CAS döngüsü gerektirir — WCET belirsiz. SPSC tek atomic read + tek atomic write = O(1), garanti edilmiş WCET.

**Neden AtomicU16?** u16 → 65,536 head/tail alanı. 16 slot ile `% 16` modulo kullanılır. u16 wrap olduğunda modulo hâlâ doğru çalışır (Kani proof: `ipc_ring_buffer_wrap_never_exceeds_slots`).

### 8.2 CRC32 Bütünlük Kontrolü

Her mesajın son 4 byte'ı CRC32. `set_crc()` payload'un CRC'sini hesaplar, `verify_crc()` doğrular. CRC32 bit-by-bit hesaplanır — lookup table yok.

**Neden lookup table yok?** 256-entry LUT = 1KB. L1 cache'te yoksa cache miss → non-deterministic latency. Bit-by-bit: her byte 8 iterasyon, deterministic. 60 byte payload × 8 = 480 iterasyon — sabit WCET.

---

## 9. Blackbox Flight Recorder

### 9.1 Tasarım

128 kayıt × 64 byte = 8KB circular buffer. Her kayıt: MAGIC (4B "SPHI"), version (2B), sequence (2B), timestamp (4B), task_id (1B), event (1B), data (46B), CRC32 (4B). Sadece kernel yazar — PMP ile korunur.

### 9.2 Power-Loss Koruması

CRC32 ile yarım yazılmış kayıt tespit edilir. Güç kesildi → kayıt yarım → CRC fail → `is_valid()` false → atlanır. `volatile` yazma ile LTO reorder engellenir.

**Neden HMAC-BLAKE3 değil?** CRC32 burada tamper koruması için değil, power-loss detection için. Blackbox'a sadece kernel yazıyor, PMP ile korunuyor. Fiziksel erişim saldırısı (JTAG/probe) FPGA+üretim seviyesinde çözülür, yazılımla değil.

### 9.3 Monotonic Tick

`BB_TICK` u32 her scheduler tick'te `wrapping_add(1)` ile artar. Wrap tespiti: `next < t` → `BB_BOOT_EPOCH` u16 artırılır. `get_tick()` → `(epoch << 32) | tick` = u48 etkili alan = ~900,000 yıl wrap-free.

### 9.4 Event Tipleri

14 event tipi: KernelBoot (0), TaskStart (1), TaskSuspend (2), TaskRestart (3), BudgetExhausted (4), PolicyIsolate (5), PolicyDegrade (6), PolicyFailover (7), PolicyShutdown (8), CapViolation (9), IopmpViolation (10), DeadlineMiss (11), WatchdogTimeout (12), PmpFail (13). Post-mortem analiz için altın madeni.

---

## 10. WASM Sandbox

### 10.1 Wasmi 1.0.9 Runtime

Wasmi interpreter — register-based bytecode, deterministic execution. JIT runtime (Wasmtime) yerine interpreter çünkü JIT non-deterministic (farklı platform = farklı native kod).

**Neden Wasmi 2.0-beta değil?** Safety-critical'da beta kullanılmaz. Wasmi 1.0.9 stable, register-based engine dahil. `prefer-btree-collections` feature ile `no_std` güvenliği — hash table yok (random init sorunu).

### 10.2 Fuel Metering

Her WASM instruction 1 fuel tüketir. Fuel tükenince execution durur. Sonsuz döngü imkansız — Kani liveness proof ile kanıtlanmış. Budget enforcement ile birlikte çift katman koruma.

### 10.3 Float Opcode Reddi

`validate_module()` modül yüklenirken tüm opcode'ları tarar. 0x43-0xBF aralığında float opcode bulunursa `Err(FloatOpcodes)` → modül reddedilir. `skip_instruction()` ile LEB128 immediate'ler ve sabit boyutlu operandlar (f32.const → 5B, f64.const → 9B) doğru atlanır — bounds check ile buffer overread imkansız (Kani proof ile bulunup düzeltildi).

### 10.4 BumpAllocator

4MB arena, O(1) allocation, free yok, fragmentasyon sıfır. `epoch_reset()` ile tüm arena sıfırlanır (modül yeniden yükleme). `checked_add` overflow koruması + `aligned >= WASM_HEAP_SIZE` OOM kontrolü. İki allocation asla örtüşmez (Kani proof: `bump_allocator_offsets_never_overlap`).

### 10.5 Compute Services

4 sabit servis: COPY (bellek kopyalama, ~80c), CRC (CRC32 hesaplama, ~120c), MAC (BLAKE3 keyed hash, ~350c), MATH (Q32.32 vektör dot product, ~200c). WCET hedefleri sabit, her servis bounded.

---

## 11. Secure Boot

### 11.1 Ed25519 İmza Doğrulama

Boot zinciri: ROM boot (M-mode) → Ed25519 imza doğrula → Sipahi kernel yükle. Ed25519 seçildi çünkü RSA-2048'e göre 64-byte imza (RSA: 256 byte) ve 32-byte public key (RSA: 256 byte) ile çok daha kompakt — bare-metal OTP fuse'a sığması gerekiyor. ECDSA-P256'ya göre ise sabit zamanlı doğrulama (ECDSA'da nonce bağımlı timing side-channel riski var) ve daha basit implementasyon. `ed25519-dalek` crate'i Rust-native, `no_std` uyumlu, RFC 8032 uyumlu — doğrulama sırasında heap tahsisatı yok (stack-only). Hatalı public key veya bozuk imza durumunda panic yerine `false` döner.

### 11.2 Key Provisioning Modeli

İki katmanlı key hiyerarşisi: Root key OTP fuse'da (değiştirilemez, cihaz ömrü), Module key .rodata'da (root key ile imzalı, güncellenebilir). QEMU v1.0'da OTP yok — `test-keys` feature ile compile-time sabit RFC 8032 Test Vector #1 kullanılır. Production'da factory provisioning: HSM içinde key pair üret → public key OTP'ye yaz → private key HSM'de kal → JTAG fuse yak.

### 11.3 CNSA 2.0 Yol Haritası

`fast-sign` (Ed25519) ve `cnsa-sign` (LMS post-quantum) mutually exclusive feature'lar. `compile_error!` ile hem ikisi birden aktif hem de ikisi de pasif durumu engellenir. LMS henüz implemente değil — v2.0'da eklenecek.

---

## 12. IOPMP (I/O Physical Memory Protection)

Stub implementasyon — gerçek IOPMP donanım (DMA controller) gerektirir. 8 bölge, enable/disable, `check_access(addr, size, write)` ile okuma/yazma/boyut kontrolü. FPGA'da aktif olacak. Disabled durumda tüm erişim serbest (fail-open), enabled durumda sadece tanımlı bölgelere erişim izinli.

---

## 13. Trap Handler

### 13.1 Assembly (trap.S)

16 caller-saved register save/restore (ra, t0-t6, a0-a7). CSR'lar (mcause, mepc) stack'e kaydedilir. ecall (mcause=8 U-mode, mcause=11 M-mode) → mepc+4 ilerletme → `trap_handler()` çağrı. ecall dönüşünde syscall sonucu saved a0 slot'una yazılır.

### 13.2 Rust (trap.rs)

Timer interrupt (code=7) → tick artır, scheduler çağır. ecall → syscall dispatch. Illegal instruction → ISOLATE. LoadAccessFault (5) ve StoreAccessFault (7) → PMP violation → ISOLATE + blackbox log. U-mode ecall sonrası MPP doğrulama.

### 13.3 Timer — Drift-Free

`schedule_next_tick()` önceki `mtimecmp` değerini okuyup `+ ticks_per_period()` ekler. `read_mtime()` bazlı değil çünkü handler gecikmesi birikimli drift yaratır.

---

## 14. Boot Sequence

`_start` (boot.S) → hart 0 seçimi → BSS sıfırlama → stack kurulumu → `rust_main`. `rust_main` (boot.rs) → PMP init → UART init → Timer init → task oluşturma → test suite → scheduler başlatma. Multi-hart: hart 0 dışındakiler `wfi` park.

---

## 15. Formal Doğrulama — 188 Kani Harness + 7/7 TLA+

### 15.1 Proof Dağılımı

| Modül | Proof | Kapsam |
|-------|-------|--------|
| verify.rs (global) | 57 | DAL, PMP, bellek, cross-module invariantlar |
| sandbox (mod+allocator) | 19+1 | LEB128, float tarama, bounds safety, allocator overlap |
| dispatch | 18 | Syscall tablo, pointer reddi, dispatch fuzzing |
| scheduler | 17 | Seçim doğruluğu, Isolated/Dead asla seçilmez, watchdog, priority |
| ipc | 15 | CRC roundtrip, kanal sınırları, ring buffer wrap |
| policy | 14 | Escalation zincirleri, PMP→Shutdown, livelock freedom |
| capability (mod+broker) | 14+2 | Token encoding, cache, invalidation, nonce, ct_eq_16 |
| blackbox | 14 | Record layout, CRC, wrap, tick monotonicity |
| crypto | 2 | BLAKE3 API memory safety (Kani stub) — cryptographic correctness via external audit |
| hal (iopmp+key+boot) | 2+1+1 | IOPMP boundary, key size, secure boot |
| **Toplam** | **188** | 88 symbolic proof (kani::any ile state space tarar) + 100 concrete/compile-time assertion |

### 15.2 Yüksek Değerli Proof'lar

Bu proof'lar `kani::any()` ile tüm olası girdi uzayını sembolik olarak tarar — sonsuz test eşdeğeri:

- **isolated_never_scheduled_any_config**: Tüm state/priority kombinasyonlarında Isolated task asla seçilmez
- **selected_has_minimum_priority**: Seçilen task her zaman en düşük priority numarasına sahip (priority inversion imkansız)
- **dispatch_rejects_invalid_syscall_id**: Geçersiz syscall ID → tam dispatch çağrısı ile E_INVALID_SYSCALL
- **policy_never_livelocks_on_repeated_failure**: 10 ardışık çöküşte terminal state'e ulaşılır (sonsuz restart döngüsü imkansız)
- **wasm_skip_instruction_never_exceeds_bounds**: Zehirli opcode/LEB128 ile buffer overread imkansız (bu proof gerçek bug buldu ve düzeltildi)
- **bump_allocator_offsets_never_overlap**: İki allocation asla örtüşmez
- **invalidated_token_never_found_in_cache**: İptal edilen token herhangi bir resource/action ile cache'te bulunamaz

### 15.3 Const Assert (Derleme Zamanı)

Sabit kontroller Kani'den çıkarılıp `const _: () = assert!(...)` ile derleme zamanına taşındı — 0 runtime maliyet, koşul sağlanmazsa kod derlenmez: Token == 32B, IpcMessage == 64B, SYSCALL_COUNT == 5, OTP_KEY_SIZE == 32, BLACKBOX_MAX_RECORDS <= 255, SIGNATURE_SIZE == 2 × OTP_KEY_SIZE.

---

## 16. Modüler Kriptografi — Compile-Time Trait Seçimi

### 16.1 HashProvider Trait

`HashProvider::keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 16]` — token MAC hesabı için. Rust monomorphization ile compile-time dispatch — runtime branching yok, seçilmeyen provider binary'de yer kaplamaz. `fast-crypto` → BLAKE3 (~350 cycle), `cnsa-crypto` → SHA-384 + Zknh HW (~1500 cycle, v2.0).

### 16.2 SignatureVerifier Trait

`SignatureVerifier::verify(public_key, message, signature) -> bool` — secure boot ve WASM modül doğrulama için. `fast-sign` → Ed25519, `cnsa-sign` → LMS post-quantum (v2.0). Trait sistemi sayesinde algoritma değişimi tek satır feature flag değişikliği — kernel kodu değişmez.

### 16.3 Feature Flag Sistemi

| Feature | Açıklama | Çakışma Koruması |
|---------|----------|------------------|
| `fast-crypto` | BLAKE3 hash/MAC | `cnsa-crypto` ile mutual exclusive |
| `cnsa-crypto` | SHA-384 + Zknh HW (v2.0) | `fast-crypto` ile mutual exclusive |
| `fast-sign` | Ed25519 imza | `cnsa-sign` ile mutual exclusive |
| `cnsa-sign` | LMS post-quantum (v2.0) | `fast-sign` ile mutual exclusive |
| `test-keys` | RFC 8032 test vektörleri | Production'da kapalı |
| `debug-boot` | Boot teşhis çıktısı | Production'da kapalı |

Çakışan feature'lar `compile_error!` ile engellenir — derleme hatası, runtime hatası değil. En az bir sign feature aktif olmalı — ikisi de kapalıysa derlenmez.

---

## 17. HAL — Hardware Abstraction Layer

### 17.1 DeviceAccess Trait

Tüm donanım aygıtları `DeviceAccess` trait'ini implemente eder: `init()`, `read_byte()`, `write_byte()`, `is_ready()`. Static dispatch — `dyn Trait` yasak, vtable overhead yok. Her operasyon bounded, blocking yok. Hata durumunda `SipahiError` döner, panic olmaz.

**Neden static dispatch?** `dyn Trait` vtable pointer dereferansı gerektirir — cache miss riski, WCET belirsizliği. Static dispatch: compiler fonksiyonu inline eder, sıfır overhead.

### 17.2 UartDevice

NS16550A UART implementasyonu. `putc()` LSR (Line Status Register) bit 5 ile transmit-ready kontrolü yapar — busy-wait ama UART donanımı her zaman boşalır (~1μs/byte). `read_byte()` LSR bit 0 ile data-ready kontrolü — veri yoksa `Err(DeviceNotReady)`, blocking değil.

### 17.3 Diagnosable Trait

Her subsystem sağlık kontrolü ve istatistik raporlama trait'i: `health_check() -> bool`, `stats() -> DiagStats`. DiagStats: name, ok, counter, error_count. API entegrasyonu v2.0'da planlanıyor (scaffolding mevcut, implementation pending).

---

## 18. Senkronizasyon — SingleHartCell

`UnsafeCell<T>` wrapper — zero-cost, lock yok, synchronization yok. SAFETY: sadece single-hart sistemde güvenli. Multi-hart desteği eklenirken `Mutex<T>` ile değiştirilecek. `Sync` trait'i `unsafe impl` ile sağlanır — derleyiciye "bu tip thread'ler arası paylaşılabilir" söylenir.

**Neden Mutex değil?** Mutex lock/unlock cycle'ı var — WCET'e eklenir, priority inversion riski. Single-hart'ta gereksiz overhead. Hubris, Tock, Embassy aynı pattern kullanır.

---

## 19. Hata Yönetimi

14 `SipahiError` variant'ı — her hata açık, sessiz başarısızlık yok. `as_str()` ile her variant'a karşılık gelen açıklama string'i. `#[must_use]` kritik fonksiyonlarda — derleyici sonucun kontrol edilmesini zorlar. Panic handler `wfi` loop — çökmek yerine durur. OOM handler aynı — heap tükenmesi kernel'ı çökertmez. `shutdown_system()` fonksiyonu UART'a log yazdırıp sonsuz `wfi` döngüsüne girer — donanım seviyesinde güvenli durdurma.

---

## 20. Boot-Time Integration Test Suite

Sipahi boot sırasında tüm subsystem'leri test eder — scheduler başlamadan önce. Test suite:

- **Policy Engine (6 test)**: Budget→Restart, Budget→Degrade, CapViolation→Isolate, PmpFail→Shutdown, DeadlineMiss DAL-A→Failover, DeadlineMiss DAL-D→Isolate
- **Capability Broker (3 test)**: validate_full MAC doğrulama, cap_invoke cache hit, cap_invoke cache miss denial
- **IPC SPSC (9 test)**: Empty recv, CRC set/verify, send OK, recv + CRC valid, double recv None, buffer full at 15, send when full Err, tampered CRC fail, invalid channel None
- **WCET Regression**: Her syscall'ın WCET limiti kontrol edilir (QEMU TCG'de bilgilendirme amaçlı)
- **Secure Boot**: BLAKE3 determinism, key-binding, Ed25519 (test-keys feature ile)
- **WASM Sandbox**: Module load, execute (result=42), fuel exhaustion trap, float rejection, epoch reset + reload
- **Blackbox**: Init kaydı, log kaydı, record doğrulama

Tüm testler UART üzerinden sonuç yazdırır: `✓` başarılı, `✗` başarısız.

### 20.2 POST — Power-On Self Test

Test suite'den önce çalışır. Bir tanesi bile fail ederse scheduler başlamaz — `wfi` loop ile halt. DO-178C DAL-A'da PBIT (Power-on Built-In Test) zorunlu.

- **CRC32 engine**: Bilinen vektör "123456789" → `0xCBF43926` (IEEE 802.3). Eşleşmezse CRC motoru bozuk — tüm bütünlük kontrolü güvenilmez.
- **PMP integrity**: `read_pmpcfg0()` ile gerçek register okunup boot'ta kaydedilen shadow ile karşılaştırılır. Uyuşmazlık = register bozulması.
- **Policy engine sanity**: `decide_action(PmpFail, 0, 0)` → Shutdown dönmeli. Dönmezse policy engine bozuk — yanlış güvenlik kararı riski.

Maliyet: sadece boot'ta, runtime sıfır overhead. Boot süresine ~1ms eklenir.

---

## 21. Task Veri Yapısı — Tam Alan Listesi

Her task 128-byte TaskContext + metadata alanları içerir:

| Alan | Tip | Açıklama |
|------|-----|----------|
| id | u8 | Task kimliği (0-7) |
| state | TaskState | Ready, Running, Suspended, Dead, Isolated |
| context | TaskContext | 16 register (ra, sp, s0-s11, mepc, mstatus) = 128B |
| entry | usize | Giriş noktası adresi (restart için) |
| stack_top | usize | Hizalanmış stack üstü (restart için) |
| priority | u8 | 0-15 (0=en yüksek, DAL-A grubu 0-3) |
| dal | u8 | Design Assurance Level (0=A, 1=B, 2=C, 3=D) |
| budget_cycles | u32 | Periyot başına CPU bütçesi (cycle) |
| remaining_cycles | u32 | Bu periyotta kalan cycle |
| period_ticks | u32 | Periyot uzunluğu (tick) |
| period_counter | u32 | Mevcut periyot içindeki tick sayacı |
| watchdog_counter | u32 | Tick sayacı — yield/kick ile sıfırlanır |
| watchdog_limit | u32 | Limit (0=devre dışı) — aşılırsa policy tetiklenir |
| watchdog_window_min | u32 | Windowed watchdog alt sınır — kick çok erkense hata |
| syscall_count | u32 | Anomali tespiti — dispatch'te wrapping_add(1) |
| ipc_send_count | u32 | Rate limiter — tick'te sıfırlanır |
| original_budget | u32 | Degrade öncesi orijinal bütçe (kurtarma için) |
| pmp_addr_napot | usize | NAPOT-encoded PMP address (entry 8, per-task stack) |

Tüm alanlar statik tahsis — heap yok. `Task::empty()` ile sıfırlanmış varsayılan değerler. `restart_task()` context'i sıfırlar, entry + stack + mepc + mstatus yeniden ayarlar, `task_trampoline` U-mode geçişi için ra'ya atanır.

---

## 22. Güvenlik Duvarları (6 Katman)

| # | Duvar | Durum | Açıklama |
|---|-------|-------|----------|
| 1 | WASM Sandbox | ✅ Tam | Fuel metering + float reddi + izole bellek |
| 2 | Capability Token | ✅ Tam | BLAKE3 MAC + nonce + expiry + constant-time cache |
| 3 | PMP (kernel) | ✅ Tam | 4 TOR bölge, L-bit kilitleme + shadow register |
| 4 | PMP (per-task) | ✅ Tam | Task stacks Entry 5 dışı, NAPOT entry 8, WASM arena M-mode only |
| 5 | IOPMP | ⚠️ Stub | Gerçek donanım (DMA controller) gerektirir — FPGA |
| 6 | M/U-mode ayrımı | ✅ Tam | Kernel M-mode, task'lar U-mode, mret geçişi |
| 7 | Fiziksel | ❌ Yok | JTAG/OTP/tamper — FPGA+üretim seviyesi |

Yazılım seviyesinde çözülebilen 5/7 duvar tamamlandı. Kalan 1 donanım (IOPMP), 1 üretim seviyesi (fiziksel).

---

## 23. Hardening Özellikleri

| Özellik | Maliyet | Koruduğu Saldırı |
|---------|---------|-------------------|
| PMP shadow register | ~5 cycle/tick | Fault injection (PMP register bozma) |
| mstatus.MPP doğrulama | ~5 cycle/ecall | Privilege escalation |
| Syscall sayacı | ~1 cycle/dispatch | Anomali tespiti / DoS |
| IPC rate limiter | ~2 cycle/send | IPC flood DoS |
| Kernel pointer sanitize | ~2 cycle/syscall | Info leak (kernel adres sızıntısı) |
| Argüman truncation kontrolü | ~3 cycle/cap_invoke | Sessiz truncation → yanlış token ID |
| Timer drift-free | 0 ek cycle | Birikimli zamanlama kayması |
| BB_TICK epoch | ~3 cycle/wrap | 49 günlük u32 wrap → token expiry kırılması |
| Windowed watchdog | ~1 cycle/kick | Kontrol akışı bozulması (çok hızlı döngü) |
| Policy lockstep | ~5 cycle/policy | Fault injection (policy karar manipülasyonu) |
| Graceful degradation | 0 (tetiklenince O(N)) | DAL-C/D otomatik kurtarma, bütçe koruması |
| POST (boot-time) | 0 (runtime yok) | Bozuk RAM/CRC/PMP/policy ile boot |

Toplam hardening overhead: ~25 cycle/tick — Sipahi'nin 1.5μs WCET bütçesinin %1.5'inden az.

---

## 24. Format ve Diagnostic Yardımcıları

UART üzerinden debug çıktısı için heap-free format fonksiyonları: `print_u32` (ondalık), `print_u64` (ondalık), `print_hex` (hex, 0x prefix yok). Tümü stack-based buffer kullanır — `[u8; 10]` u32 için, `[u8; 20]` u64 için. `alloc::format!` veya `core::fmt` kullanılmaz — bunlar binary boyutunu şişirir ve non-deterministic olabilir.

---

## 25. Build Sistemi ve Araçlar

- **Toolchain:** Rust nightly-2026-03-01, riscv64imac-unknown-none-elf target
- **Build:** `make build` (build-std flags), `cargo clippy -- -D warnings` (target config.toml'da)
- **Run:** `make run` (QEMU 8.2.2 virt machine, -bios none, 512MB RAM)
- **Verify:** `cargo kani` (188 harness), const assert (7 derleme zamanı kontrol), TLC (7 TLA+ spec)
- **Supply chain:** `cargo audit` (RustSec CVE scan, 0 CVE) + `cargo deny check` (license/bans/sources policy)
- **CI:** GitHub Actions 4 job — clippy+build, QEMU boot test (HALT criteria), supply chain audit, Kani (master push only)
- **WASM:** Wasmi 1.0.9, `default-features = false`, `prefer-btree-collections`
- **Crypto:** BLAKE3 (`fast-crypto` feature), Ed25519 (`fast-sign` feature, `ed25519-dalek`)

---

## 26. Performans Hedefleri (100MHz CVA6)

| İşlem | Hedef | Karşılık |
|-------|-------|----------|
| trap_entry | ≤30 cycle | ≤0.30μs |
| sys_yield | ≤10 cycle | ≤0.10μs |
| ipc_recv | ≤40 cycle | ≤0.40μs |
| ipc_send | ≤60 cycle | ≤0.60μs |
| scheduler_tick | ≤80 cycle | ≤0.80μs |
| cap_invoke (cache hit) | ≤10 cycle | ≤0.10μs |
| token_validate (BLAKE3) | ≤400 cycle | ≤4.00μs |
| Toplam syscall (worst case) | ≤1.5μs | — |

Kesin ölçüm FPGA'da yapılacak. QEMU TCG'de rdcycle instruction count döner, gerçek cycle değil.

---

## Ek-1. Windowed Watchdog

### Tasarım

Sipahi'nin watchdog'u iki yönlü çalışır — hem üst sınır hem alt sınır kontrolü yapar.

**Üst sınır (`watchdog_limit`):** Task belirli sürede kick göndermezse stuck kabul edilir. `watchdog_counter` her tick'te artırılır, `sys_yield` veya `watchdog_kick()` ile sıfırlanır. `counter >= limit` → `WatchdogTimeout` policy event tetiklenir.

**Alt sınır (`watchdog_window_min`):** Task çok erken kick gönderirse kontrol akışı bozulmuş kabul edilir. `watchdog_kick()` çağrıldığında `counter < window_min` ise → `WatchdogTimeout` policy event tetiklenir.

### Neden İki Yönlü?

Basit watchdog sadece stuck task yakalar. Ama bir task sonsuz döngüye girip her iterasyonda kick çağırıyorsa, basit watchdog bunu "sağlıklı" görür. Windowed watchdog bu durumu yakalar: kick çok hızlı geliyorsa task'ın normal kontrol akışı bozulmuş demektir.

### Parametreler

`WATCHDOG_WINDOW_MIN = 3` tick. Task en erken 3 tick sonra kick gönderebilir. 1. veya 2. tick'te kick gelirse → policy engine devreye girer.

Maliyet: ~1 cycle/kick (tek karşılaştırma).

---

## Ek-2. Policy Lockstep (Yazılım Dual Execution)

### Tasarım

`decide_action()` her çağrıda iki kez çalıştırılır. İki sonuç farklıysa `Shutdown` tetiklenir.

### Neden?

`decide_action()` pure fonksiyon — aynı girdiye her zaman aynı çıktıyı vermeli. Farklı sonuç = donanım seviyesinde bozulma (kozmik ışın, fault injection, RAM hatası). Policy engine kernel'ın "beyni" — kararın doğruluğu tüm sistemin güvenliğinin temelidir.

Maliyet: ~5 cycle/policy kararı.

---

## Ek-3. Graceful Degradation + Auto-Recovery

### degrade_system()

DAL-C/D task'lar Suspended yapılır, bütçeleri yarılanır. DAL-A/B tam bütçeyle devam eder.

### try_recover_from_degrade()

Her tick'te çağrılır. DAL-A/B sağlıklıysa DAL-C/D `original_budget` ile Ready'ye döner.

### Neden original_budget?

Her degrade/recover döngüsünde bütçe yarılanmasın diye orijinal değer saklanır. Recovery'de orijinale dönülür.

Maliyet: 0 cycle normal çalışmada, O(N) tetiklendiğinde (~20 cycle, N=8).

---

## Ek-4. POST (Power-On Self-Test)

Boot sırasında: CRC32 bilinen vektör, PMP shadow integrity, policy engine sanity. Herhangi biri fail → `wfi` halt, scheduler başlamaz.

Maliyet: 0 cycle runtime (sadece boot, ~100 cycle).

---

## Ek-5. Illegal Instruction → ISOLATE

U-mode task illegal instruction çalıştırınca trap handler `WasmTrap` event gönderir → policy → ISOLATE. Restart değil çünkü illegal instruction genelde bellek bozulması veya saldırı — restart sorunu çözmez.

---

## Ek-6. IPC Head wrapping_add Güvenliği

`head + 1` yerine `head.wrapping_add(1)` — `overflow-checks = true` ile u16::MAX'ta panic engellenir. Modulo hâlâ doğru çalışır (Kani proof: `ipc_ring_buffer_wrap_never_exceeds_slots`).

---

## Ek-7. ct_eq_16 + black_box LLVM Barrier

16-byte MAC karşılaştırması constant-time: bitwise XOR + OR accumulate, erken çıkış yok. `core::hint::black_box()` LLVM'in döngüyü `memcmp`'ye optimize etmesini engeller. Olmasa timing side-channel ile MAC 4096 denemede kırılabilir.

---

## Ek-8. TLA+ Formal Doğrulama — Sistem Seviyesi

7 TLA+ spec: SipahiIPC (✅), SipahiWatchdog (✅), SipahiCapability (✅), SipahiPolicy (✅), SipahiScheduler (✅), SipahiBudgetFairness (✅), SipahiDegradeRecover (✅).

Kani fonksiyon seviyesinde, TLA+ sistem seviyesinde doğrulama yapar. İkisi farklı soruları cevaplar ve birbirini tamamlar. Sprint U-12'de tüm spec'ler TLC 2026.04 uyumluluğuna getirildi (tick bound → StateConstraint, bounded message ID, WF→SF fairness ayarlamaları).

*Sipahi Microkernel v1.5 — 188 Kani Harness · 7/7 TLA+ Verified · 12 Hardening · 0 Clippy Warning · 0 Runtime Panic · 0 Heap Allocation (kernel) · 5/7 Security Wall Active*
