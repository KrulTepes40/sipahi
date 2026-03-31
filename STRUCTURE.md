# SİPAHİ v1.0 — KLASÖR YAPISI

```
sipahi/
├── .cargo/
│   └── config.toml          # RISC-V target ayarları
├── src/
│   ├── main.rs              # #![no_std] #![no_main] entry point
│   ├── arch/                # KATMAN 0: Donanım (1,200 satır bütçe)
│   │   ├── mod.rs
│   │   ├── boot.S           # _start, stack ayarla, Rust'a atla (~30 satır)
│   │   ├── trap.S           # register save/restore (~40 satır)
│   │   ├── context.S        # task context switch (~60 satır)
│   │   ├── csr.rs           # CSR okuma/yazma (mtvec, mcause, mepc, pmp*)
│   │   └── uart.rs          # UART driver (debug çıktı)
│   ├── hal/                 # KATMAN 0: HAL soyutlama
│   │   ├── mod.rs
│   │   ├── pmp.rs           # PMP bölgeleri + integrity check (~100 satır)
│   │   ├── clint.rs         # Timer (mtime, mtimecmp)
│   │   ├── device.rs        # HalDevice trait
│   │   ├── boot.rs          # Boot sequence (~300 satır)
│   │   ├── secure_boot.rs   # Ed25519 doğrulama, imza kontrol (~350 satır)
│   │   └── key.rs           # OTP/HSM key provisioning (~250 satır)
│   ├── kernel/              # KATMAN 1: Çelik Çekirdek (4,900 satır bütçe)
│   │   ├── mod.rs
│   │   ├── scheduler/
│   │   │   ├── mod.rs
│   │   │   ├── task.rs      # Task struct, TaskState enum
│   │   │   ├── priority.rs  # Fixed-priority preemptive
│   │   │   └── budget.rs    # Budget enforcement, DAL safety factor
│   │   ├── capability/
│   │   │   ├── mod.rs
│   │   │   ├── token.rs     # Token struct (32B), lifecycle
│   │   │   ├── broker.rs    # Capability doğrulama, BLAKE3 keyed hash
│   │   │   └── cache.rs     # Token cache (4 slot, constant-time)
│   │   ├── syscall/
│   │   │   ├── mod.rs
│   │   │   └── dispatch.rs  # 5 syscall: cap_invoke, ipc_send/recv, yield, task_info
│   │   ├── memory/
│   │   │   ├── mod.rs
│   │   │   └── regions.rs   # R0-R7 statik bellek bölgeleri
│   │   └── policy/
│   │       ├── mod.rs
│   │       └── failure.rs   # 6 failure modu, escalation
│   ├── ipc/                 # KATMAN 2: Sinir Sistemi (1,100 satır bütçe)
│   │   ├── mod.rs
│   │   ├── channel.rs       # SPSC ring buffer + IPC CONTRACT
│   │   ├── message.rs       # 64B mesaj formatı, CRC32
│   │   └── blackbox.rs      # Flight recorder, circular buffer
│   ├── sandbox/             # KATMAN 3: WASM İzolasyon (1,800 satır bütçe)
│   │   ├── mod.rs
│   │   ├── runtime.rs       # Wasmi host interface + fuel bridge
│   │   ├── loader.rs        # Module loader + Ed25519 + yükleme politikası
│   │   └── compute.rs       # 4 compute service (COPY, CRC, MAC, MATH)
│   └── common/              # Genel: Config + Types + Error + Crypto (300 satır)
│       ├── mod.rs
│       ├── types.rs         # Q32.32 fixed-point, ortak tipler
│       ├── error.rs         # SipahiError enum
│       ├── config.rs        # MAX_TASKS, TICK_PERIOD, WCET hedefleri, syscall/compute ID
│       └── crypto/          # Modüler kriptografi (compile-time trait seçimi)
│           ├── mod.rs
│           └── provider.rs  # HashProvider + SignatureVerifier trait
├── tests/
│   ├── unit/                # Birim testler (host makinede çalışır)
│   │   └── mod.rs
│   ├── integration/         # Entegrasyon testleri (QEMU'da)
│   │   └── mod.rs
│   └── fi/                  # Fault injection testleri (FI-1 ~ FI-7)
│       └── mod.rs
├── docs/
│   └── sipahi_v10_0.txt     # Mimari doküman
├── sipahi.ld                # Linker script
├── Cargo.toml               # Bağımlılıklar ve feature flag'ler
├── rust-toolchain.toml      # Nightly pinleme (determinizm)
├── .gitignore               # Git ignore kuralları
├── Makefile                 # build, run, test, wcet kısayolları
├── LICENSE                  # Apache 2.0
└── README.md                # (v1.0 sonunda yazılacak)
```

## BÜTÇE EŞLEŞMESİ

| Klasör      | Katman | Bütçe    | Sprint  |
|-------------|--------|----------|---------|
| src/arch/   | K0     | ~400     | 1-2     |
| src/hal/    | K0     | ~800     | 1,3,5,13|
| src/kernel/ | K1     | ~4,900   | 3-7,9-10|
| src/ipc/    | K2     | ~1,100   | 8,11    |
| src/sandbox/| K3     | ~1,800   | 12      |
| src/common/ | Genel  | ~300     | 0-1     |
| Rezerve     | —      | ~700     | —       |
| **TOPLAM**  |        | **10,000**|        |

## SPRINT → DOSYA HARİTASI

| Sprint | Dosyalar | Çıktı |
|--------|----------|-------|
| 0      | Cargo.toml, .cargo/config.toml, sipahi.ld, LICENSE | Derleme ortamı hazır |
| 1      | boot.S, main.rs, uart.rs, common/* | QEMU'da "Sipahi" yazısı |
| 2      | trap.S, csr.rs | Interrupt yakalanıyor |
| 3      | clint.rs, scheduler/task.rs | Timer tick çalışıyor |
| 4      | context.S, scheduler/priority.rs | 2 task arası geçiş |
| 5      | hal/pmp.rs, memory/regions.rs | Bellek izolasyonu aktif |
| 6      | hal/device.rs | HalDevice trait + IOPMP stub |
| 7      | syscall/dispatch.rs | ecall → 5 syscall çalışıyor |
| 8      | ipc/channel.rs, ipc/message.rs | Task'lar arası mesaj |
| 9      | capability/token.rs, broker.rs, cache.rs | Token doğrulama aktif |
| 10     | scheduler/budget.rs, policy/failure.rs | Budget aşımı → SUSPENDED |
| 11     | ipc/blackbox.rs | Olay kaydı çalışıyor |
| 12     | sandbox/runtime.rs, loader.rs, compute.rs | WASM task çalışıyor |
| 13     | hal/boot.rs, secure_boot.rs, key.rs | İmzalı kernel boot |
| 14     | tests/integration/*, tests/fi/* | 12 release gate PASS |

## SPRINT BAĞIMLILIK GRAFİĞİ

```
Sprint 0  (proje setup)
   │
   ▼
Sprint 1  (boot + UART) ──────── "Sipahi lives."
   │
   ▼
Sprint 2  (trap handler)
   │
   ▼
Sprint 3  (timer + task struct)
   │
   ▼
Sprint 4  (context switch) ───── İlk multi-task
   │
   ▼
Sprint 5  (PMP + memory) ─────── Bellek izolasyonu
   │
   ▼
Sprint 6  (device trait) ──────── HAL soyutlama
   │
   ▼
Sprint 7  (syscall dispatch) ─── User→Kernel geçişi
   │
   ▼
Sprint 8  (SPSC IPC) ─────────── Task iletişimi ← KRİTİK SPRINT
   │
   ▼
Sprint 9  (capability broker) ── Güvenlik katmanı
   │
   ▼
Sprint 10 (budget + failure) ─── Determinizm katmanı
   │
   ▼
Sprint 11 (blackbox) ─────────── Olay kaydı
   │
   ▼
Sprint 12 (WASM sandbox) ─────── İzolasyon katmanı
   │
   ▼
Sprint 13 (secure boot) ──────── Güven zinciri
   │
   ▼
Sprint 14 (entegrasyon) ───────── v1.0 RELEASE GATE
```

Her sprint bir öncekine bağımlıdır — sıra atlanamaz.
Sprint 8 (IPC) en kritik sprint: "IPC correctness = system correctness."

## DOSYA BOYUT KURALI

Her dosya MAX 400 satır. 400'ü aşarsa parçala.
Neden: code review, test, Kani analizi dosya bazında çalışır.
Büyük dosya = kaçan bug.

hal/boot.rs ESKİ 900 satır → YENİ 3 dosya × ~300 satır.
