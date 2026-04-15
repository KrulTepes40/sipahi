# Sipahi Microkernel — Yeni Sohbet Bağlam Dokümanı

> Bu doküman Sipahi kod tabanını ilk kez inceleyen AI veya geliştiriciler içindir.
> Aşağıdaki tasarım kararları bilinçli ve gerekçelidir — bug değildir.

**Proje:** Safety-critical RISC-V microkernel
**Dil:** Rust `no_std` · **Hedef:** `riscv64imac-unknown-none-elf`
**LOC:** ~7,360 · **Kani Proof:** 173 (all PASS) · **TLA+ Spec:** 3/7 verified
**Geliştirici:** Gazihan (GitHub: KrulTepes40)
**Repo:** https://github.com/KrulTepes40/sipahi (master, tag v1.5)

---

## Build Komutları (ÖNEMLİ)

```bash
cargo clippy -- -D warnings    # target config.toml'da — --target FLAG KULLANMA
make build                      # build-std flags Makefile'da — cargo build --release KULLANMA
make run                        # QEMU flags Makefile'da
```

`--target` flag'i clippy'de KULLANILMAZ çünkü `config.toml`'da tanımlı.
`cargo build --release` tek başına KULLANILMAZ çünkü `build-std` flag'leri Makefile'dadır.

---

## Bug Zannedilen Tasarım Kararları

### 1. Float yok (F/D extension dahil edilmedi)

**Bug değil.** `riscv64imac` kasıtlı — F/D yok. IEEE 754 float farklı donanımlarda farklı sonuç üretebilir (rounding mode, denormalized numbers). Sipahi tüm hesaplamaları Q32.32 fixed-point (`i64`) ile yapar. WASM modüllerinde float opcode tespit edilirse modül reddedilir (`is_float_opcode()`). Compiler seviyesinde float kullanımı engellenir.

### 2. MMU yok, sadece PMP

**Bug değil.** S-mode + MMU kullanılmıyor çünkü sayfa tablosu overhead'i ve TLB flush non-determinism WCET'i tahmin edilemez kılar. M/U-mode ayrımı PMP ile fiziksel bellek koruması sağlar. Per-task PMP (pmpcfg2, entry 8-15) Sprint U-3'te planlanmış.

### 3. Dinamik bellek tahsisatı yok (Vec, Box, HashMap yok)

**Bug değil.** Kernel seviyesinde heap allocation yapılmaz. Tüm yapılar sabit boyutlu: `[Task; MAX_TASKS]`, `[SpscChannel; MAX_CHANNELS]`, `[TokenCache; 4]`. `alloc` crate sadece WASM sandbox (Wasmi runtime) için linklenir. `malloc`/`free` çağrısı yok — use-after-free, double-free, fragmentation imkansız.

### 4. `wrapping_add` kullanımı (overflow-checks = true olmasına rağmen)

**Bug değil.** `overflow-checks = true` aktif — normal `+` operatörü overflow'da panic atar. `wrapping_add` bilinçli olarak kullanılır çünkü bu değerlerin wrap etmesi bekleniyor: `BB_TICK` (monotonic counter), IPC `head`/`tail` (ring buffer indeksi), `syscall_count` (u32 wrap güvenli). `saturating_sub` ise budget enforcement'ta kullanılır — budget asla negatife düşmemeli.

### 5. `ct_eq_16` + `core::hint::black_box` (basit `==` yerine)

**Bug değil.** MAC karşılaştırması constant-time yapılmalı. `==` veya `memcmp` ilk farklı byte'ta çıkar → timing side-channel. `black_box()` LLVM'nin döngüyü optimize edip erken çıkış eklemesini engeller. Sadece 16-byte MAC için kullanılır, diğer karşılaştırmalar normal.

### 6. `decide_action()` iki kez çağrılıyor (policy lockstep)

**Bug değil.** Duplicate kod değil — yazılım dual execution. Pure fonksiyon iki kez çalıştırılır, sonuçlar karşılaştırılır. Farklıysa donanım seviyesinde bozulma (bit flip, fault injection) → Shutdown. Donanım lockstep CPU'nun yazılım eşdeğeri.

### 7. Scheduler sabit boyutlu array (linked list veya priority queue değil)

**Bug değil.** `[Task; 8]` sabit array, O(N) linear scan (N=8). Priority queue O(log N) ama heap allocation veya pointer chasing gerektirir — cache miss, non-deterministic. 8 task için linear scan her zaman aynı cycle = constant-time garantisi, dallanmasız.

### 8. CRC32 lookup table yok (bit-by-bit hesaplama)

**Bug değil.** 256-entry LUT = 1KB L1 cache'te yoksa cache miss → non-deterministic latency. Bit-by-bit: her byte 8 iterasyon, toplam 480 iterasyon (60 byte payload) — sabit WCET. Performans trade-off bilinçli: determinizm > hız.

### 9. Wasmi 1.0.9 (2.0-beta veya Wasmtime değil)

**Bug değil.** Beta sürüm safety-critical'da kullanılmaz. JIT (Wasmtime) farklı platformlarda farklı native kod üretir — non-deterministic. Wasmi 1.0.9 interpreter, register-based bytecode, deterministic execution. `prefer-btree-collections` feature ile hash table yok (random init sorunu).

### 10. `SingleHartCell` (Mutex yerine UnsafeCell wrapper)

**Bug değil.** Sipahi tek hart (CPU core) üzerinde çalışır. Mutex lock/unlock cycle'ı WCET'e eklenir, priority inversion riski oluşturur — tek hart'ta gereksiz overhead. `unsafe impl Sync` bilinçli — multi-hart desteği eklenirken `Mutex<T>` ile değiştirilecek.

### 11. `dyn Trait` kullanılmıyor (static dispatch only)

**Bug değil.** `dyn Trait` vtable pointer dereferansı gerektirir — cache miss riski, WCET belirsizliği. Tüm trait'ler (`DeviceAccess`, `HashProvider`, `SignatureVerifier`) static dispatch ile monomorphize edilir. Compiler fonksiyonu inline eder, sıfır overhead.

### 12. Sadece 5 syscall

**Bug değil.** Minimal attack surface: `cap_invoke`, `ipc_send`, `ipc_recv`, `yield`, `task_info`. Az syscall = az doğrulama yüzeyi. Her syscall O(1) jump table dispatch ile çağrılır (match/branch değil). Yeni syscall eklemek kolay ama gerekçe olmadan eklenmez.

### 13. `core::fmt` kullanılmıyor (custom `print_u32`, `print_hex`)

**Bug değil.** `core::fmt` büyük binary üretir ve execution time belirsiz (format string parsing, trait dispatch). Custom fonksiyonlar heap-free, deterministic, minimal code size. Sadece u32, u64, hex desteklenir — yeterli.

### 14. Task'lar sonsuz loop içinde (`loop { yield_to_scheduler(); }`)

**Bug değil.** Safety-critical task'lar asla return etmez. Fonksiyon sonuna düşme = undefined behavior (stack'ten rastgele adrese jump). Sonsuz loop + yield = kontrollü çalışma. Task bitirmek isterse state machine ile `Dead` state'e geçer.

### 15. `#[repr(C)]` token yapısı (Rust struct layout yerine)

**Bug değil.** 32-byte token'ın bellek düzeni öngörülebilir olmalı — PMP koruma, DMA transfer, cross-platform serialization. Rust default struct layout compiler versiyonuna göre değişebilir. `#[repr(C)]` sabit, öngörülebilir, binary-stable.

### 16. IPC'de SPSC (tek üretici tek tüketici), MPMC değil

**Bug değil.** MPMC lock veya CAS döngüsü gerektirir — WCET belirsiz. SPSC tek atomic read + tek atomic write = O(1), garanti edilmiş WCET. Task A → Task B iletişimi için bir kanal, Task B → Task A için ayrı kanal. 8 statik kanal yeterli.

### 17. Illegal instruction → ISOLATE (RESTART değil)

**Bug değil.** Illegal instruction genelde bellek bozulması veya saldırı göstergesi. Restart aynı instruction'a döner → sonsuz trap döngüsü. ISOLATE kalıcı karantina — task bir daha schedule edilmez, sistem çalışmaya devam eder.

### 18. PMP L-bit (Lock) kullanımı — M-mode bile kısıtlı

**Bug değil.** L-bit set edildiğinde PMP kuralları M-mode dahil tüm privilege seviyelerinde zorlanır. Kernel'ın kendi .text bölgesine yazmasını bile engeller (W^X). Yanlışlıkla kernel kodunu ezmek imkansız.

### 19. `compile_error!` ile feature flag çakışma kontrolü

**Bug değil.** `fast-crypto` ve `cnsa-crypto` aynı anda aktif olamaz. İkisi de pasif olamaz. Bu runtime hatası değil, derleme hatası — yanlış konfigürasyon binary üretmeden yakalanır.

### 20. Policy engine pure fonksiyon (static mut yok, side effect yok)

**Bug değil.** `decide_action(event, restart_count, dal) -> FailureMode` — aynı girdiye her zaman aynı çıktı. Bu lockstep doğrulamasının ön koşulu. Side effect olsaydı iki çağrı farklı sonuç üretebilirdi (birinci çağrı state değiştirir, ikinci farklı state görür) ve lockstep çalışmazdı.

---

## Mimari Özet

```
┌─────────────────────────────────────────┐
│              M-mode (Kernel)            │
│  ┌─────────┐ ┌──────┐ ┌────────────┐   │
│  │Scheduler│ │Policy│ │ Capability │   │
│  │(budget, │ │(6mod,│ │ (BLAKE3    │   │
│  │ watch-  │ │lock- │ │  MAC, 4-   │   │
│  │ dog,    │ │step) │ │  slot CT   │   │
│  │ PMP)    │ │      │ │  cache)    │   │
│  └─────────┘ └──────┘ └────────────┘   │
│  ┌──────┐ ┌────────┐ ┌──────────────┐  │
│  │ IPC  │ │Blackbox│ │  Trap/Timer  │  │
│  │(SPSC,│ │(flight │ │  (CLINT,     │  │
│  │ CRC) │ │ rec.)  │ │   ecall)     │  │
│  └──────┘ └────────┘ └──────────────┘  │
├─────────────────────────────────────────┤
│              U-mode (Tasks)             │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐  │
│  │Task 0│ │Task 1│ │Task 2│ │ ...  │  │
│  │DAL-A │ │DAL-B │ │DAL-C │ │      │  │
│  └──────┘ └──────┘ └──────┘ └──────┘  │
├─────────────────────────────────────────┤
│         WASM Sandbox (Wasmi 1.0.9)      │
│   fuel metering · float reject · Q32.32│
└─────────────────────────────────────────┘
```

## Doğrulama Katmanları

| Katman | Araç | Kapsam |
|--------|------|--------|
| Derleme zamanı | `const assert!`, `compile_error!`, Clippy | Yapı boyutları, feature çakışma, lint |
| Fonksiyon seviyesi | Kani (173 proof) | Buffer bounds, overflow, invariant'lar |
| Sistem seviyesi | TLA+ (3/7 verified) | FIFO ordering, eventual delivery, fault detection |
| Runtime | POST, PMP shadow, watchdog, lockstep | Boot integrity, tamper detection |

## Hardening Özet (12 özellik, ~25 cycle/tick overhead)

PMP shadow register, mstatus.MPP doğrulama, syscall sayacı, IPC rate limiter, kernel pointer sanitize, argüman truncation kontrolü, timer drift-free, BB_TICK epoch, windowed watchdog, policy lockstep, graceful degradation + auto-recovery, POST.

---

*Sipahi v1.5 — 173 Kani · 3 TLA+ · 0 Clippy · 0 Panic · 0 Heap (kernel)*
