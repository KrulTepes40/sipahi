# Sipahi Native Task Model (SNTM) — Tasarım Dokümanı

> **Versiyon:** v0.8 — §8 syscall API tutarsızlık fix (Codex U-23 prompt review)
> **Hedef:** Sipahi v1.5 → v2.0 (WASM yerine native PMP-isolated task'lar)
> **Yazar:** Gazihan (KrulTepes40)
> **Tarih:** 15 Mayıs 2026
> **Durum:** Araştırma + Tasarım — Implementation YAPILMADI
> **Önceki:** v0.7 (13 Mayıs 2026), v0.6 (13 Mayıs 2026), v0.5 (12 Mayıs 2026), v0.4 (11 Mayıs 2026), v0.3 (10 Mayıs 2026), v0.2 (10 Mayıs 2026), v0.1 (3 Mayıs 2026)
>
> **v0.7 → v0.8 Değişiklikleri (§8 syscall surface fix):**
> - §8 stale iddia "yeni syscall gerekmez" düzeltildi:
>   §4.8.3 panic_handler `syscall::exit(255)` çağırıyor → SYS_EXIT (6.
>   syscall) zorunlu. v0.2'den beri kendi içinde tutarsız olduğu
>   U-23 prompt review'da yakalandı. SYS_EXIT semantiği + kernel-side
>   isolate_task + schedule_yield açıklaması eklendi.
> - Syscall ID şeması açık tablo: SYS_CAP_INVOKE..SYS_EXIT=5, SYSCALL_COUNT=6.
> - U-23 prompt'unda 4 implementation blocker giderildi:
>   1) Dispatch SYSCALL_TABLE array (match değil)
>   2) WCET_EXIT sabiti + check_wcet_limits 6-element array
>   3) isolate_task mevcut (pub(crate) yap)
>   4) syscall_ids_match_config proof SYS_EXIT line ekle
>
> **v0.6 → v0.7 Değişiklikleri (operational sprint completion gate + quality):**
> - §18 YENİ: SNTM Sprint Completion Gate (8 alt bölüm, kısa operational)
>   - 18.1 Definition of Done (per task type)
>   - 18.2 Required Commands (mevcut U-22 gate + SNTM komutları)
>   - 18.3 Feature Gate Policy (`sntm` + `sntm-safe` umbrella)
>   - 18.4 Negative Test Requirement (mechanical: `coverage.toml` symmetric map)
>   - 18.5 Carry-Forward Template
>   - 18.6 No-Go Conditions (6 spesifik kategori)
>   - **18.7 Proof/Test Quality Gate** (3-yorum kuralı: VERIFIES/CALLS/FAILS-IF,
>     requirement IDs, grandfather list, light tautology detector)
>   - 18.8 Implementation Note (gate script flow)
> - `coverage.toml` YENİ: 12 feature mapped, 7 grandfather isim, requirement
>   ID schema placeholder
> - `scripts/check_coverage.sh` YENİ: symmetry + name existence + 3-yorum
>   quality + requirement traceability + deferred discipline
> - `scripts/check_proof_quality.sh` YENİ: light tautology detector (7 pattern,
>   200 mevcut Kani proof PASS — U-18 audit etkili)
> - `scripts/sntm_sprint_gate.sh` E0/E0b adımları olarak entegre
> - Tek cümle kural: "Bir test/proof, hangi requirement'ı doğruladığını,
>   hangi production fonksiyonunu çağırdığını ve hangi hatalı
>   implementasyonda fail edeceğini söylemiyorsa coverage sayılmaz."
>
> **v0.5 → v0.6 Değişiklikleri (Codex SNTM-SAFE öneri analizi + entegrasyon):**
> - §17 YENİ: SNTM-SAFE — Native Task Security Leap
>   - §17.1 Threat model: arbitrary native binary YOK (build-time certified only)
>   - §17.2 Safe Native Profile (forbid unsafe/alloc/asm/ffi/recursion/dyndispatch)
>   - §17.3 Binary Verifier (forbidden opcode + section + relocation scan)
>   - §17.4 Task Certificate (source/toolchain/manifest/binary hash bundle)
>   - §17.5 Static Local Capability Table (MAC drop, 80× hızlanma, MAC_KEY local
>          attack surface'i eliminate)
>   - §17.6 Typed IPC + Generated sipahi_api (compile-time channel safety)
>   - §17.7 Stack Safety (cargo-call-stack + guard default-on)
>   - §17.8 CFI Roadmap (verifier → Zicfilp/Zicfiss v2.5+ → CHERIoT)
>   - §17.9 Runtime Cost Table (her madde net hız etkisi)
>   - §17.10 CI Gates (8 build-time check matrix)
>   - §17.11 Certificate Mesh — manifest tek source-of-truth, 10 artifact
> - §12 Aşama 3 YENİ: SNTM-SAFE phased rollout v1.6 → v1.9
>   (base SNTM v1.5'i sıkıştırmadan incremental layer)
> - Net analiz sonucu: güvenlik substantial artar + runtime NET HIZLANIR
>   (static cap table cache-miss path 400c → 5c = 80×)
>
> **v0.4 → v0.5 Değişiklikleri (Codex 3. review — implementation gap fix):**
> - §4.5.3 + §4.5.4 YENİ: `sfence.vma zero, zero` PMP reload sonrası
>   (RISC-V Privileged Spec §3.7.2 — CVA6 speculative execution gerekli;
>   mevcut Sipahi v1.0 [pmp.rs:143](src/arch/pmp.rs#L143) **bu fence'i de
>   eksik** → v1.1.1 patch'i SNTM'den bağımsız aciliyet)
> - §4.5.4 YENİ: `PmpProfile` struct + `Region` + `get_pmp_profile()` —
>   v0.3'te sadece çağrılıyordu, tanım yoktu (first-compile gap)
> - §4.8.2 düzeltildi: `target-feature=-relax` codegen-side YETERLI ama
>   `link-arg=--no-relax` linker-side belt+suspenders (her ikisi)
> - §4.8.5 düzeltildi: objcopy `.bin` per-section (text/rodata/data ayrı)
>   — tek `.bin` zero-fill israfı + section hash ayrımı imkansız
> - §4.3 düzeltildi: guard region "opsiyonel" → **default-on** (NAPOT
>   layout'ta implicit, TOR layout'ta explicit no-access region zorunlu);
>   `guard_page = false` için DAL waiver gerekçesi
> - §12 (Aşama 1.5 U-22.5) güncellendi: sprint sırası G4(verify.rs proof
>   remove) → G1(config.rs sabit remove) → G2(sandbox module remove),
>   aksi compile fail (verify.rs COMPUTE_* + WCET_COMPUTE_* referansları)
>
> **v0.3 → v0.4 Değişiklikleri (pre-SNTM coherence):**
> - §11.5 YENİ: WASM Artefakt Temizleme Kararları — tüm WASM-tied
>            item'lar için v1.1 → v1.5 → v2.0 karar matrisi
> - §11.6 YENİ: Compute Services Akıbeti — COMPUTE_COPY/CRC/MAC/MATH
>            kararları (silinecek/taşınacak/kalacak)
> - §11.7 YENİ: WASM Kani Proof Inventory — gate/keep/delete tablosu
> - §12 Aşama 1.5 YENİ: Pre-SNTM Cleanup Sprint (U-22.5, ~3 saat)
>            v1.1.1 tag öncesi temizlik adımları
> - §16 audit trail güncellendi
>
> **v0.2 → v0.3 Değişiklikleri (Codex 2. review):**
> - §5.2 `is_valid_user_ptr`: `region.base + region.size` → `checked_add` defansif
> - §4.5.1 YENİ: PMP Packing Algorithm — static/dynamic entry layout, NAPOT vs TOR
> - §4.5.2 YENİ: PMP Priority Invariant — "static lower, dynamic higher" doktrini
> - §4.5.3 YENİ: Context Switch PMP Reload Atomicity — all-open pencere yok kanıtı
> - §4.8 YENİ: Task ABI Specification — gp/tp, linker, relocation, small-data,
>            panic, global init, build flags
>
> **v0.1 → v0.2 Değişiklikleri (Codex 1. review):**
> - Tock scheduler "cooperative" → "preemptive round-robin" (factual fix)
> - Vanilla PMP kernel-data koruma kararı: "imkansız" → "policy-dependent"
> - is_valid_user_ptr stack-only → multi-region (text/rodata/data/bss/stack/mmio)
> - ELF loader yaklaşımı: kernel-side → build-time flat segments + signature
> - PMP entry budget açık yazıldı (8/16/64 gerçeği, TOR vs NAPOT trade-off)
> - DMA/IOPMP ayrı bölüm: MMIO grant ≠ DMA isolation
> - "formal verified ✓" → "formal-verification friendly ✓" (Kani sınırları)
> - "Safety-critical path ✓" → "Better than WASM interpreter, not certified"
> - Hubris/Tock/Muen/ARINC 653 mimari patternleri açıkça referanslandı
> - 7 katmanlı güvenlik tablosu pratik PMP policy ile yeniden ifade edildi

---

## 1. Motivasyon: Neden WASM Kaldırılıyor?

Sipahi v1.0'da `wasmi` WASM interpreter sandbox olarak eklenmişti.
Audit sonucu (M14, U-22 sprint): production'da çalışmıyor, `.wasm_arena = 0 byte`,
loader syscall yok. ~30K LOC TCB artışı, WCET belirsiz, alloc gerekli —
Sipahi doctrine'ına uymuyor.

```
Sipahi Doctrine          | WASM+wasmi    | SNTM
─────────────────────────────────────────────────────
no_alloc kernel          | wasmi alloc ✗ | alloc yok ✓
<10K LOC TCB             | +30K LOC ✗    | +~300 LOC ✓
Bounded execution        | WCET belirsiz | WCET native ✓
Formal verifiable        | 30K audit ✗   | Kani-friendly ~ (sınırlı)
Determinism > Latency    | Interpreter ✗ | Native CPU ✓
Defense-in-depth         | Yazılım       | Donanım (PMP) ✓
```

**Akademik destek:** DLR'nin 2025 tarihli safety-critical avionics WASM
çalışması ([DLR ELIB 219593](https://elib.dlr.de/219593/)), mevcut WASM
interpreter'larının ED-12C/DO-178C varsayımlarına uymadığını belirtiyor.
Sipahi'nin wasmi'yi production TCB'ye koyması ciddi sertifikasyon yükü
doğururdu — bu yüzden çıkarılıyor.

---

## 2. Literatür Bağlamı — SNTM Hangi Kampta?

SNTM yeni bir paradigma değil; mevcut başarılı modellerden derlenmiş bir kombinasyondur.

### 2.1 Hubris (Oxide Computer) — En Yakın Akraba

Sipahi'nin SNTM yaklaşımına en çok benzeyen sistem. Hubris kernel ~2000 satır
Rust, MPU isolation, **fixed task set at build time**, sıfır dynamic
allocation, runtime task creation/destruction yok. `app.toml` manifest ile
build-time configure.

```
Hubris             SNTM eşdeğeri
────────────────────────────────────────
~2000 LOC kernel   ~10K LOC kernel (Sipahi mevcut)
app.toml           sipahi.toml manifest
MPU per-task       PMP per-task
Fixed task set     Manifest'te tanımlı task'lar
No dynamic alloc   Mevcut Sipahi doctrine
Single image       kernel + tasks tek binary
```

**Kaynak:** [Hubris Reference](https://hubris.oxide.computer/reference/),
[GitHub oxidecomputer/hubris](https://github.com/oxidecomputer/hubris).

**SNTM'in farkı:** Sipahi RISC-V (Hubris ARM Cortex-M öncelik), formal
verification (Kani + TLA+), capability IPC + manifest-defined channels,
DAL-A hedef.

### 2.2 Tock OS — PMP + Preemptive + Grant Memory

**v0.1'deki HATA:** "Tock cooperative scheduling" — yanlış. Tock default
scheduler'ı **preemptive round-robin**'dir. Capsule (kernel-resident)
modülleri cooperative çalışır ama **process'ler preemptive**.

[Tock Book](https://book.tockos.org/doc/overview): "kernel schedules
processes preemptively, processes have stronger system liveness guarantees
than capsules."

```
Tock              SNTM eşdeğeri
─────────────────────────────────────────
Preemptive sched  Sipahi schedule_timer_tick (hâlihazırda preemptive)
RISC-V PMP        Sipahi PMP shadow + per-task NAPOT
Grant memory      SNTM: kernel heap YOK, task'lar kendi static buffer'ları
Capsule isolation Sipahi: capsule yok, hepsi user-mode task
```

**Grant model'in önemi:** Tock'da task kernel'a buffer "grant" eder, kernel
heap kullanmaz. SNTM bu fikri benimser — `ipc_send` task buffer'ından
okur, kernel kendi heap'inde kopya tutmaz. Zero kernel heap.

**Kaynak:** [Tock Design](https://tockos.org/documentation/design/),
[Antmicro VeeR EL2 Tock PMP port](https://antmicro.com/blog/2024/10/support-for-veer-el2-with-user-mode-and-pmp-in-tock-os).

### 2.3 Muen — Static Policy + Generated System Image

Muen (SPARK/Ada, x86) separation kernel mimarisi: **static cyclic scheduling**,
**static resource assignment**, system policy ile generated image. Kernel
small + static = formal verification mümkün.

```
Muen              SNTM benzerliği
─────────────────────────────────────────────────
SPARK/Ada         Rust + Kani (eşdeğer formal yaklaşım)
Static policy     sipahi.toml manifest (build-time)
Generated image   build pipeline kernel + task'ları tek binary üretir
Cyclic schedule   Sipahi fixed-priority + period (eşdeğer doctrine)
VMX root/non-root M-mode/U-mode (RISC-V eşdeğeri)
```

**Kaynak:** [Muen SK](https://muen.codelabs.ch/),
[AdaCore Muen Project](https://www.adacore.com/academia/projects/muen-project).

**SNTM farkı:** Sipahi RISC-V (Muen x86), Rust (Muen SPARK), DAL-A safety
hedef yanında security (capability IPC + audit trail).

### 2.4 ARINC 653 — Avionic Partition Modeli

Spatial + temporal partition. **Major Time Frame (MTF)** sabit cycle.
Her partition fixed temporal window içinde çalışır. Health Monitor (HM)
3 seviyede: process, partition, system.

```
ARINC 653                    SNTM yansıma
────────────────────────────────────────────────────────────
Spatial partition            PMP per-task region
Temporal partition           Fixed-priority preemptive +
                              budget_cycles + period_ticks (mevcut)
Major Time Frame             Sipahi 10ms tick × DEFAULT_PERIOD_TICKS
Partition window             Task budget × period
HM process-level             Watchdog + budget exhausted policy
HM partition-level           Task isolation + restart/degrade
HM system-level              Lockstep + PMP integrity + halt_system
```

SNTM ARINC 653'ü literal olarak implement etmiyor (sertifikasyon ayrı iş)
ama mimari pattern aynı: **statik partitioning + zaman dilimi + sağlık
monitoring**.

**Kaynak:** [ARINC 653 Wikipedia](https://en.wikipedia.org/wiki/ARINC_653),
[Wind River ARINC 653](https://www.windriver.com/solutions/learning/arinc-653-compliant-safety-critical-applications).

### 2.5 seL4 — Capability Microkernel

seL4 farklı lig: capability-tabanlı, deep formal proof (Isabelle/HOL).
SNTM seL4 kadar kanıtlı **değil** — daha küçük, daha statik, Sipahi'nin
bare-metal hedefi için pratik.

```
seL4                         SNTM
──────────────────────────────────────────────────────
Capability microkernel       Capability + manifest-defined IPC
Isabelle/HOL proof           Kani (bounded) + TLA+ (temporal)
Dynamic capability create    Static manifest (build-time)
Untyped memory + retype      Static PMP regions per task
```

**Kaynak:** [seL4 Fact Sheet](https://sel4.org/About/fact-sheet.html).

**SNTM iddiası:** "seL4'ten daha kanıtlı" değil; "seL4'ten daha küçük TCB,
daha statik konfigürasyon, RISC-V bare-metal native."

---

## 3. Alternatif Teknolojiler — Doğru Yer Belirleme

### 3.1 CHERI / CHERIoT (Microsoft + Cambridge)

Donanım capability, byte-granularity. **Geleceğin teknolojisi.**

```
Avantaj:    Spatial + temporal memory safety, compiler-enforced
Dezavantaj: CVA6'da yok, CHERI-aware compiler gerekli
SNTM ile:   v3.0+ hibrit (PMP coarse + CHERI fine)
```

CHERIoT embedded cihazlarda deterministic memory safety + compartmentalization.
([Microsoft CHERIoT](https://www.microsoft.com/en-us/research/publication/cheriot-complete-memory-safety-for-embedded-devices/))

### 3.2 Smepmp (Enhanced PMP) — RISC-V Ratified

Vanilla PMP'nin geliştirilmiş hali. **MML (Machine Mode Lockdown), MMWP
(Machine Mode Whitelist Policy), RLB (Rule Lock Bypass)**.

```
Smepmp         Vanilla PMP'ye katkı
──────────────────────────────────────────────────
MML            Locked rules artık M-mode için de uygulanır
MMWP           M-mode default = deny (eşleşmeyen adres reddedilir)
RLB            Geçici unlock mekanizması (boot-time)
mseccfg CSR    M-mode security configuration
```

**Önemli düzeltme (Codex):** "Vanilla PMP kernel .data koruyamaz" yanlış.
RISC-V PMP spec'e göre U-mode için eşleşmeyen adres reddedilir
([RISC-V Privileged Spec](https://riscv.github.io/riscv-isa-manual/snapshot/privileged/)).
Sorun: eğer Sipahi all-RAM RW PMP entry açıyorsa U-mode kernel data okuyabilir
— bu **policy bug**, donanım sınırı değil.

**SNTM doğru policy:**
- U-mode için global all-RAM entry **YOK**
- Her task: text RX + rodata R + data RW + stack RW + mmio R/W
- Kernel `.text/.rodata/.data/.bss` **U-mode'a hiç grant edilmez**
- M-mode L=0 entry'lerde zaten erişebilir (manuel override)

Smepmp'nin değeri: M-mode bug'larında ek lockdown. Vanilla PMP doğru
policy ile zaten kernel-data izolasyon sağlar.

### 3.3 RISC-V Worlds, SmMTT — Geleceğin Sistem Seviyesi İzolasyon

Worlds (system-wide isolation, draft 2025) ve SmMTT (S-mode supervisor
domain protection). CVA6'da yok, M-mode kernel için Worlds özellikle
ilgili. v3.0+ aday.

### 3.4 SFI/LFI — Software Fault Isolation

LFI (ASPLOS 2024) ARM64'te ~%7 overhead. RISC-V için olgunluğu düşük.
**SNTM'in alternatifi değil, opsiyonel ek katman.**
([ASPLOS 2024](https://www.asplos-conference.org/asplos2024/main-program/abstracts/index.html))

### 3.5 IOPMP — DMA Bus Master Koruması

**Codex'in eksik bıraktığı kritik nokta.** PMP CPU bus master'lar için.
DMA-capable cihazlar (örn. ethernet, USB controller) bypass edebilir.
Çözüm: IOPMP — non-CPU bus master'lar için PMP eşdeğeri.

```
PMP koruma kapsamı:    CPU → memory
IOPMP koruma kapsamı:  DMA controller, USB, GPU → memory

Sipahi'de:
  v1.0/v1.5: IOPMP yok — DMA-capable cihaz YOKsa OK
  v2.0+: AMCI ile birlikte IOPMP zorunlu
  v3.0+: SoC seviyesi enforcement
```

**Spec:** [riscv-non-isa/iopmp-spec](https://github.com/riscv-non-isa/iopmp-spec)
v1.0.0-draft5+, SystemVerilog RTL hazır.

### 3.6 Karşılaştırma Tablosu (v0.2 — Düzeltilmiş)

```
                    CHERI  Worlds SmMTT Smepmp Tock  SFI   IOPMP SNTM
─────────────────────────────────────────────────────────────────────────
CVA6'da mevcut       ✗     ✗      ✗     ✗      ✗    ✗     ✗     ✓ (PMP)
Donanım izolasyon    ✓     ✓      ✓     ✓      ✓    ✗     ✓     ✓ (PMP)
Native hız           ✓     ✓      ✓     ✓      ✓    ~     ✓     ✓
WCET analyzable      ~     ~      ~     ~      ✓    ✓     ✓     ✓
no_alloc uyumlu      ~     ~      ~     ~      ~    ✓     N/A   ✓
Preemptive           ✓     ✓      ✓     ✓      ✓ ★  ✓     N/A   ✓
DMA isolation        ✗     ~      ✗     ✗      ✗    ✗     ✓ ★   ~ (manifest)
Formal-verif friendly~     ~      ~     ✓      ~    ~     ~     ✓ (Kani)
                                                                  (sınırlı)
Bare-metal           ✓     ✓      ~     ✓      ✓    ✓     ✓     ✓
Rust native          ~     ~      ~     ~      ✓    ~     N/A   ✓
Better than WASM     ✓     ✓      ✓     ✓      ✓    ✓     ✓     ✓
Safety-cert ready    ✗     ✗      ✗     ✗      ✗    ✗     ✗     ✗ ★★
Ek LOC               ~500  ~300   ~300  ~100   N/A  ~2K   ~200  ~300

★ = v0.1'den düzeltilen
★★ = "ready"DEĞİL ama "WASM'dan daha uygun"
```

**Sonuç:** CVA6 vanilla PMP donanımında SNTM bugün uygulanabilir. CHERI/
Worlds/SmMTT geldiğinde **üstüne** eklenebilir. SFI/LFI alternatif değil,
opsiyonel ek. IOPMP DMA için **tamamlayıcı** (eklenebilir, gerekirse).

---

## 4. SNTM Mimari Tasarım (v0.2 — Multi-Region Aware)

### 4.1 Temel Prensip

> "Interpreter yok. Her task native RISC-V binary. PMP donanım izolasyon
> sağlar. Kernel ELF parse etmez — build-time flat segment + signature."

```
ESKİ (WASM):
  .wasm bytecode → wasmi interpreter (30K LOC) → sandbox
  Yazılım koruma, WCET belirsiz, alloc gerekli

YENİ (SNTM):
  task source → cargo build → ELF
  → host build-tool: ELF segment extraction + manifest validation
  → flat signed image
  → kernel: bounded copy + PMP setup + run
  
  Kernel ELF bilmez. Sadece signed flat segment table'ı kopyalar.
```

### 4.2 Task Yapısı (v0.2 — Multi-Region)

Her task ayrı Cargo crate olarak derlenir. **v0.2 değişiklik: task'lar
artık çoklu memory region kullanır, tek "stack range" değil.**

```
sipahi/
├── kernel/                  # Sipahi kernel (mevcut src/)
├── tasks/
│   ├── task_sensor/
│   │   ├── Cargo.toml
│   │   └── src/main.rs      # #![no_std] #![no_main]
│   ├── task_actuator/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── task_monitor/
│       ├── Cargo.toml
│       └── src/main.rs
├── tools/
│   ├── sntm-pack/           # Host tool: ELF → flat segments
│   └── sntm-validate/       # Manifest validator (PMP budget vb.)
└── sipahi.toml              # Manifest
```

### 4.3 Task Memory Region Modeli (YENİ — v0.2)

Her task **6 region**'a kadar kullanabilir:

```
Region    Permission   Açıklama                        PMP entry
────────────────────────────────────────────────────────────────────
text      RX           Task code (read-only execute)   1× TOR
rodata    R            Read-only data (const'lar)      1× TOR
data      RW           Initialized writable data       1× TOR (data+bss
                                                        bitişik ise)
bss       RW           Zero-initialized writable        (yukarıdaki ile
                                                        bitişik)
stack     RW           Task stack (8KB NAPOT default)  1× NAPOT
mmio      R / RW       Device-specific MMIO            1× TOR (gerekirse)
guard     N (no acc)   Stack underflow detector        DEFAULT-ON (v0.5)
```

**Guard region policy (v0.5 — Codex 3. review):**

DAL-A için stack underflow/overflow tespit edilebilir olmalı, "opsiyonel"
posture yanlış. v0.5 default-on policy:

- **NAPOT stack layout** (size power-of-2, base aligned): NAPOT bound
  zaten implicit guard sağlar — stack region dışındaki erişim NO_MATCH
  → kernel TOR ile shadowed olmadığı sürece deny. **Manifest'te ayrı
  guard region GEREKMEZ**, `guard_implicit = true` (default).
- **TOR stack layout** (NAPOT-uyumsuz size/alignment): explicit
  no-access region zorunlu — stack altına bitişik 4KB+ TOR perm=NONE
  region eklenir. `guard_explicit = true` (TOR ise default).
- **`guard_page = false` waiver**: yalnızca manifest'te açık
  `dal_level = "B"` veya altı + waiver_reason metni varsa kabul edilir.
  amci-validate tool DAL-A + guard_page=false kombinasyonunu reject eder.

```toml
# Manifest örneği — implicit guard (NAPOT stack)
[tasks.task_a]
stack_size = 8192      # power-of-2 → NAPOT → implicit guard

# Manifest örneği — explicit guard (TOR stack)
[tasks.task_b]
stack_size = 6144      # 6KB → NAPOT-uyumsuz → TOR
guard_page = true      # zorunlu, 4KB no-access region eklenir

# Manifest örneği — waiver (DAL-A için reject olur)
[tasks.task_c]
stack_size = 6144
guard_page = false
dal_level   = "B"
waiver_reason = "memory budget constraint, runtime monitor compensates"
```

**Tipik task PMP profili:** 4-5 entry (TOR text + TOR rodata + TOR data+bss
+ NAPOT stack + opsiyonel TOR mmio + opsiyonel TOR guard for TOR-stack).

### 4.4 Manifest (sipahi.toml v0.2)

```toml
[kernel]
binary = "target/riscv64imac-unknown-none-elf/release/sipahi"
stack_size = 8192
secure_boot = true

# v0.2: PMP budget açık
[platform]
pmp_entries = 16              # CVA6 default; QEMU virt 16
                              # Eğer hedef silikon 8 ise validate fail

[[task]]
name = "task_sensor"
binary = "target/tasks/task_sensor.elf"
priority = 4
budget_cycles = 400_000
period_ticks = 10
dal = "A"
stack_size = 4096

# v0.2: multi-region PMP layout
[task.text]
base = "0x80100000"
size = "16K"
perm = "RX"

[task.rodata]
base = "0x80104000"
size = "4K"
perm = "R"

[task.data]
base = "0x80105000"
size = "4K"
perm = "RW"
# .bss data ile bitişik — tek PMP entry yeterli

[task.stack]
base = "0x80106000"
size = "4K"
perm = "RW"
napot = true                  # NAPOT mode, tek entry

[task.mmio]
base = "0x40000000"
size = "4K"
perm = "RW"                   # Sensor MMIO

# v0.2: DMA capability ayrı declaration
[task.dma]
enabled = false               # Bu task DMA yapmıyor
# Eğer enabled = true olsaydı:
# iopmp_required = true       # IOPMP olmadan reddedilir

[[task]]
name = "task_actuator"
binary = "target/tasks/task_actuator.elf"
priority = 2
budget_cycles = 200_000
period_ticks = 10
dal = "A"
stack_size = 4096

[task.text]
base = "0x80110000"
size = "16K"
perm = "RX"

[task.rodata]
base = "0x80114000"
size = "4K"
perm = "R"

[task.data]
base = "0x80115000"
size = "4K"
perm = "RW"

[task.stack]
base = "0x80116000"
size = "4K"
perm = "RW"
napot = true

[[channel]]
id = 0
producer = "task_sensor"
consumer = "task_actuator"
slots = 16
msg_size = 64
```

### 4.5 PMP Entry Budget — Açık Hesap (YENİ — v0.2)

**Codex'in haklı kritiği:** PMP entry sayısı sınırlı, manifest validator
budget kontrol etmeli.

```
Tipik CVA6 / SiFive PMP entry sayıları:
  SiFive E31: 8 entry
  RocketChip default: 8 entry  
  QEMU virt: 16 entry
  Spec maksimum: 64 entry
  
Sipahi v1.0 mevcut kullanım:
  Entry 0-1: text RX (TOR)            2 entries
  Entry 2-3: rodata R (TOR)           2 entries
  Entry 4-5: data+bss+stack RW (TOR)  2 entries
  Entry 6-7: UART MMIO RW (TOR)       2 entries (production'da KAPALI)
  Entry 8:   per-task NAPOT           1 entry (context switch'te değişir)
  ─────────────────────────────────────────────────
  Total fixed: 8 entries
  Production (no UART): 6 entries fixed + 1 dynamic = 7 entries

SNTM v0.2 hedef kullanım:
  Static kernel regions:              6 entries (text, rodata, kernel data)
  Per-task dynamic profile:           5 entries (text, rodata, data, stack,
                                                  optional mmio)
                                                  
  Context switch'te:                   Per-task 5 entry yeniden config edilir
  Toplam aktif:                        6 (kernel) + 5 (active task) = 11
  
  16-entry PMP'de:                     ✓ FİT (5 entry margin)
  8-entry PMP'de:                      ✗ FİT ETMEZ (manifest validator reddet)
```

**Manifest validator kuralı:**

```rust
// tools/sntm-validate/src/main.rs (özet)
const KERNEL_PMP_ENTRIES: usize = 6;

fn validate_pmp_budget(manifest: &Manifest) -> Result<(), Error> {
    let max_per_task = manifest.tasks.iter().map(|t| t.required_pmp_entries()).max();
    let total_active = KERNEL_PMP_ENTRIES + max_per_task.unwrap_or(0);
    
    if total_active > manifest.platform.pmp_entries {
        return Err(Error::PmpBudgetExceeded {
            required: total_active,
            available: manifest.platform.pmp_entries,
        });
    }
    Ok(())
}

impl TaskManifest {
    fn required_pmp_entries(&self) -> usize {
        let mut count = 0;
        // text TOR: alt sınır + üst sınır = 2 entry... AMA kernel zaten
        // alt sınır verdiyse 1 entry (TOR ardışık entries'i kullanır)
        // Practical: 1 entry per region (NAPOT) veya 2 (TOR)
        if self.text.napot { count += 1 } else { count += 2 }
        if self.rodata.napot { count += 1 } else { count += 2 }
        if self.data.napot { count += 1 } else { count += 2 }
        if self.stack.napot { count += 1 } else { count += 2 }
        if let Some(_) = self.mmio { 
            if self.mmio.unwrap().napot { count += 1 } else { count += 2 }
        }
        count
    }
}
```

**NAPOT vs TOR trade-off:**
- **NAPOT:** Tek entry, gerek alignment power-of-2, gerek size power-of-2
- **TOR:** İki entry (alt+üst), arbitrary boundary

Sipahi mevcut: text/rodata/data TOR (linker section bound), stack NAPOT
(8KB compile-time aligned). SNTM bu pattern'ı task'lara uygular.

### 4.5.1 PMP Packing Algorithm (YENİ — v0.3)

Codex review: "tipik task 4-5 entry" karışık ifade. Net algoritma:

```
ENTRY LAYOUT — Static (kernel) vs Dynamic (per-task)
─────────────────────────────────────────────────────────
Entry 0      = OFF (kernel.text alt sınır)
Entry 1      = TOR RX + LOCK   → kernel.text
Entry 2      = OFF (kernel.rodata alt sınır)
Entry 3      = TOR R + LOCK    → kernel.rodata
Entry 4      = OFF (kernel.data alt sınır)
Entry 5      = TOR RW + LOCK   → kernel.data + .bss + kernel_stack
─────────────────────────────────────────────────────────
TOTAL STATIC: 6 entries (0-5), L-bit ile boot'ta KİLİTLİ

Entry 6..N   = DYNAMIC per-task PMP profile
               Context switch'te tamamı yeniden yazılır
─────────────────────────────────────────────────────────
DYNAMIC: 5 region × (1 NAPOT veya 2 TOR) = 5-10 entry
```

**NAPOT Decision Tree (Manifest Validator):**

```rust
// tools/sntm-validate/src/pmp_pack.rs

#[derive(Clone, Copy)]
pub enum PmpEncoding {
    Napot { addr: usize, size_log2: u8 },  // 1 entry
    Tor   { lo: usize, hi: usize },        // 2 entries (lo=OFF, hi=TOR)
}

pub fn pack_region(base: usize, size: usize) -> Result<PmpEncoding, PmpError> {
    // Şart 1: NAPOT için size power-of-2 ≥ 8 byte
    let size_pow2 = size.is_power_of_two() && size >= 8;
    
    // Şart 2: NAPOT için base, size'a aligned
    let aligned = base & (size - 1) == 0;
    
    if size_pow2 && aligned {
        // NAPOT: tek entry
        // pmpaddr = (base >> 2) | ((size >> 3) - 1)
        let size_log2 = size.trailing_zeros() as u8;
        Ok(PmpEncoding::Napot { addr: base, size_log2 })
    } else {
        // TOR fallback: iki entry
        let hi = base.checked_add(size).ok_or(PmpError::Overflow)?;
        Ok(PmpEncoding::Tor { lo: base, hi })
    }
}

pub fn count_entries(regions: &[Region]) -> usize {
    regions.iter().map(|r| match pack_region(r.base, r.size) {
        Ok(PmpEncoding::Napot { .. }) => 1,
        Ok(PmpEncoding::Tor   { .. }) => 2,
        Err(_) => 0,  // validator zaten reject etmiş olur
    }).sum()
}
```

**Per-Region NAPOT Olabilir Mi? (Tipik Sipahi):**

```
Region            Size       Power-of-2?    Aligned?    Encoding
─────────────────────────────────────────────────────────────────
text              16K        ✓              ✓ (linker)  NAPOT (1)
rodata            4K         ✓              ✓ (linker)  NAPOT (1)
data + bss        8K         ✓              ✓ (linker)  NAPOT (1)
stack             8K         ✓              ✓ (8K aln)  NAPOT (1)
mmio              4K         ✓              ✓           NAPOT (1)
                                                        ─────────
                                                        TOTAL: 5
```

**Eğer task layout NAPOT-uyumlu değilse** (ör. text 17KB, alignment
yanlış), validator TOR'a düşer:

```
Worst case TOR: 5 region × 2 entry = 10 entry
→ Static (6) + Dynamic TOR (10) = 16 entries
→ 16-entry PMP'de: SIĞAR (tam dolu)
→ 8-entry PMP'de:  SIĞMAZ → manifest validator FAIL
```

**Manifest validator'ın decision flow:**

```
For each task:
  1. Her region için pack_region() çağır
  2. Toplam entry sayısı hesapla
  3. Her region NAPOT vs TOR sonucunu cache'le
  4. Tüm task'lar için max(per_task_entries) bul
  5. KERNEL_PMP_ENTRIES (6) + max_per_task ≤ platform.pmp_entries?
     - EVET → manifest valid
     - HAYIR → fail with: "PMP budget exceeded: required N, available M"
```

### 4.5.2 PMP Priority Invariant (YENİ — v0.3 — KRİTİK)

**Codex review (kritik bulgu):** RISC-V PMP "first match wins" — düşük
indeks öncelikli. Eğer task entry kernel entry'den DÜŞÜK indekste olsa,
task kendi grant'ı ile kernel adres uzayını **shadow edebilir**.

```
SHADOW ATTACK SCENARIO (yanlış implementasyon):
─────────────────────────────────────────────────────
Entry 5: kernel.data RW + LOCK    (kernel'in koruması)
Entry 1: task.text   RX           (task'a grant edilen, ama düşük
                                    indekste yanlışlıkla)

Task program counter kernel.data adresine atlasa:
  → Önce Entry 1 kontrol edilir
  → Eğer task.text region kernel.data adresini KAPSIYOR ise (örn.
    overlap manifest validator yakalamadıysa)
  → Entry 1 RX permission döner → KERNEL CODE EXECUTE!
  → İzolasyon kırıldı

Doğrusu:
─────────────────────────────────────────────────────
Entry 0-5: kernel (static, lock'lu, EN ÖNCELİKLİ)
Entry 6+:  task (dynamic, kernel'i shadow EDEMEZ)
```

**INVARIANT:**

```
∀ task t, ∀ static_entry s, ∀ dynamic_entry d:
  index(s) < index(d)
  
  AND
  
  ∀ task region r in t's PMP profile:
    r.address_range ∩ kernel.address_range = ∅
```

İlk koşul **layout invariant'ı** (compile-time, manifest validator).
İkinci koşul **overlap invariant'ı** (compile-time + runtime).

**Manifest Validator Priority Check:**

```rust
// tools/sntm-validate/src/priority_check.rs

pub const KERNEL_STATIC_ENTRIES: usize = 6;  // Entry 0-5

pub fn validate_priority(manifest: &Manifest) -> Result<(), Error> {
    // Kontrol 1: Static kernel entry sayısı 6 sabit
    if KERNEL_STATIC_ENTRIES != 6 {
        return Err(Error::InvalidStaticEntryCount);
    }
    
    // Kontrol 2: Task region'ları kernel adres uzayıyla çakışmamalı
    let kernel_ranges = [
        (manifest.kernel.text.base, manifest.kernel.text.size),
        (manifest.kernel.rodata.base, manifest.kernel.rodata.size),
        (manifest.kernel.data.base, manifest.kernel.data.size),
    ];
    
    for task in &manifest.tasks {
        for region in task.regions() {
            for (k_base, k_size) in &kernel_ranges {
                if regions_overlap(region.base, region.size, *k_base, *k_size) {
                    return Err(Error::TaskKernelOverlap {
                        task: task.name.clone(),
                        region: region.kind,
                    });
                }
            }
        }
    }
    
    // Kontrol 3: Task'lar arası overlap yok (defense-in-depth)
    for (i, t1) in manifest.tasks.iter().enumerate() {
        for t2 in &manifest.tasks[i+1..] {
            for r1 in t1.regions() {
                for r2 in t2.regions() {
                    if regions_overlap(r1.base, r1.size, r2.base, r2.size) {
                        return Err(Error::CrossTaskOverlap { ... });
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn regions_overlap(a_base: usize, a_size: usize, b_base: usize, b_size: usize) -> bool {
    let a_end = a_base.saturating_add(a_size);
    let b_end = b_base.saturating_add(b_size);
    !(a_end <= b_base || b_end <= a_base)
}
```

**Runtime Doğrulama (Defansif):**

```rust
// scheduler/mod.rs context switch öncesi (debug build):

#[cfg(debug_assertions)]
fn assert_pmp_priority_invariant() {
    // PMP Entry 0-5 lock'lu olmalı
    let cfg = pmp::read_pmpcfg0();
    let mut i = 0;
    while i < KERNEL_STATIC_ENTRIES {
        let cfg_byte = ((cfg >> (i * 8)) & 0xFF) as u8;
        let locked = cfg_byte & PMP_L != 0;
        debug_assert!(locked, "Kernel PMP entry {} unlocked!", i);
        i += 1;
    }
}
```

### 4.5.3 Context Switch PMP Reload Atomicity (YENİ — v0.3)

**Codex review:** PMP profile reload sırasında "all-open pencere" yok
olduğu **invariant olarak açıkça yazılmalı**. Aksi: gelecekteki
implementer "burada interrupt enable etsek?" diye düşünebilir.

```
INVARIANT — Context Switch PMP Reload:
─────────────────────────────────────────────────────
Eski task profile dynamics → Deny stage → Yeni task profile dynamics

Bu sequence'de:
  1. MIE = 0 (interrupt disabled — trap context girişinde set edilmiş)
  2. U-mode running = false (M-mode kernel handler içindeyiz)
  3. Sadece M-mode CSR write yapılıyor
  4. Static kernel entry'leri (0-5) DEĞİŞMİYOR (lock'lu)
  
Sonuç: U-mode perspective'inden hiç all-open pencere YOK.
       Race condition imkansız çünkü sadece tek bir actor (M-mode)
       bu sequence sırasında çalışıyor.
```

**Reload Sequence (Kod):**

```rust
// scheduler/mod.rs schedule_timer_tick context switch:

#[cfg(not(kani))]
unsafe fn reload_pmp_profile(profile: &PmpProfile) {
    // SAFETY: Trap context, MIE=0, single-hart, no concurrent access.
    // Static entries (0-5) lock'lu — bu fonksiyon sadece dynamic
    // entries'i (6+) yazar.
    
    // Stage 1: Eski dynamic entry'leri DENY yap
    // (boş PMP entry = deny U-mode for matched address)
    let mut i = KERNEL_STATIC_ENTRIES;
    while i < MAX_PMP_ENTRIES {
        pmp::write_pmpcfg_entry(i, 0);  // OFF mode = no match = U-deny
        i += 1;
    }
    
    // Stage 2: Yeni profile'i sırayla yaz
    // pmpaddr önce, sonra pmpcfg (perm aktif olur)
    let mut entry_idx = KERNEL_STATIC_ENTRIES;
    let mut region_idx = 0;
    while region_idx < profile.region_count {
        let region = &profile.regions[region_idx];
        match region.encoding {
            PmpEncoding::Napot { addr, size_log2 } => {
                let napot_val = (addr >> 2) | ((1usize << (size_log2 - 3)) - 1);
                pmp::write_pmpaddr(entry_idx, napot_val);
                pmp::write_pmpcfg_entry(entry_idx, region.perm | PMP_NAPOT);
                entry_idx += 1;
            }
            PmpEncoding::Tor { lo, hi } => {
                // Entry N: OFF, address = lo
                pmp::write_pmpaddr(entry_idx, lo >> 2);
                pmp::write_pmpcfg_entry(entry_idx, 0);  // OFF
                entry_idx += 1;
                // Entry N+1: TOR, address = hi
                pmp::write_pmpaddr(entry_idx, hi >> 2);
                pmp::write_pmpcfg_entry(entry_idx, region.perm | PMP_TOR);
                entry_idx += 1;
            }
        }
        region_idx += 1;
    }

    // v0.5 SPEC COMPLIANCE FIX (Codex 3. review):
    // RISC-V Privileged Spec §3.7.2 — "PMP CSR writes have no defined
    // ordering with respect to subsequent memory accesses until an
    // explicit fence."
    //
    // QEMU sessiz geçer (TCG modu PMP fence enforce etmiyor). CVA6 ve
    // gerçek RISC-V çekirdekleri speculative execution + memory pipeline
    // var; fence olmadan U-mode geri dönüşte eski PMP değerleri kısa
    // pencerede geçerli kalabilir → izolasyon ihlali.
    //
    // SFENCE.VMA rs1=x0, rs2=x0: tüm address translation cache flush.
    // RISC-V'de hem virtual memory hem PMP için aynı barrier.
    unsafe { core::arch::asm!("sfence.vma zero, zero"); }

    // Shadow update (debug_assert MIE=0 yukarıda)
    update_pmp_shadow_dynamic(profile);
}
```

**⚠️ Mevcut Sipahi v1.0 Aciliyet (v0.5 — Codex 3. review):**

`src/arch/pmp.rs::write_per_task_napot` (line 143) ve U-22'de eklediğim
PMP shadow update path'i de sfence.vma **eksik**. SNTM'den bağımsız,
v1.1.1 patch'i olarak öncelikli düzeltme:

```rust
// src/arch/pmp.rs:143 — mevcut implementasyona ekleme
pub fn write_per_task_napot(napot_addr: usize, cfg_val: usize) {
    unsafe {
        asm!("csrw pmpcfg2, zero");
        asm!("csrw pmpaddr8, {}", in(reg) napot_addr);
        asm!("csrw pmpcfg2, {}", in(reg) cfg_val);
        asm!("sfence.vma zero, zero");  // ← v1.1.1 EKLENECEK
    }
}
```

**TLA+ Spec Referansı:**

`Tla+/SipahiSNTM.tla` (yeni, v1.5 sprint'inde yazılacak):

```tla
\* PMP profile reload atomicity invariant:
ATOMIC_RELOAD ==
    /\ MIE = FALSE
    /\ Mode = "M"
    /\ \E sequence: 
         /\ DenyStage(sequence)
         /\ NewProfileWrite(sequence)
       => /\ NoUserModeExecution(sequence)
          /\ NoOtherHartAccess(sequence)
```

**Neden DENY stage gerekli:**

Eğer sadece "yeni profile yaz" yaparsan:
```
Entry 6: eski_task.stack RW
Entry 7: eski_task.text RX
...
[New profile yaz]
Entry 6: new_task.text RX  ← yeni
Entry 7: eski_task.text RX ← hala eski!  
```

Aralarda yeni task entry 6'da ama eski task entry 7-10'da kalır.
**Bu yeni task'ın eski task region'larına erişebilmesine sebep olur.**

DENY stage:
```
Stage 1: Tüm dynamic entries OFF (deny)
Stage 2: Yeni profile sırayla yaz
→ Aralarda hiç eski profile + hiç yeni profile karışımı YOK
```

**Critical:** Bu sequence MIE=0 + M-mode altında atomic. Eğer
gelecekteki implementer "her entry'i tek tek update edelim, deny
stage'e gerek yok" derse → izolasyon kırılır. INVARIANT yazımı bu
hatayı önler.

### 4.5.4 Type Definitions: PmpProfile + Region + get_pmp_profile (YENİ — v0.5)

**Codex 3. review:** v0.3'te `&PmpProfile`, `get_pmp_profile()`,
`region.encoding`, `region.perm` ifadeleri çağrılıyor ama tanımlar
yoktu — ilk compile fail. Aşağıdaki tipler `src/kernel/memory/mod.rs`
veya yeni `src/kernel/pmp/profile.rs`'e eklenir.

```rust
// src/kernel/pmp/profile.rs

use crate::arch::pmp::PmpEncoding;
use crate::common::config::MAX_TASKS;

/// PMP region permissions — read/write/execute bitleri.
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
    pub const NONE: Self = Self { r: false, w: false, x: false }; // guard
}

/// Tek bir region — task'a grant edilen tek PMP entry (NAPOT) veya
/// entry-çifti (TOR).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Region {
    pub base:     usize,        // RAM address (manifest'ten)
    pub size:     usize,        // byte (NAPOT için power-of-2)
    pub encoding: PmpEncoding,  // §4.5.1: Napot{addr,size_log2} | Tor{lo,hi}
    pub perm:     Permission,
}

/// Task'ın tam PMP profili — max 6 region (text/rodata/data/stack/mmio
/// /guard). region_count actual sayı, regions[0..region_count] valid.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PmpProfile {
    pub region_count: u8,
    pub regions:      [Region; 6],
}

/// Build-time const — manifest'ten sntm-validate üretir.
/// Boot sırasında değişmez (read-only). Task ID = array index.
pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] = generated::PMP_PROFILES;

/// Caller task ID'ye göre PMP profile lookup.
///
/// Returns:
///   Some(&'static PmpProfile) — geçerli task ID, profile var
///   None                       — out-of-bounds veya Isolated/Dead task
///
/// SAFETY: PMP_PROFILES build-time const, immutable static.
/// task_id manifest tarafından assign edilir — sıralı 0..MAX_TASKS.
#[inline]
#[must_use = "PMP profile lookup result must be checked"]
pub fn get_pmp_profile(task_id: u8) -> Option<&'static PmpProfile> {
    let idx = task_id as usize;
    if idx >= MAX_TASKS {
        return None;
    }
    // task_state Dead/Isolated/Init ise None döner (defansif):
    let state = crate::kernel::scheduler::query_task_state(task_id);
    if !state.is_runnable() {
        return None;
    }
    Some(&PMP_PROFILES[idx])
}
```

**Manifest'ten generated tablo örneği** (sntm-validate çıktısı):

```rust
// src/kernel/pmp/generated.rs — sntm-validate tool tarafından üretilir
pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] = [
    // Task 0 — task_a (manifest sipahi.toml [tasks.task_a])
    PmpProfile {
        region_count: 5,
        regions: [
            Region { base: 0x8010_0000, size: 0x4000, // 16K
                     encoding: PmpEncoding::Napot { addr: 0x8010_0000, size_log2: 14 },
                     perm: Permission::RX },          // text
            Region { base: 0x8010_4000, size: 0x1000, // 4K
                     encoding: PmpEncoding::Napot { addr: 0x8010_4000, size_log2: 12 },
                     perm: Permission::R },           // rodata
            Region { base: 0x8010_5000, size: 0x1000, // 4K
                     encoding: PmpEncoding::Napot { addr: 0x8010_5000, size_log2: 12 },
                     perm: Permission::RW },          // data + bss
            Region { base: 0x8011_0000, size: 0x2000, // 8K
                     encoding: PmpEncoding::Napot { addr: 0x8011_0000, size_log2: 13 },
                     perm: Permission::RW },          // stack
            Region { base: 0x1000_1000, size: 0x1000, // 4K MMIO
                     encoding: PmpEncoding::Napot { addr: 0x1000_1000, size_log2: 12 },
                     perm: Permission::RW },          // UART (gerekiyorsa)
            Region { base: 0, size: 0,
                     encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
                     perm: Permission::NONE },        // boş slot
        ],
    },
    // ... diğer task'lar ...
];
```

**Test edilebilirlik:** `PMP_PROFILES` const static → Kani harness'leri
bu tabloyu doğrudan inspect edebilir (region_count bounded ≤ 6, overlap
yok, perm valid kombinasyon vb. invariant'lar).

### 4.6 Build Pipeline (v0.2 — Kernel ELF Parse Etmez)

```
Host build-time (sntm-pack tool):
─────────────────────────────────────────
  task_sensor.elf
    ↓ riscv64-linux-gnu-objcopy
  task_sensor.bin (flat segments)
    ↓
  manifest validator
    - PMP budget check
    - Region overlap check
    - Alignment check (NAPOT power-of-2)
    - DMA + IOPMP requirement check
    ↓
  generated Rust const tables:
    const TASK_TABLE: [TaskInfo; N] = [...];
    const PMP_PROFILES: [PmpProfile; N] = [...];
    const CHANNEL_TABLE: [ChannelInfo; M] = [...];
    ↓
  cargo build (kernel + const tables)
    ↓
  signed Sipahi image:
    [kernel.bin][task_table][task1.bin][task2.bin]...[ed25519.sig]


Kernel boot-time:
─────────────────────────────────────────
  1. Verify ed25519 signature (FAIL → halt)
  2. Copy task segments to PMP-targeted RAM addresses
     (sntm-pack guarantees alignment + bounds)
  3. Zero .bss for each task
  4. Set up kernel base PMP entries
  5. Configure first task's per-task PMP profile
  6. Run scheduler

Kernel asla ELF parse ETMEZ.
Kernel asla TOML parse ETMEZ.
Kernel asla relocation çözmez.
Kernel asla dynamic allocation yapmaz.
```

**Bu yaklaşımın avantajları:**
- Kernel TCB küçük kalır (ELF parser ~5K LOC olurdu)
- Build-time validation: PMP budget, alignment, overlap → compile fail
- Signed image: tampering boot-time detect edilir
- Kani-friendly: kernel kodu sadece bounded copy + PMP setup

### 4.7 Boot Sequence (v0.2)

```
1. Secure boot:
   ed25519 verify (image signature)
   FAIL → halt_system

2. Kernel init:
   UART, CLINT, kernel PMP base regions

3. Production POST (mevcut U-21 fix):
   mtvec, mtime, misa, medeleg/mideleg, mcounteren, PMP integrity
   FAIL → halt_system

4. Task load:
   For each task in manifest:
     - Bounded copy task.bin → task.text region
     - Zero task.bss region
     - Set up TaskContext (sp, mepc, mstatus.MPP=U)

5. Per-task PMP profile compute (build-time const'lar):
   PMP_PROFILES[N] tablosu boot'ta zaten compile-time hazır
   Context switch'te aktif profile load edilir

6. Channel assign + seal:
   For each channel: assign producer/consumer task ID
   ipc::seal_channels() — sonradan değişmez

7. start_first_task:
   En yüksek priority Ready task → mret to U-mode
```

### 4.8 Task ABI Specification (YENİ — v0.3)

**Codex review (kritik):** "_start, sp, mepc yeterli değil — gp/tp,
linker, relocation, small-data, panic, global init eksik. Boot etmeyen
task'ların klasik sebebi bu detaylar."

Aşağıdaki ABI tüm SNTM task'ları için **zorunlu**.

#### 4.8.1 Register Conventions (Boot Time)

Kernel `mret` öncesi register state:

```
Register   Değer                          Set eden
──────────────────────────────────────────────────────────
mepc       task.entry (manifest'ten)     Kernel
mstatus    MPP=U (00), MPIE=1            Kernel
sp         task.stack.base + size - 16   Kernel
gp         0                              Kernel (task yeniden set ederse)
tp         0                              Kernel (task TLS yok)
ra         0                              Kernel (start_first_task scrub)
a0..a7     0                              Kernel (task argümansız)
t0..t6     0                              Kernel (info-leak engelleme)
s0..s11    0                              Kernel (start_first_task scrub)
mscratch   __stack_top (kernel_sp)        Kernel
```

**`gp` (Global Pointer) Politikası:**

Sipahi SNTM **small-data optimization KULLANMAZ**. Sebep:
- gp-relative addressing gerekirse boot'ta gp setup gerekir
- Manifest'ten task'a gp adresini geçirmek extra ABI complexity
- RISC-V `linker relaxation` gp-rel offset'lerini optimize ederken
  PMP boundary'leri shift edebilir

**Çözüm:** Task'lar **belt+suspenders linker relaxation kapalı** derlenir
(v0.5 — Codex 3. review):
- `-C target-feature=-relax` → codegen-side: relax marker emit etme
- `-C link-arg=--no-relax` → linker-side: marker olsa bile relax yapma
- `-C link-arg=-G0` → small-data section yok → gp setup gerekmez

`rustc --print=target-features --target=riscv64imac-unknown-none-elf`
çıktısında `relax — Enable Linker relaxation` listede. Yalnız bu flag
gelecekteki LLVM sürümünde davranış değiştirirse `--no-relax` linker
side yedek savunma. İkisinin birlikte kullanımı standart embedded
RISC-V Rust pratiği (esp32-hal, hifive1-hal vb. örnekler).

#### 4.8.2 Cargo Project Template

`tasks/task_template/Cargo.toml`:

```toml
[package]
name = "task_template"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "task_template"
path = "src/main.rs"

[dependencies]
sipahi-api = { path = "../../sipahi-api" }

[profile.release]
opt-level = "s"           # boyut hedefli (PMP region budget'a uy)
lto = true                # link-time optimization
codegen-units = 1         # reproducible build
panic = "abort"           # stack unwinding YOK (eh_frame)
overflow-checks = true    # integer overflow → trap (safety-critical)

# v0.3: small-data ve linker relaxation devre dışı
# Linker flag'ler workspace .cargo/config.toml'dan gelir (aşağı bak)
```

`.cargo/config.toml` (workspace root):

```toml
[target.riscv64imac-unknown-none-elf]
rustflags = [
    # Linker script (per-task)
    "-C", "link-arg=-T../task_layouts/task_template.ld",
    
    # v0.5 BELT+SUSPENDERS: Linker relaxation iki katmandan da kapalı.
    # (1) codegen-side: relax marker emit etme
    "-C", "target-feature=-relax",
    # (2) linker-side: marker olsa bile relax yapma (yedek savunma)
    "-C", "link-arg=--no-relax",
    # Gerekçe: relax pseudo-instruction'ları optimize ederken adres
    # kayması yapabilir → PMP region boundary'leri shift olur → izolasyon
    # invariant'ı kırılır. İkisi birden = LLVM versiyon değişikliğinde
    # davranış garanti edilir.
    
    # Small-data section kapalı — gp setup gerektirmez
    "-C", "link-arg=-G0",
    
    # eh_frame ve diğer gereksiz section'lar discard edilir
    "-C", "link-arg=--gc-sections",
    
    # Reproducible build
    "-C", "codegen-units=1",
]

[unstable]
build-std = ["core"]
build-std-features = ["compiler-builtins-mem"]
```

#### 4.8.3 Task Source Template

`tasks/task_template/src/main.rs`:

```rust
//! Sipahi SNTM task template — all SNTM tasks follow this pattern.
#![no_std]
#![no_main]

use sipahi_api::{syscall, ipc};

/// Task entry point — kernel mret to here.
/// 
/// SAFETY: Kernel ensures sp, mepc, mstatus correct. Task starts
/// with all caller-saved + callee-saved registers cleared (zero).
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // No global init — Sipahi forbids static initializers
    // (ctor sections discarded by linker /DISCARD/)
    
    // No gp setup — small-data disabled (-G0 in build flags)
    // No tp setup — TLS not used in SNTM
    
    main_loop()
}

/// Main task logic.
fn main_loop() -> ! {
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        
        // Periodic IPC send (örnek)
        if counter % 100 == 0 {
            let msg = ipc::Message::new(counter as u64);
            let _ = syscall::ipc_send(0, &msg);
        }
        
        syscall::yield_cpu();
    }
}

/// Panic handler — Sipahi doctrine: panic = abort.
/// 
/// stack unwinding YOK (panic_strategy = "abort" Cargo.toml'da).
/// .eh_frame section linker'da DISCARD edildi.
/// Task panic ederse: exit syscall ile kernel'a bildir, kernel
/// task isolate eder (policy engine).
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // exit syscall — kernel task'ı isolate edecek
    syscall::exit(255);
    // exit divergent (-> !) — buraya ulaşılmaz
    loop { core::hint::spin_loop(); }
}

// NOT: #[alloc_error_handler] YOK — task'larda alloc kullanılmaz.
//      Eğer alloc gerekirse static buffer + linked list pattern.
```

#### 4.8.4 Linker Script Template

`task_layouts/task_template.ld` (manifest'ten generate edilir):

```ld
/* Sipahi SNTM task linker script — generated by sntm-validate */

ENTRY(_start)

MEMORY
{
    /* Manifest'ten gelen değerler — sntm-validate substitute eder */
    TEXT   (rx)  : ORIGIN = 0x80100000, LENGTH = 16K
    RODATA (r)   : ORIGIN = 0x80104000, LENGTH = 4K
    DATA   (rw)  : ORIGIN = 0x80105000, LENGTH = 4K
    /* Stack ayrı bir region — task içinde değil */
}

SECTIONS
{
    .text :
    {
        KEEP(*(.text._start))   /* Entry point first */
        *(.text*)
    } > TEXT
    
    .rodata :
    {
        *(.rodata*)
        *(.srodata*)
    } > RODATA
    
    .data :
    {
        *(.data*)
        *(.sdata*)
    } > DATA
    
    .bss (NOLOAD) :
    {
        __bss_start = .;
        *(.bss*)
        *(.sbss*)
        *(COMMON)
        __bss_end = .;
    } > DATA
    
    /* Sipahi doctrine: bare-metal'de kullanılmayan section'lar */
    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.got)
        *(.got.plt)
        *(.plt)
        *(.comment)
        *(.note*)
        *(.dynsym)
        *(.dynstr)
        *(.hash)
        *(.gnu.hash)
        *(.init_array)    /* Static initializers YASAK */
        *(.fini_array)
        *(.preinit_array)
        *(.ctors)
        *(.dtors)
    }
}

/* Compile-time guarantees */
ASSERT(SIZEOF(.text)   <= LENGTH(TEXT),   ".text   exceeds region")
ASSERT(SIZEOF(.rodata) <= LENGTH(RODATA), ".rodata exceeds region")
ASSERT(SIZEOF(.data)   <= LENGTH(DATA),   ".data exceeds region")
```

#### 4.8.5 Build Flag Reference

```bash
# Task build (sntm-pack tarafından çağrılır):
cargo +nightly build --release \
    --target riscv64imac-unknown-none-elf \
    --bin task_template \
    --target-dir target/tasks/task_template

# Sonra ELF → per-section flat segments (v0.5 — Codex 3. review fix).
# ESKİ (v0.3): tek .bin, --only-section çoklu → VMA gap'leri zero-fill
#              0x80100000-0x80105000 arası 0x5000 byte boş alan israfı
#              + section hash ayrımı imkansız
# YENİ: her section ayrı .bin → kernel her birini PMP region'a kopyalar
riscv64-linux-gnu-objcopy -O binary --only-section=.text \
    target/tasks/task_template/release/task_template \
    target/tasks/task_template.text.bin

riscv64-linux-gnu-objcopy -O binary --only-section=.rodata \
    target/tasks/task_template/release/task_template \
    target/tasks/task_template.rodata.bin

riscv64-linux-gnu-objcopy -O binary --only-section=.data \
    target/tasks/task_template/release/task_template \
    target/tasks/task_template.data.bin

# .bss NOLOAD → kopya yok, boot'ta sıfırlanır (BSS clear loop)
# .stack NOLOAD → kopya yok, sadece RAM region reserve

# Kernel boot'ta her .bin'i kendi region'ına kopyalar:
#   bounded_copy(task.text.bin   → manifest.task.text.base,   sized)
#   bounded_copy(task.rodata.bin → manifest.task.rodata.base, sized)
#   bounded_copy(task.data.bin   → manifest.task.data.base,   sized)
#   zero_fill   (manifest.task.bss.base, manifest.task.bss.size)

# Avantajlar:
#   - Zero-fill israfı yok (her .bin = section size)
#   - Section başına ed25519 imza / BLAKE3 hash ayrı hesaplanabilir
#   - PMP region boundary'leri build-time explicit
#   - sntm-pack final image'da [text.bin][rodata.bin][data.bin] sırayla
#     concatenate eder, manifest tabel section offset'lerini tutar

# Manifest'e göre validate:
sntm-validate \
    --manifest sipahi.toml \
    --task task_template \
    --binary target/tasks/task_template.bin
```

#### 4.8.6 Relocation Policy

```
ABSOLUTE ADDRESSING ONLY:
─────────────────────────────────────────────────
- Linker relaxation KAPALI (codegen: -C target-feature=-relax + linker: --no-relax)
- PIC/PIE KAPALI (no -fpic, no -fpie)
- Task binary'leri manifest'teki SABİT adreslere link edilir
- Task binary YENİDEN LOCATE EDİLEMEZ (kernel relocation YOK)

WHY NOT PIC:
- Position-independent code RISC-V'de gp-relative kullanır
- Sipahi gp setup'ı yapmaz (small-data kapalı)
- PIC compile-time complexity getirir, runtime fayda yok
- Task adresleri manifest'te sabit → absolute addressing yeterli

WHY NOT RELAXATION:
- Linker relaxation: rel/auipc-jalr → c.j gibi instruction shrinking
- Bu boyut optimize ederken adresleri shift eder
- PMP TOR boundary linker section sonu → relaxation sonrası farklı
- Validator manifest'i compile-time check yapar, runtime shift bypass'a sebep
```

#### 4.8.7 Panic Handling

Task panic stratejisi:

```
PANIC STRATEGY:
─────────────────────────────────────────────────
1. Cargo.toml: panic = "abort"           (no unwinding)
2. linker .eh_frame DISCARD              (no eh_frame data)
3. Task source: #[panic_handler]         (task bağımsız her zaman)
4. Panic body: syscall::exit(255)        (kernel'a bildir)
5. Kernel: handle_task_fault() / policy  (Restart/Isolate/Shutdown)

PANIC TRACE:
- Production'da task panic = silent (UART trace yok, performance)
- Self-test/debug-boot'ta uart::println("[TASK PANIC] ...") opsiyonel
- Blackbox event: PolicyIsolate (kernel side, log)
```

#### 4.8.8 Global Init Kuralları

**RUSTRULES (Sipahi SNTM doctrine):**

```rust
// YASAK — sebep: ctor sections discard ediliyor, çağrılmaz
static FOO: Lazy<HashMap<...>> = Lazy::new(...);

// YASAK — global mutable state runtime initialization gerektirir
static mut COUNTER: u32 = compute_at_runtime();

// SERBEST — compile-time const, no runtime init
static PRIORITIES: [u8; 8] = [4, 8, 12, ...];
const MAX_COUNT: u32 = 100;

// SERBEST — once_cell için no_std variant veya manual SingleHartCell
static CELL: SingleHartCell<u32> = SingleHartCell::new(0);
// Task `_start` içinde manual init:
// unsafe { *CELL.get_mut() = 42; }
```

**Linker enforcement:** `.init_array, .fini_array, .preinit_array, .ctors, .dtors`
section'ları `/DISCARD/` ile silinir. Eğer task crate bunları üretirse
linker fail eder veya silently discard ile başarısız initialization olur.

#### 4.8.9 Common Boot Failure Patterns (Diagnostic)

Native Rust task'larının boot etmeme sebepleri ve fix:

```
SEMPTOM                          MUHTEMEL SEBEP             FIX
────────────────────────────────────────────────────────────────────────
Task immediate trap (mcause=2)   Illegal instruction         Build flags
                                  (FP instr accidentally)    target=imac
                                                              + no float

Task immediate trap (mcause=5)   Load access fault           Manifest data
                                  Wrong .data address        region != linker

Task spinning at _start          gp not set, small-data      -G0 flag
                                  addressing fail

Task crashes after few ticks     Stack overflow              stack region size
                                  PMP boundary               artır

Task starts but yield no effect  syscall ABI mismatch        sipahi-api crate
                                                              version match

Task crash on panic              eh_frame referans           panic = abort
                                                              + DISCARD eh_frame

Task .data zero unexpectedly     bss zero olmuş, .data       sntm-pack ELF
                                  copy edilmemiş              .data segment
                                                              bin'e dahil mi?

Task linker fail                 Section overlap             manifest validator
                                  Region overflow             reject etmeli
```

---

## 5. Çoklu-Region User Pointer Validation (v0.2)

### 5.1 Sorun (v0.1'den)

`is_valid_user_ptr` mevcut Sipahi'de **task stack range** kontrol ediyor.
SNTM'de task'ın 6 farklı region'u var — pointer hangi region'da?

### 5.2 Yeni API (v0.2)

```rust
#[derive(Clone, Copy)]
pub enum Access {
    Read,
    Write,
    Execute,
}

/// User pointer doğrulama — caller'ın tüm grant edilmiş region'larında.
/// 
/// SNTM v2.0+: task'ın text/rodata/data/bss/stack/mmio region'ları
/// PMP_PROFILES tablosundan okunur.
///
/// v0.3 OVERFLOW DEFANSİF: hem `ptr + size` hem `region.base + region.size`
/// için `checked_add`. Manifest validator zaten region overflow'u boot'ta
/// yakalar — runtime check defense-in-depth (cosmic ray / bug-late-injection).
#[must_use = "pointer validation result must be checked"]
fn is_valid_user_ptr(
    caller_task_id: u8,
    ptr: usize,
    size: usize,
    access: Access,
) -> bool {
    if ptr == 0 { return false; }
    let end = match ptr.checked_add(size) {
        Some(e) => e,
        None => return false,  // user pointer overflow → deny
    };
    
    let profile = match get_pmp_profile(caller_task_id) {
        Some(p) => p,
        None => return false,  // Dead/Isolated/uninitialized → deny
    };
    
    // Bounded loop: 6 region max
    let mut i = 0;
    while i < profile.region_count && i < 6 {
        let region = &profile.regions[i];
        
        // v0.3 FIX: region.base + region.size de checked_add.
        // Eğer manifest bozuk veya cosmic-ray ile bozulduysa silently
        // wrap'e izin verme — defansif olarak deny.
        let region_end = match region.base.checked_add(region.size) {
            Some(e) => e,
            None => return false,  // region overflow → deny (defansif)
        };
        
        // Pointer bu region içinde mi?
        if ptr >= region.base && end <= region_end {
            // Access permission kontrol
            return match access {
                Access::Read    => region.perm.r,
                Access::Write   => region.perm.w,
                Access::Execute => region.perm.x,
            };
        }
        i += 1;
    }
    false  // Hiçbir region'da değil → deny
}
```

### 5.3 Syscall Tarafında Kullanım

```rust
// sys_ipc_send: msg_ptr task'ın READABLE region'ında olmalı
fn sys_ipc_send(channel_id: usize, msg_ptr: usize, ...) -> usize {
    let caller = current_task_id();
    if !is_valid_user_ptr(caller, msg_ptr, IPC_MSG_SIZE, Access::Read) {
        return E_INVALID_ARG;
    }
    // ...
}

// sys_ipc_recv: buf_ptr task'ın WRITABLE region'ında olmalı
fn sys_ipc_recv(channel_id: usize, buf_ptr: usize, ...) -> usize {
    let caller = current_task_id();
    if !is_valid_user_ptr(caller, buf_ptr, IPC_MSG_SIZE, Access::Write) {
        return E_INVALID_ARG;
    }
    // ...
}
```

**Codex'in haklı uyarısı:** stack-only kalsa, task `ipc_send(channel, &CONSTANT_DATA)`
yazmak istese reject edilirdi (`.rodata`'dan okumak yasak olurdu). Multi-region
read/write ayrımı bu pattern'ı destekler.

---

## 6. 7 Katmanlı Güvenlik (v0.2 — PMP Policy Doğru İfade Edildi)

```
Katman 1 — CPU Privilege Mode:
  Task U-mode'da. Privileged instruction → CPU illegal instruction trap.
  csrw, mret, sfence → yazılımla bypass edilemez.

Katman 2 — PMP Bellek İzolasyonu (POLICY):
  Her task: SADECE manifest'te tanımlı region'lara erişir.
  Kernel .text/.rodata/.data/.bss → U-mode'a hiç grant EDİLMEZ.
  Diğer task'ların region'larına erişim YOK.
  
  ÖNEMLİ POLICY KURAL: Asla "all-RAM RW" entry açma. Bu vanilla PMP'nin
  zayıflığı değil, kötü policy — Sipahi v1.0'da da bu kurala uyuluyor.

Katman 3 — PMP Shadow Verification:
  Her tick PMP register → shadow karşılaştırma (mevcut U-21 fix).
  Glitch → mismatch → SHUTDOWN.

Katman 4 — Multi-Region Syscall Validation:
  is_valid_user_ptr(caller, ptr, size, access) — 6 region scan
  ipc_send → READ region required
  ipc_recv → WRITE region required
  Cross-task pointer → impossible (region'lar disjoint)

Katman 5 — mstatus.MPP Check (mevcut U-15 fix):
  Trap kaynağı M-mode gibi görünürse → SHUTDOWN.

Katman 6 — Lockstep Policy (mevcut):
  decide_action 2× çağrı + black_box fence.
  SEU → mismatch → SHUTDOWN.

Katman 7 — Watchdog + Budget (mevcut):
  Sonsuz loop → budget enforcement → suspend.
  Watchdog timeout → policy → isolate/restart.
```

### 6.1 Vanilla PMP'nin Gerçek Sınırı

Codex'in doğru kritiği: vanilla PMP **kernel data koruyabilir** doğru
policy ile. Smepmp ek garantiler getiriyor ama vanilla yetersiz değil.

```
Vanilla PMP doğru policy ile:           Smepmp ek katkısı:
─────────────────────────────────       ──────────────────────────
✓ U-mode kernel data deny               ✓ M-mode için aynı kural (MML)
✓ Per-task region isolation             ✓ Default deny (MMWP)
✓ Cross-task deny                       ✓ Boot-time lockdown (RLB)
                                        ✓ M-mode bug'larında ek savunma

Vanilla yeterli (policy doğruysa):      Smepmp daha güçlü:
- v1.0 → v2.0 için OK                   - v2.5+ hedef (CVA6 destek geldiğinde)
- Sipahi'nin tüm korumaları çalışır     - Defense in depth
```

### 6.2 DMA / IOPMP — Ayrı Katman (YENİ v0.2)

**Codex'in tamamen haklı eklemesi.** PMP CPU bus master için. DMA-capable
cihazlar bypass eder.

```
Senaryo:                                Sonuç:
─────────────────────────────────────────────────────────────
Task DMA yapan cihazı (Ethernet) yönetiyor    ⚠ DMA → kernel data
PMP CPU'yu kontrol ediyor                       PMP atlanır

Çözümler (sıralı):
  1. SNTM v1.5: DMA-capable cihaz YOK manifest'te (default)
     ★ Sipahi v1.0/v1.5 zaten bu durumda
  
  2. SNTM v2.0+: IOPMP ekle, DMA cihazları IOPMP rule'larına tabi
     manifest'te [task.dma] enabled = true ise
       → manifest validator iopmp_required check
  
  3. DMA-capable cihaz IOPMP olmadan grant edilirse:
     → manifest validator REJECT
     → "Trusted task" kuralı uygulanmadıkça hata
```

Manifest yeni alan:

```toml
[task.dma]
enabled = false              # Default: yok
# Eğer enabled = true:
# iopmp_required = true      # IOPMP olmadan validation fail
# trusted_only = true        # Sadece DAL-A trusted task

[platform]
iopmp_present = false        # CVA6 vanilla'da false
                             # FPGA + IOPMP IP olunca true
```

---

## 7. Performans Karşılaştırması (v0.2)

```
Metrik              | WASM+wasmi        | SNTM
────────────────────────────────────────────────────
Execution speed     | %10-20 native     | %100 native
Context switch      | Interpreter state | Register save/restore
                    | + WASM stack      | + PMP profile reload (5 entry)
Bellek overhead     | wasmi instance    | Task binary only
                    | + linear memory   | (birkaç KB)
                    | + fuel counter    |
Binary boyutu       | +1MB (wasmi)      | +task binary (KB)
WCET                | Ölçülemez         | rdcycle native
Kernel TCB          | +30K LOC          | +~300 LOC (loader + validator)
PMP entry kullanım  | ~2 (kernel sade)  | ~11 (kernel + task profile)
Build complexity    | Düşük             | Orta (sntm-pack + validator)
```

**WCET context switch:** Mevcut Sipahi 80c (config.rs:122). SNTM'de PMP
profile reload için ek ~100-150c (5 entry × ~20-30c CSR write). Toplam
~200-250c. Hâlâ tick budget (100K) içinde rahat.

---

## 8. Sipahi API (v0.7 — Mevcut 5 Syscall + 1 Yeni)

Mevcut 5 Sipahi syscall'ı **aynen geçerli**, panic handler için **1 yeni
syscall** eklenir (§4.8.3 panic body uyumu — v0.2'deki "yeni syscall
gerekmez" iddiası v0.7'de düzeltildi, kendi içinde tutarsızdı):

```rust
// sipahi_api crate (task'lar bunu kullanır)
pub mod syscall {
    pub fn cap_invoke(cap: u8, resource: u16, action: u8) -> Result<(), Error>;
    pub fn ipc_send(channel: u8, msg: &Message) -> Result<(), Error>;
    pub fn ipc_recv(channel: u8) -> Result<Message, Error>;
    pub fn yield_cpu();
    pub fn task_info(task_id: u8) -> TaskInfo;
    pub fn exit(code: u8) -> !;   // YENİ v0.7 — panic handler + voluntary term
}

// Syscall ID şeması (config.rs):
//   SYS_CAP_INVOKE = 0
//   SYS_IPC_SEND   = 1
//   SYS_IPC_RECV   = 2
//   SYS_YIELD      = 3
//   SYS_TASK_INFO  = 4
//   SYS_EXIT       = 5  (YENİ v0.7)
//   SYSCALL_COUNT  = 6  (5 → 6 v0.7)
```

**Niye yeni syscall**: §4.8.3 panic_handler `syscall::exit(255)` çağırıyor —
task panic ederse kernel'a "beni isolate et" bildirir. Bu syscall'sız task
panic kernel'ın handle_task_fault path'ine düşer (trap kontrolüyle), ama
**voluntary** termination (görev tamamlandı, exit code = 0) ifade edilemez.
panic = abort kontratıyla beraber, voluntary exit syscall'ı SNTM task
lifecycle'ı için zorunlu.

**Kernel side davranış**: SYS_EXIT handler `isolate_task()` çağırır + 
`schedule_yield()` ile dispatch'i kernel'a verir. Task → TaskState::Isolated,
scheduler atlar. invalidate_task_capabilities eski helper içinde mevcut.

**Diğer değişiklik:** `is_valid_user_ptr` multi-region'a güncellenir
(yukarıda §5.2). Syscall ABI (a7=id, a0-a3=args, return=a0) aynı kalır.

---

## 9. Formal Verification Sınırları (v0.2 — Düzeltilmiş)

**Codex'in haklı uyarısı:** "formal verified ✓" fazla iddialı. Doğru
ifade:

```
Kani'nin yapabileceği:                Kani'nin YAPAMAYACAĞI:
─────────────────────────────────     ──────────────────────────────
✓ Manifest parser bounds              ✗ PMP donanımı doğru çalışıyor mu
✓ Task loader bounded copy            ✗ CVA6 RTL bug yok mu
✓ is_valid_user_ptr multi-region      ✗ Compiler doğru kod üretiyor mu
✓ PMP profile collision detect        ✗ Task binary WCET'i ne
✓ Channel ID bounds                   ✗ DMA davranışı (PMP-out-of-scope)
✓ ed25519 signature path              ✗ ed25519 kriptografik güvenlik
✓ Kernel control flow                 ✗ Task içi davranış

→ "formal verification friendly" ✓
→ "formal verified" ✗ (sadece bazı invariantlar için)
```

### 9.1 Doğru Formal Verification Pipeline

```
Kani (bounded model checking):
  - load_task bounded copy (size ≤ region_size)
  - is_valid_user_ptr region scan (6 region max)
  - PMP profile collision detection
  - Manifest parser overflow protection

TLA+ (temporal logic):
  - SipahiSNTM.tla: task lifecycle (Load → Ready → Running → ...)
  - PMP profile reload atomicity (context switch)
  - Channel sealing invariant

Static analysis (cargo clippy + custom lint):
  - No alloc in kernel code paths
  - No panic in production (panic = abort)
  - No early-exit in lookup paths

Build-time (sntm-validate):
  - PMP budget < platform.pmp_entries
  - Region overlap detection
  - NAPOT alignment compliance
  - DMA → IOPMP requirement
  - Channel producer/consumer assignment exists

Empirical (FPGA):
  - Per-task context switch cycle measurement
  - Per-syscall cycle measurement
  - PMP reconfiguration measurement
  - End-to-end IPC latency

Manual review:
  - Assembly correctness (trap.S, context.S)
  - SAFETY:// comment quality
  - Audit trail (commit messages, design docs)
```

---

## 10. Kazanım ve Kayıp — Dürüst Hesap (Codex)

### Gerçek Kazanımlar:

```
✓ TCB küçülür (~30K LOC azalma — wasmi)
✓ Production binary küçülür (~1MB → ~32KB)
✓ Interpreter WCET belirsizliği gider
✓ alloc baskısı azalır (sandbox alloc gone)
✓ Sandbox güvenliği wasmi correctness yerine PMP/CPU'ya dayanır
✓ Native task FPGA WCET ölçümü daha anlamlı
✓ AMCI'ye geçiş daha doğal (per-hart task profile)
```

### Kayıplar (Codex'in haklı eklemesi):

```
✗ WASM'ın bytecode-level memory safety'si gider
✗ Native task unsafe yazarsa kendi partition'ını bozabilir
   (PMP isolation diğer task'ları korur, ama task kendi içinde özgür)
✗ Taşınabilirlik azalır (RISC-V binary ≠ portable bytecode)
✗ Loader/build pipeline karmaşıklığı gelir (sntm-pack + validator)
✗ PMP policy hatası varsa WASM'dan daha tehlikeli olabilir
   (yanlış grant edilmiş kernel data > sandbox bypass)
```

### Net Sonuç:

**SNTM "native diye otomatik güvenli" değil.** Güvenlik şunlardan gelir:
1. PMP manifest doğruluğu (sntm-validate compile-time check)
2. Syscall validation (is_valid_user_ptr multi-region)
3. Task ABI (sipahi_api crate kontratı)
4. Build pipeline doğruluğu (sntm-pack + signature)

WASM'ı kaldırdığımızda **sandbox boyutunu küçültüp formal-verifiable
katmana taşıyoruz**. Daha güvenli değil, **daha kanıtlanabilir**.

---

## 11. Evrim Yolu (v0.2)

```
v1.1 (mevcut, U-22 sonrası):
  Sipahi v1.0 + WASM feature-gated (production'da kapalı)
  AMCI design doc (SACA-FS, CFG, SC-CTK eklenmiş)

v1.5 (SNTM Phase 1 — 1 ay):
  - sipahi_api crate (syscall wrappers)
  - tasks/task_hello — ilk native task
  - Manifest parser + sntm-validate (host tool)
  - QEMU'da single-task SNTM boot

v2.0 (SNTM Phase 2 + AMCI Phase 1 — 2-3 ay):
  - Full multi-region PMP profiles
  - Task loader (bounded copy + signature verify)
  - 2-task SNTM demo (sensor + actuator)
  - AMCI multi-hart başlangıç (1 supervisor + 1 worker)
  - Smepmp opsiyonel destek

v2.5 (Smepmp + IOPMP):
  - Smepmp policy enforce (CVA6 destek geldiğinde)
  - IOPMP DMA isolation (MPFS-DISCO-KIT'te zaten var,
    mainline CVA6 için bekle)

v3.0+ (Hardware-accelerated):
  - CHERI-RISC-V hibrit (PMP coarse + CHERI fine)
  - RISC-V Worlds (system-level isolation)
  - Zknh + SACA-FS + SC-CTK + CFG entegre
  - Production silikon (FPGA → ASIC)

v3.5+ (Sertifikasyon):
  - DO-178C DAL-B/A submission
  - AbsInt aiT WCET tool
  - Independent assessment
  - Reference design
```

---

## 11.5 WASM Artefakt Temizleme Kararları (YENİ — v0.4)

U-22'de WASM feature-gated. Ama Sipahi içinde hala **WASM-tied
adacık** var: dispatch_compute, COMPUTE_* sabitleri, sandbox Kani
proof'ları, alloc bağımlılığı. SNTM implementation öncesi bu adacık
**bilinçli olarak** çözülmeli — karmaşa SNTM'e taşınmasın.

### 11.5.1 WASM-Adacık Envanteri

```
src/sandbox/ klasörü:
  mod.rs (~600 satır)
    - is_float_opcode                ← WASM validator
    - read_u32_leb128/read_leb128    ← WASM bytecode parser
    - dispatch_compute                ← compute services dispatcher
    - skip_instruction                ← WASM opcode walker
    - verification mod (Kani proof'lar)
    
  allocator.rs (~100 satır)
    - BumpAllocator                  ← WASM heap allocator
    - global_allocator alias

main.rs:
  extern crate alloc;                ← WASM için (+ ed25519-dalek)
  #[global_allocator] static ALLOCATOR
  #[alloc_error_handler]
  #![feature(alloc_error_handler)]

config.rs:
  COMPUTE_COPY/CRC/MAC/MATH (4 sabit, u8 service ID)
  WCET_COMPUTE_COPY/CRC/MAC/MATH (4 WCET sabiti)

verify.rs:
  Proof 12: COMPUTE_* WCET ordering (stub-ish)

Cargo.toml:
  wasmi = { ..., optional = true }    ← dep:wasmi
  wasm-sandbox feature
  ed25519-dalek features = ["alloc"]  ← alloc tüketicisi
  blake3 (alloc gerektirmez)

sipahi.ld:
  .wasm_arena (NOLOAD) — production'da 0 byte, hala tanımlı

Self-test (tests/mod.rs):
  Sprint 12 WASM test bloğu (~50 satır)
  COMPUTE_* test'leri (~30 satır)

Kani proof'lar (toplam 200):
  sandbox::verification mod: ~10-15 proof
  allocator wrapping_add:    ~2 proof
  LEB128 parser:             ~3 proof
  Float rejection:           ~2 proof
```

**Adacık büyüklüğü:** ~800 LOC + ~15 Kani proof + ~8 config sabit
+ 1 cargo optional dep + 1 linker section.

### 11.5.2 Karar Matrisi (Item × Versiyon)

```
Item                             v1.1 (mevcut)   v1.5 (SNTM)        v2.0 (final)
─────────────────────────────────────────────────────────────────────────────────
src/sandbox/mod.rs               feature gate ✓  SİL                YOK
src/sandbox/allocator.rs         feature gate ✓  SİL                YOK
extern crate alloc;              ungate (ed25519)KALIR              ed25519-compact'a
                                                                     geç → SİL
#[global_allocator]              wasm gate ✓     KALIR (alloc için) SİL
#[alloc_error_handler]           wasm gate ✓     KALIR              SİL
#[feature(alloc_error_handler)]  wasm gate ✓     KALIR              SİL
COMPUTE_* ID sabitleri           hala var        SİL (orphan)       YOK
WCET_COMPUTE_* sabitleri         hala var        docstring'e taşı   YOK
dispatch_compute() fn            sandbox'ta      SİL (caller yok)   YOK
WASM Kani proofs                 self-test'te    wasm gate kalır    SİL (~15)
Allocator Kani proofs            her zaman      KALIR (BumpAlloc) SİL
LEB128 Kani proofs               her zaman      KALIR (utility)    SİL veya taşı
wasmi optional dep               Cargo.toml      KALIR (gate)       TAMAMEN SİL
.wasm_arena linker section       var (boş)      KALIR              linker'dan SİL
Sprint 12 WASM self-test         var             wasm gate kalır    SİL
ed25519-dalek alloc feature      var             KALIR              ed25519-compact
                                                                     ile değiştir
```

### 11.5.3 Karar Justifikasyonu

```
1. sandbox/ klasörü SİL (v1.5):
   Gerekçe: SNTM tasks/ klasörü ile yer değişir. wasm-sandbox feature
   gate kalsa bile dead code linter şikayet eder. Temiz çizgi.
   
2. extern crate alloc UNGATE bırak (v1.5):
   Gerekçe: ed25519-dalek alloc bağımlılığı runtime'da Vec kullanabilir
   (signature verify path). Gate edersek link error riski. v2.0'da
   ed25519-compact'a geçince temizle.
   
3. COMPUTE_* sabitleri SİL (v1.5):
   Gerekçe: Kaynak kodunda kullanıcı YOK (M14 audit). dispatch_compute
   sandbox/mod.rs'te sadece sandbox tarafından çağrılabilirdi. SNTM
   task'ları kendi kütüphanelerini kullanır.
   
4. WCET_COMPUTE_* docstring (v1.5):
   Gerekçe: WCET sabiti olarak değer düşük (kernel'da kimse yapmıyor).
   Ama "tarihsel benchmark" olarak doc'ta kalması faydalı. Future
   compute service yazılırsa referans.
   
5. WASM Kani proofs gate kalır (v1.5), SİL (v2.0):
   Gerekçe: 200 → 185 sayısı v1.5'te değişmesin (self-test build hala
   kullanıyor). v2.0'da WASM tamamen silinince proofs temizlenir.
   
6. ed25519-compact migration (v2.0):
   Gerekçe: alloc'tan tamamen kurtulmanın tek yolu. ed25519-dalek
   Vec kullanıyor signature verify path'inde. ed25519-compact pure
   no_alloc alternative — API biraz farklı ama port edilebilir.
```

---

## 11.6 Compute Services Akıbeti (YENİ — v0.4)

`COMPUTE_COPY`, `COMPUTE_CRC`, `COMPUTE_MAC`, `COMPUTE_MATH` —
WASM host function olarak tasarlanmıştı, hiç çağrılmadı.

### Per-Service Karar

```
COMPUTE_COPY (memcpy):
  Tasarım amacı:  WASM sandbox bellek kopyalama wrapper'ı
  Gerçek kullanım: HİÇ (production'da çağrılmadı)
  v1.5 karar:     SİL
  Sebep:          core::ptr::copy_nonoverlapping veya
                  core::slice::copy_from_slice her crate'te zaten var.
                  Sipahi-spesifik wrapper'a gerek yok.
                  
COMPUTE_CRC (CRC32):
  Tasarım amacı:  Blackbox record CRC + WASM call
  Gerçek kullanım: Kernel ipc/mod.rs:crc32() içerir (zaten)
                   IpcMessage::set_crc() / verify_crc() içerir (zaten)
  v1.5 karar:     SİL config sabiti. crc32 fn'i ipc/mod.rs'te kalır.
                   sipahi_api crate task tarafı için re-export.
  Sebep:          Service ID'ye gerek yok — kernel zaten internal kullanıyor,
                  task tarafı sipahi_api wrapper ile erişebilir.
                  
COMPUTE_MAC (BLAKE3 keyed hash):
  Tasarım amacı:  Token MAC + WASM verify
  Gerçek kullanım: Kernel broker.rs Crypto::keyed_hash() içerir (zaten)
  v1.5 karar:     SİL config sabiti. Crypto::keyed_hash kernel-side kalır.
  Sebep:          MAC key kernel'a ait — task tarafı erişemez. sys_cap_invoke
                  ile zaten kullanılıyor.
                  
COMPUTE_MATH (Q32.32 vektör dot product):
  Tasarım amacı:  WASM sandbox math accel
  Gerçek kullanım: HİÇ
  v1.5 karar:     SİL
  Sebep:          Niche, kimse kullanmadı. Task'lar fixed-point lib veya
                  manuel hesap yapabilir.
```

### Sonuç

**4 compute service → 0 kernel exposure.** dispatch_compute fonksiyonu
sandbox/mod.rs ile birlikte v1.5'te silinir. COMPUTE_* / WCET_COMPUTE_*
sabitleri config.rs'ten kaldırılır.

`sipahi_api` crate task-side **`crc32()` wrapper'ı** sağlar — kernel
fonksiyonunu doğrudan re-export. Diğer 3 service için task tarafı kendi
implementasyonunu yazar (veya core/std crate'leri kullanır).

---

## 11.7 WASM Kani Proof Inventory (YENİ — v0.4)

Mevcut 200 Kani proof'tan ~15 tanesi WASM-tied. Bunların akıbeti:

```
Proof                                Lokasyon                v1.5    v2.0
─────────────────────────────────────────────────────────────────────────
Proof 12: COMPUTE_* WCET ordering    verify.rs                SİL     SİL
                                     (compute sabitleri silinecek)
                                     
Proof 145: dispatch_compute_empty    sandbox/mod.rs:663       SİL     SİL
                                     (dispatch_compute silinecek)
                                     
Allocator wrapping_add (2 proof)     sandbox/allocator.rs     KALIR   SİL
                                     (BumpAllocator v1.5'te kalır,
                                      ed25519 için)
                                     
LEB128 parser (3 proof)              sandbox/mod.rs           KALIR   SİL
                                     (utility, future SNTM ELF
                                      parser'a evrilirse faydalı)
                                     
Float rejection (2 proof)            sandbox/mod.rs           KALIR   SİL
                                     (WASM'a özgü, v2.0'da silinir)
                                     
WASM execution path (~5 proof)       sandbox/mod.rs           KALIR   SİL
                                     (wasm-sandbox feature gate ile)

Toplam:
  v1.5 sonrası:  200 - 2 = 198 proof aktif
                 ~13 proof feature-gated (wasm-sandbox açıkken)
  v2.0 sonrası:  198 - 13 = 185 proof aktif
                 (WASM tamamen silinince)
```

### Doc Senkronu

`README.md` ve `ARCHITECTURE.md`'de Kani proof sayısı güncellemesi:

```
v1.1 (mevcut):     "200 Kani proof"
v1.5 (cleanup):    "198 active Kani proofs (+ 13 feature-gated WASM)"
v2.0 (final):       "185 active Kani proofs (WASM tamamen kaldırıldı)"
```

---

## 12. v1.0 → SNTM Geçiş Planı (v0.2)

### Aşama 1 — WASM Feature Gate (✓ U-22'de TAMAMLANDI)

```
✓ wasmi optional dependency  (Cargo.toml: dep:wasmi)
✓ wasm-sandbox feature gate
✓ Production .wasm_arena = 0 byte
✓ TCB azaldı (~200KB tasarruf)
```

### Aşama 1.5 — Pre-SNTM Cleanup Sprint (v1.1.1, ~3 saat) — YENİ v0.4

**Amaç:** SNTM Phase 1 başlamadan önce WASM artefaktlarını çöz.
Detay: §11.5, §11.6, §11.7.

**Sprint U-22.5 — 10 görev (v0.5: Codex 3. review sıra düzeltmesi):**

> **⚠️ KRİTİK SİLME SIRASI:** Reviewer doğru tespit etti — verify.rs
> COMPUTE_* (line 121) ve WCET_COMPUTE_* (line 92-97, 245-251) referansları
> var. Eğer G1/G2 (config.rs sabitleri) önce silinirse verify.rs compile
> fail. **DOĞRU SIRA: G4 → G1 → G2 → G3** (proof remove ilk, sonra
> sabitler, en son sandbox modülü).

```
GÖREV                                              SÜRE  SIRA
─────────────────────────────────────────────────────────────
G4   verify.rs proof temizliği (İLK!)               15 dk  [1]
     - Proof 4 (wcet_ordering_consistent) içinden
       WCET_COMPUTE_* assertion'ları sil (line 92-97)
     - Proof 12 (compute WCET ordering, line 245-251) sil
     - Proof 5 (syscall_ids_valid) içinde COMPUTE_*
       referansı VARSA temizle (line 121)
     → Bu adımdan sonra cargo kani compile etmeli
     
G1   COMPUTE_* (4 sabit) config.rs'ten sil          10 dk  [2]
     - COMPUTE_COPY, COMPUTE_CRC, COMPUTE_MAC,
       COMPUTE_MATH ID sabitleri
     → G4 sonrası bu sabitlerin tek tüketicisi sandbox/mod.rs
     
G2   WCET_COMPUTE_* (4 sabit) docstring'e taşı     15 dk  [3]
     veya tamamen sil (kararı sen ver)
     - WCET_COMPUTE_COPY, WCET_COMPUTE_CRC,
       WCET_COMPUTE_MAC, WCET_COMPUTE_MATH
     → tick budget const_assert (config.rs U-22 G6) güncellemesi:
       WCET_COMPUTE_CRC referansını WCET_CONTEXT_SWITCH veya
       eşdeğer worst-case kernel WCET ile değiştir
     
G3   dispatch_compute fonksiyonunu sandbox'tan sil  15 dk  [4]
     - sandbox/mod.rs:288 fn dispatch_compute
     - İçerideki Proof 145 da silinir
     
G5   sipahi_api crate iskeletini yarat             30 dk
     (henüz tasks/ yok ama crate hazır olsun)
     - sipahi_api/Cargo.toml
     - sipahi_api/src/lib.rs
     - pub mod syscall { ... } stub
     - pub mod crc { pub use kernel::ipc::crc32; }
     
G6   Sandbox stale yorum audit (U-22 G18 doğrula)   5 dk
     grep "64KB\|120c\|CRC.*cycle" src/sandbox/
     
G7   Cargo.toml ed25519-dalek alloc transition not  5 dk
     # NOT: v2.0 SNTM final'inde ed25519-compact
     # ile değiştirilecek — alloc bağımlılığı kaldırılır
     
G8   ARCHITECTURE.md veya yeni dosya:               45 dk
     SIPAHI_V1_TO_V2_TRANSITION.md
     - §11.5/§11.6/§11.7 özet
     - Pre-SNTM cleanup checklist
     - v1.5/v2.0 migration aşamaları
     
G9   make check + cargo kani + run-self-test         15 dk
     - 200 → 198 Kani proof (G3, G4 sonrası)
     - Self-test PASS
     - Production NF-free
     
G10  CHANGELOG.md v1.1.1 entry + git tag v1.1.1     5 dk
     ### [1.1.1] - 2026-XX-XX
     - Pre-SNTM cleanup: COMPUTE_* services removed
     - dispatch_compute deleted (orphan code)
     - sipahi_api crate scaffolding
     - ed25519-compact migration note (v2.0 hedef)
─────────────────────────────────────────────────────────
TOPLAM                                              ~2.5 saat
                                                    (1 günden kısa)
```

**Çıktı v1.1.1:**
- 200 Kani proof → 198 Kani proof
- ~150 satır kod siliyor (compute services + Proof 12)
- sipahi_api crate hazır (boş ama scaffolded)
- v1.5 → SNTM Phase 1 temiz baseline'dan başlar

**Niye Aşama 1.5 ZORUNLU:**
- WASM-tied artefaktlar SNTM kodu yazılırken bulaşma yapar
- COMPUTE_* sabitleri "neden hala burada" sorusunu doğurur (new contributor confusion)
- 200 Kani proof iddiası bir kısmı feature-gated stale'dir
- sipahi_api crate iskelet'i v1.5 işini kolaylaştırır

### Aşama 2 — sipahi_api Crate (v1.5, ~1 hafta)

```
□ tasks/ dizini oluştur
□ sipahi_api crate: ecall wrappers
   - syscall::cap_invoke
   - syscall::ipc_send / ipc_recv
   - syscall::yield_cpu
   - syscall::task_info
   - exit (panic_handler)
□ İlk native task: task_hello (loop + yield + UART_via_ipc)
□ Build script: cargo build for tasks separately
□ QEMU smoke test: task_hello boot
```

### Aşama 3 — Manifest + sntm-validate (v1.5, ~1 hafta)

```
□ tools/sntm-validate/ host tool
   - sipahi.toml parser (toml crate)
   - PMP budget check
   - Region overlap detection
   - NAPOT alignment validation
   - DMA → IOPMP requirement
   - Channel assignment verification
□ Generated Rust const tables
   - TASK_TABLE[N]
   - PMP_PROFILES[N]
   - CHANNEL_TABLE[M]
```

### Aşama 4 — Multi-Region PMP Profile (v1.5, ~2 hafta)

```
□ memory/mod.rs: PmpProfile struct + multi-region support
□ scheduler/mod.rs: context switch'te per-task PMP profile reload
□ syscall/dispatch.rs: is_valid_user_ptr multi-region (Access enum)
□ Kani proof'lar:
   - PMP profile collision detection
   - is_valid_user_ptr scan bounded
□ TLA+ spec: SipahiSNTM.tla task lifecycle
```

### Aşama 5 — Task Loader (v2.0, ~1 hafta)

```
□ tools/sntm-pack: ELF → flat segments + signature
   - riscv64-linux-gnu-objcopy ile .bin extract
   - Manifest'ten layout oluştur
   - ed25519 signature
□ Kernel: load_task bounded copy
   - Build-time const: TASK_BINARIES[N] flat segment'leri
   - Boot'ta verify_signature → copy → zero_bss
   - PMP profile setup (build-time const)
□ İki-task demo: task_sensor + task_actuator IPC ile haberleşiyor
```

### Aşama 6 — wasmi Tamamen Sil (v2.0)

```
□ Cargo.toml: wasmi dependency tamamen sil
□ src/sandbox/ klasörü sil
□ wasm-sandbox feature sil
□ extern crate alloc; sil (eğer ed25519-dalek de değiştirildiyse)
□ ed25519-dalek → ed25519-compact (no_alloc alternatif)
   ★ Bu adım Sipahi'yi tam no_std + no_alloc yapar
```

### Aşama 7 — SNTM-SAFE Phased Rollout (v1.6 → v1.9, YENİ v0.6)

§17'de detaylı, burada özet:

**SAFE-1 — Safe Native Profile (v1.6, ~1 hafta):**

```
□ §17.2 Safe Native Profile doc + manifest schema
□ task-lint tool (~300 LOC):
  - forbid(unsafe_code) check
  - alloc/asm/ffi/recursion grep
  - dyn Trait / fn ptr usage check
  - trust_tier manifest gate
□ CI integration: cargo build --features task_* sonrası task-lint zorunlu
□ Mevcut task_a/task_b'yi safe tier'a uydur (zaten unsafe yok)
```

**SAFE-2 — Static Cap Table + Typed IPC (v1.7, ~1.5 hafta) — EN YÜKSEK DEĞER:**

```
□ §17.5 LOCAL_CAP_TABLE codegen (sntm-validate extension)
□ src/kernel/capability/local_cap.rs: local_cap_check() implementation
□ Local capability call sites migrate:
  - validate_cached() → local_cap_check() (local resources)
  - validate_full() KALIR (cross-hart, HSM, external token)
□ Kani proof: local_cap_check_bounded, local_cap_no_overflow
□ Benchmark: cache miss path 400c → 5c verify (mtime ile)
□ §17.6 Typed IPC: sntm-validate channel codegen
□ sipahi_api/src/channels.rs auto-generated
□ Demo task'lar typed API kullansın (ham ipc_send kaldır)
□ Kani proof: typed_ipc_size_invariant
```

**SAFE-3 — Binary Verifier + Task Certificate (v1.8, ~2-3 hafta):**

```
□ §17.3 riscv-bin-verify (yeni tool, ~1500 LOC):
  - ELF parser (lopdf veya elf-rs ile)
  - Forbidden opcode tarayıcı (RV64IMAC instruction decode)
  - Section/relocation kontrolleri
  - Region boundary check (manifest cross-ref)
□ §17.4 TaskCertificate schema + sntm-validate generator
□ ed25519 sign certificate (build-time)
□ CI gate: bin-verify FAIL → build fail
□ Image format: kernel + tasks + per-task .cert + image sig
```

**SAFE-4 — Stack Analyzer + Full CI Gate (v1.9, ~1 hafta):**

```
□ §17.7 cargo-call-stack integration
□ Manifest stack_size ≤ analyzer max_stack_bytes check
□ §17.10 sntm_safe_gate.sh — 10-step full gate
□ CI: tüm 10 gate green olmazsa image deploy bloklanır
□ Watermark/scribble debug-boot için opsiyonel
```

**CFI Faz (v2.5+, hardware-dependent):**

```
□ §17.8 CFI roadmap milestone — CVA6-CFI olgunluğa bekle
□ Manifest cfi_required = true bayrağı
□ Zicfilp/Zicfiss varsa enable, yoksa skip (graceful)
□ CHERIoT entegrasyonu — uzak vadeli (v3.0+)
```

**Sıralama gerekçesi:**
- SAFE-2 (static cap table) EN YÜKSEK NET WIN — 80× hızlanma + güvenlik
  artışı. v1.7'de erken alınması mantıklı.
- SAFE-3 (binary verifier) en uzun süre + en büyük yeni tool — base SNTM
  stabilize olduktan sonra eklenmeli.
- SAFE-4 son kapanış — diğer faz'ların green olduğu doğrulanır.

---

## 13. AMCI ile Etkileşim

SNTM ve AMCI birbirini tamamlar:

```
SNTM:                              AMCI:
─────────────────────────          ───────────────────────────
Per-task PMP isolation             Per-hart kernel isolation
Manifest-defined tasks             Manifest-defined harts
Local (single-hart)                Global (multi-hart)

Birlikte:
  Per-hart Sipahi instance (AMCI)
  Her hart üzerinde SNTM task'lar
  Cross-hart capability IPC (AMCI)
  Within-hart task IPC (mevcut)
```

AMCI v2.0 + SNTM v2.0 hedef tarihleri çakışır — birlikte implement edilirler.
AMCI design doc'undaki §3.2 PerHartState pattern SNTM task'lar için
genişletilir (per-hart + per-task PMP profile).

---

## 14. Açık Sorular (v0.2)

```
1. ELF entry point convention:
   _start mı, main mı? RISC-V calling convention nasıl set edilir?
   → Karar: _start, sp + mepc manifest'ten gelir, no calling convention
              (task argümansız "() -> !")

2. Task'lar arası shared memory:
   IPC channel + capability YETERSİZ mi? Shared memory gerekir mi?
   → Karar: v2.0'da shared memory YOK. IPC channel + capability ile
              tüm communication. Future: read-only shared rodata mümkün.

3. Task crash → restart vs isolate:
   Mevcut policy engine SNTM task'lar için yeterli mi?
   → Karar: Mevcut FailureMode enum (Restart/Isolate/Degrade/Failover/
              Alert/Shutdown) yeterli, manifest'ten task başına config.

4. Per-task heap (Tock grant model):
   Task içinde heap kullanmak isterse?
   → Karar: v2.0'da heap YOK. Static allocation only (her task kendi
              static buffer'larına sahip). v3.0'da Tock grant model
              değerlendirilebilir.

5. IOPMP zorunluluğu:
   DMA-capable cihaz olmadan IOPMP gerekli mi?
   → Karar: v2.0 hedef donanımda DMA-capable cihaz YOK (basit MMIO
              senaryo). v2.5+ IOPMP support FPGA bring-up sonrası.

6. CHERI gelecek senaryosu:
   CVA6 CHERI'ye port edildiğinde SNTM ne kadar değişir?
   → Karar: SNTM PMP coarse → CHERI fine üst katman. SNTM core
              dokunulmaz, manifest'e cheri_capabilities eklenir.

7. Task binary signing:
   Her task ayrı imza mı, tek bütün image imza mı?
   → Karar: Tek image imzası (kernel + tüm task'lar). Task-bazlı
              imzalama AMCI cross-hart task migration'da değerlendirilir.

8. WASM ne zaman çıkar:
   v1.5 feature-gate yeterli mi, v2.0 tamamen sil mi?
   → Karar: v2.0 SNTM working olduğunda wasmi dependency tamamen silin.
              ed25519-dalek → ed25519-compact ile alloc da çıksın.
```

---

## 15. Referanslar

### Araştırma kaynakları:

1. **Hubris** — Oxide Computer, Rust microkernel:
   [hubris.oxide.computer](https://hubris.oxide.computer/),
   [github.com/oxidecomputer/hubris](https://github.com/oxidecomputer/hubris)

2. **Tock OS** — RISC-V PMP Rust microkernel:
   [tockos.org](https://tockos.org/documentation/design/),
   [book.tockos.org](https://book.tockos.org/doc/overview),
   [Antmicro Tock RISC-V PMP](https://antmicro.com/blog/2024/10/support-for-veer-el2-with-user-mode-and-pmp-in-tock-os)

3. **Muen** — SPARK/Ada separation kernel:
   [muen.codelabs.ch](https://muen.codelabs.ch/),
   [AdaCore Muen](https://www.adacore.com/academia/projects/muen-project)

4. **ARINC 653** — Avionic partition standard:
   [Wikipedia](https://en.wikipedia.org/wiki/ARINC_653),
   [Wind River ARINC 653](https://www.windriver.com/solutions/learning/arinc-653-compliant-safety-critical-applications)

5. **seL4** — Capability microkernel formal verification:
   [sel4.org](https://sel4.org/About/fact-sheet.html)

6. **CHERI / CHERIoT** — Hardware capability:
   [CHERIoT Microsoft Research](https://www.microsoft.com/en-us/research/publication/cheriot-complete-memory-safety-for-embedded-devices/)

7. **RISC-V Smepmp** — Enhanced PMP:
   [docs.riscv.org Smepmp](https://docs.riscv.org/reference/isa/priv/smepmp.html)

8. **RISC-V IOPMP** — DMA bus master protection:
   [riscv-non-isa/iopmp-spec](https://github.com/riscv-non-isa/iopmp-spec)

9. **RISC-V PMP** — CPU memory protection:
   [Privileged Spec](https://riscv.github.io/riscv-isa-manual/snapshot/privileged/),
   [InCore PMP deep-dive](https://incoresemi.com/risc-v-memory-protection-diving-deep-into-the-complexities/)

10. **DLR WASM Avionics** — Safety-critical interpreter analysis:
    [DLR ELIB 219593](https://elib.dlr.de/219593/)

11. **LFI** — Lightweight Fault Isolation (ASPLOS 2024):
    [ASPLOS 2024 abstracts](https://www.asplos-conference.org/asplos2024/main-program/abstracts/index.html)

12. **RISC-V Worlds, SmMTT** — System-level isolation drafts (2025-2026)

---

> *"Sipahi SNTM does not interpret code in a sandbox;*
> *it compiles code to native RISC-V and lets the CPU*
> *enforce isolation at every memory access, every cycle,*
> *in hardware — under a build-time-validated manifest policy."*
>
> — v0.2 felsefesi: native execution + verified manifest + multi-region PMP

---

## 16. Değişiklik Audit Trail

### v0.3 → v0.4 (Pre-SNTM coherence, 11 Mayıs 2026)

WASM artefakt envanteri + cleanup kararları + Aşama 1.5 sprint:

```
KATEGORİ                       v0.3 eksik                    v0.4 düzeltme
─────────────────────────────────────────────────────────────────────────
1. WASM-tied artefakt          U-22 feature gate ✓           §11.5 envanter
   envanteri yok                ama orphan code envanteri      ~15 item liste
                                yapılmamış
                                
2. Compute services            COMPUTE_* sabitleri config'de  §11.6 per-service
   akıbet belirsiz              dispatch_compute orphan        karar (sil/taşı/kalir)
                                
3. WASM Kani proof              200 proof iddiası ama         §11.7 inventory:
   inventory yok                 ~15 sandbox-tied               198 (v1.5) → 185 (v2.0)
                                
4. Pre-SNTM cleanup sprint     SNTM Phase 1 doğrudan          §12 Aşama 1.5:
   tanımlı değil                 başlıyordu (kirli yatakta)     U-22.5 10 görev ~3 saat
                                                                v1.1.1 tag

EKLENEN BÖLÜMLER (v0.4):
§11.5  WASM Artefakt Temizleme Kararları (envanter + karar matrisi + justify)
§11.6  Compute Services Akıbeti (4 service per-decision)
§11.7  WASM Kani Proof Inventory (gate/keep/delete)
§12 Aşama 1.5  Pre-SNTM Cleanup Sprint (U-22.5, 10 görev, v1.1.1 tag)

Sonuç: SNTM Phase 1 (U-23) artık temiz baseline'dan başlıyor.
       Pre-SNTM cleanup 3 saatlik mini sprint, U-23'ten önce çalıştırılır.
```

### v0.2 → v0.3 (Codex 2. review, 10 Mayıs 2026)

Codex'in 5 implementation-öncesi kritik bulgusu:

```
KATEGORİ                       v0.2 eksik                   v0.3 düzeltme
─────────────────────────────────────────────────────────────────────────
1. is_valid_user_ptr           region.base + size çıplak    checked_add
   overflow check               (overflow risk)               defansif (§5.2)

2. PMP entry hesabı            "4-5 entry" karışık          §4.5.1 PMP Packing
                                NAPOT vs TOR muğlak           Algorithm — Rust kodu

3. PMP priority                yok — shadow attack          §4.5.2 PRIORITY
   invariant                    açık                         INVARIANT — kernel low
                                                              entry, task high entry
                                                              + manifest validator

4. Context switch              implicit "no race"           §4.5.3 ATOMICITY
   atomicity                                                  invariant — DENY stage
                                                              + 3-CSR-write sequence
                                                              + TLA+ reference

5. Task ABI                    _start + sp + mepc           §4.8 ABI SPEC (full):
                                                              gp/tp policy, linker.ld
                                                              template, relocation,
                                                              small-data, panic,
                                                              global init, build flags,
                                                              boot failure diagnostic

EKLENEN BÖLÜMLER (v0.3):
§4.5.1 PMP Packing Algorithm (Rust pseudo-code)
§4.5.2 PMP Priority Invariant + manifest validator priority check
§4.5.3 Context Switch PMP Reload Atomicity + DENY stage zorunluluğu
§4.8   Task ABI Specification (9 alt başlık, build template, diagnostic)
```

### v0.1 → v0.2 (Codex 1. review, 3 Mayıs 2026)

```
KATEGORİ                      v0.1 hata                    v0.2 düzeltme
────────────────────────────────────────────────────────────────────────
Tock scheduler               "cooperative"                 "preemptive RR"
PMP kernel data koruma       "imkansız"                    "policy-dependent"
is_valid_user_ptr            stack-only                    multi-region + Access
ELF loader                   kernel-side parse             build-time flat
PMP entry budget             implicit                      açık hesap (8/16/64)
DMA/IOPMP                    yok                           §6.2 ayrı katman
Formal verified              ✓                             "verifiable friendly"
Safety-cert ready             ✓                             "WASM'dan iyi, yetersiz"
SFI/LFI                      alternatif                    opsiyonel ek katman
CHERI                        hibrit                        validated future direction

EKLENEN BÖLÜMLER (v0.2):
§2 Literatür bağlamı (Hubris, Tock, Muen, ARINC 653, seL4)
§4.5 PMP entry budget — açık hesap
§4.7 Boot sequence v0.2
§5 Multi-region user pointer validation (Access enum)
§6.2 DMA/IOPMP ayrı katman
§9 Formal verification SINIRLARI (Codex'in haklı kritiği)
§10 Dürüst kazanım/kayıp tablosu
§11 Evrim yolu (v1.1 → v3.5)
§14 Açık sorular (8 madde, kararlarıyla)
§16 Bu changelog
```

### Implementation Readiness — v0.4

```
Sprint U-22.5 (Pre-SNTM cleanup) ★ YENİ v0.4         Hazır ✓
Sprint U-23 (sipahi_api + tasks/ workspace)         Hazır ✓
Sprint U-24 (sipahi.toml + sntm-validate)           Hazır ✓
Sprint U-25 (multi-region PMP profile)               Hazır ✓
Sprint U-26 (sntm-pack + task loader)                Hazır ✓
Sprint U-27 (two-task demo)                          Hazır ✓
Sprint U-28 (FPGA bring-up)                          Donanım bekliyor
Sprint U-29 (WASM tamamen sil + ed25519-compact)     Hazır ✓

Sprint sırası:
  1. U-22.5  (~3 saat)   Pre-SNTM cleanup    → tag v1.1.1
  2. U-23    (~1 hafta)  sipahi_api Phase 1
  3. U-24    (~1.5 h.)   Manifest + validator
  4. U-25    (~1.5 h.)   Multi-region PMP
  5. U-26    (~1.5 h.)   sntm-pack + loader
  6. U-27    (~1 hafta)  Two-task demo
                          → tag v2.0-rc
  7. U-28    (hw dep.)   FPGA bring-up
  8. U-29    (~1 gün)    WASM tamamen sil
                          ed25519-compact
                          → tag v2.0

Blocker: Hiç yok. U-22.5 ile bugün başlayabilirsin.
```

---

## 17. SNTM-SAFE: Native Task Security Leap (YENİ — v0.6)

> **Kaynak:** Codex önerisi (12 Mayıs 2026) + analiz sonrası phased rollout.
> **Felsefe:** WASM'in runtime VM sandbox modeli yerine **build-time certified
> native partition** modeli. Güvenlik runtime'da pahalı kontrolle değil,
> image oluşmadan önce yanlış task'ın sisteme girememesiyle sağlanır.

SNTM v1.5 base implementation'ı (§4-§9) WASM'in fonksiyonel yerini alır.
SNTM-SAFE (§17) WASM'in **güvenlik garantilerinin native eşdeğerini** kurar:
WASM linear memory + opcode validation → Safe Rust + binary verifier + PMP.

### 17.1 Threat Model: Arbitrary Native Binary YOK

```
WASM modeli:                     SNTM-SAFE modeli:
─────────────────────────────────────────────────────────────
Untrusted bytecode               Build-controlled Safe Rust
  ↓ VM interpreter                 ↓ task-lint
  ↓ runtime validation             ↓ binary verifier
  ↓ linear memory                  ↓ manifest validator
                                   ↓ task certificate (signed)
  Runtime cost: HIGH               Runtime cost: ~PMP reload only
  TCB: interpreter + VM (~30K)     TCB: kernel + verifier (~10K + ~1.5K)
```

**KRİTİK FARK:** SNTM arbitrary task binary çalıştırmaz. **Build pipeline'dan
geçmemiş binary asla Sipahi image'a giremez.** Bu kısıtlama WASM'ın esnekliğini
kaybeder ama Sipahi hedef workload'ı (safety-critical avionics) için bunu
zaten yapmıyoruz — her task build pipeline'dan geçer.

### 17.2 Safe Native Profile

DAL-A logic task'ları için zorunlu Rust profili:

```rust
#![no_std]
#![no_main]
#![forbid(unsafe_code)]
```

Manifest seviyesinde task-lint tool ek yasaklar uygular:

| Yasak | Sebep | Gerekirse |
|-------|-------|-----------|
| `unsafe` blocks | UB risk | Manifest'te `trust_tier = "unsafe"` + DAL waiver |
| `extern crate alloc` | Heap → deterministik değil | Manifest'te `alloc_required = true` + waiver |
| `core::arch::asm!` | Privileged instruction risk | `trust_tier = "asm"` waiver |
| `extern "C"` (FFI) | Black-box dep | Manifest whitelist |
| Recursion | Stack bound belirsiz | Açık `recursion_bound = N` |
| `dyn Trait` / fn ptr | Indirect call → CFI risk | Whitelist'li fn ptr table |
| `panic = unwind` | .eh_frame TCB | Cargo.toml'da zorunlu `abort` |
| Global runtime init | Boot-time UB | `.init_array` linker DISCARD |
| F/D float instr | RV64IMAC dışı | Hard-disable |
| `core::sync::atomic` | Multi-hart sync semantics | Manifest'te explicit allow list |
| MMIO direct cast | Type safety yok | Generated `mmio::*` typed wrapper |

**Trust tier sistemi:**

```toml
[tasks.logic_task]
trust_tier = "safe"          # default — yasaklar tam aktif

[tasks.uart_driver]
trust_tier = "trusted_unsafe" # driver — unsafe + asm + mmio direct
waiver_reason = "MMIO volatile r/w requires unsafe"
dal_level    = "B"            # DAL-A safe tier'da değil
```

**Niye Bu Çalışır:** Safe Rust subset'i + no heap + no recursion + no
indirect call → call graph determinize → static stack analyzer çalışır,
WCET hesabı kanıtlanabilir, formal verify scope geniş.

### 17.3 SNTM Binary Verifier

**Tool:** `riscv-bin-verify` (yeni, ~1500 LOC, Rust)

Built RISC-V ELF dosyasını manifest kurallarına göre denetler.
**Runtime maliyeti SIFIR** (build-time check).

```
Reject kuralları:
─────────────────────────────────────────────────────────────
1. Privileged instructions
   - csrw/csrr (CSR access)
   - mret/sret/uret
   - sfence.vma / sfence.w.inval
   - wfi (M-mode only)
   - Note: U-mode'da bunlar zaten trap → kernel illegal_instr
     Verifier DEFENSE-IN-DEPTH (image'a hiç girmesin)
   
2. F/D float instructions
   - flw/fsw/fadd/fsub/fmul/fdiv/fsqrt
   - fcvt.s.*/fcvt.d.* family
   - RV64IMAC dışı extension
   
3. Forbidden sections
   - .got / .got.plt (dynamic linking)
   - .plt (PLT trampolines)
   - .eh_frame / .eh_frame_hdr (unwinding)
   - .init_array / .fini_array (global init)
   - .ctors / .dtors
   - Writable + executable section (W^X violation)
   
4. Relocation residue
   - R_RISCV_RELAX (linker relaxation kalıntısı)
   - PIC/PIE relocations
   - Cross-region external symbol refs
   
5. Region boundary violations
   - .text symbol → .data range (executable data risk)
   - .data symbol → .text range (writable text risk)
   - Manifest region dışı address
   
6. Indirect call hedef whitelist (v1.7+, opsiyonel)
   - Verifier jalr hedeflerini taramaya çalışır
   - SCOPE: kernel range içine jalr → REJECT (CFI defense)
   - Full forward-edge CFI v2.5+ (Zicfilp)
```

**LFI inspirasyonu:** LFI runtime verifier %7 overhead bildiriyor. SNTM
build-time verifier ise 0 runtime cost. LFI'nin algoritmik fikirleri
(forbidden opcode scan, region boundary check) build-time'a taşınabilir.
Referans: https://zyedidia.github.io/papers/lfi_asplos24.pdf

### 17.4 SNTM Task Certificate

Her task için build-time **kanıt paketi** üretilir:

```rust
#[repr(C)]
pub struct TaskCertificate {
    // Kimlik
    pub task_id:        u8,
    pub task_name_hash: [u8; 32],  // BLAKE3(task name)
    
    // Tedarik zinciri
    pub source_commit:  [u8; 32],  // git commit hash (sntm-validate inject)
    pub toolchain_hash: [u8; 32],  // Rust nightly version + targets hash
    pub manifest_hash:  [u8; 32],  // sipahi.toml hash
    
    // Build-time invariants
    pub pmp_profile_hash:   [u8; 32],  // PMP_PROFILES[task_id] hash
    pub allowed_syscalls:   u8,         // bitmap: 5 syscall × bit
    pub allowed_channels:   [u8; 8],   // channel ID list
    pub allowed_mmio:       [Range; 4], // MMIO base + len
    pub max_stack_bytes:    u32,        // cargo-call-stack çıktısı
    pub forbidden_opcode_scan: bool,    // verifier verdict
    pub unsafe_count:       u16,        // task-lint count
    
    // Binary sections
    pub text_hash:   [u8; 32],
    pub rodata_hash: [u8; 32],
    pub data_hash:   [u8; 32],
    
    // Generated proofs
    pub kani_proof_ids: [u32; 16],     // task-specific Kani harness IDs
    
    // Format version
    pub abi_version: u32,              // SNTM-SAFE cert format
}
```

**Kernel runtime davranışı:**
- Kernel certificate field'larını **tekrar hesaplamaz**
- Sadece image signature (ed25519) verify edilir
- Certificate parsing kernel'de YOK — opaque blob
- Forensics tool host-side certificate inspect ederek `image hash != cert hash` durumunu yakalar
- Boot-time cost: ed25519 verify (mevcut zaten ~3ms)

**Sertifikasyon değeri:** DAL-A audit'inde "her task'ın kanıt paketi var,
kim/ne/nasıl/ne zaman build edildi belge" sorularını **tek dosya** ile
yanıtlar.

### 17.5 Static Local Capability Table — EN BÜYÜK NET WIN

**Mevcut v1.0 sorunu:**

```
Local task → kernel resource erişimi:
  1. Token cache lookup (~10c hit, ~400c miss)
  2. BLAKE3 keyed hash (cache miss path)
  3. ct_eq_16 verify
  4. Owner check
  5. Nonce check
  6. Expiry check
  
MAC_KEY attack surface: provision_key'den boot'ta yazılır,
SingleHartCell'de tutulur, leak risk var.
```

**SNTM-SAFE çözümü:**

```rust
// Build-time generated: src/kernel/capability/local_cap_table.rs
//
// Manifest'ten sntm-validate üretir. Compile-time const, kernel mutable
// değil. Caller task_id zaten scheduler context'inde authoritative.

pub static LOCAL_CAP_TABLE: [[CapAction; MAX_RESOURCES]; MAX_TASKS] = [
    // Task 0 (logic_task) — read sensor, write actuator
    [
        CapAction::ReadOnly,    // resource 0: sensor_channel
        CapAction::WriteOnly,   // resource 1: actuator_channel
        CapAction::None,        // resource 2: uart (deny)
        // ...
    ],
    // Task 1 (uart_driver) — UART R/W
    [
        CapAction::None,
        CapAction::None,
        CapAction::ReadWrite,   // resource 2: uart (grant)
        // ...
    ],
];

#[inline(always)]
pub fn local_cap_check(
    caller_task_id: u8,
    resource_id: u8,
    action: CapAction,
) -> bool {
    if caller_task_id as usize >= MAX_TASKS { return false; }
    if resource_id as usize >= MAX_RESOURCES { return false; }
    // ~5c: 2 bounds + 1 array load + 1 compare
    LOCAL_CAP_TABLE[caller_task_id as usize][resource_id as usize]
        .allows(action)
}
```

**Performans karşılaştırma:**

| | v1.0 MAC path | SNTM-SAFE static | Hızlanma |
|---|---|---|---|
| Cache hit | ~10c | ~5c | 2× |
| Cache miss (BLAKE3) | ~400c | ~5c | **80×** |
| MAC_KEY surface | var | yok | — |
| Nonce replay risk | mevcut | yok (no token) | — |
| Cross-hart capability | aynı kalır | MAC + epoch + sig (kullanılır) | — |
| External/persistent token | aynı kalır | MAC kullanır | — |

**KRİTİK:** MAC sistemi **kaldırılmıyor**, sadece **local same-image
task → kernel resource** path'inde devre dışı. Cross-hart (AMCI), HSM
provisioned token, external attestation hâlâ MAC kullanır.

**Bu madde tek başına SNTM-SAFE'in değerinin yaklaşık %60'ı.**

### 17.6 Typed IPC + Generated sipahi_api

**Mevcut v1.0 API:**

```rust
syscall::ipc_send(channel_id: u8, msg_ptr: *const IpcMessage) -> isize;
//                ^^ build-time yanlış olabilir
//                                  ^^ size + alignment + type kontrolsüz
```

**SNTM-SAFE API:**

Manifest:

```toml
[[channel]]
id        = 0
producer  = "sensor_task"
consumer  = "logic_task"
message   = "SensorReading"
size      = 16
flow      = "sensor_to_logic"
period_ms = 10                # opsiyonel flow constraint
```

`sntm-validate` üretir (`sipahi_api/src/channels.rs`):

```rust
#[repr(C, align(8))]
pub struct SensorReading {
    pub timestamp_us: u64,
    pub value_q32_32: i64,
}

// Sadece producer task derleyebilir (compile-time check via crate features):
#[cfg(feature = "task_sensor_task")]
pub fn send_sensor_reading(msg: &SensorReading) -> Result<(), IpcError> {
    const CHANNEL_ID: u8 = 0;
    syscall::ipc_send(CHANNEL_ID, msg as *const _ as *const IpcMessage)
}

// Sadece consumer task derleyebilir:
#[cfg(feature = "task_logic_task")]
pub fn recv_sensor_reading() -> Option<SensorReading> {
    // ...
}
```

**Kazanım:**
- Channel ID hatası → compile fail (CHANNEL_ID const, mismatch typo yakalanır)
- Wrong message size → compile fail (struct repr(C) fixed)
- Wrong producer/consumer → compile fail (cfg feature gate)
- Pointer validation kernel-side basitleşir (size = sizeof::<MessageType>())

**Hubris analojisi:** Hubris IPC API'leri da manifest-driven generated.
Felsefe: "small kernel, typed boundary, no opaque blob". Bizim için aynı.
Referans: https://hubris.oxide.computer/reference/

### 17.7 Stack Safety: Build-Time Bound + PMP Guard

```
Stack Safety Pipeline:
─────────────────────────────────────────────────────────────
1. cargo +nightly build -Z emit-stack-sizes
   → her function'ın stack frame boyutu bilinir

2. cargo-call-stack analiz
   → static call graph + max_stack_bytes
   → indirect call yoksa (Safe Native Profile) güvenilir sonuç

3. Manifest'te stack_size tanımlı (8KB default)
   sntm-validate: max_stack_bytes ≤ stack_size kontrol
   FAIL → compile fail

4. NAPOT PMP per-task stack region (mevcut v1.0)
   Stack overflow → PMP NO_MATCH → trap → policy escalation

5. TOR layout için explicit guard region (v0.5 §4.3 fix)
   Adjacent region'a underflow yazımı → PMP fault

6. Watermark/scribble (opsiyonel, runtime observation)
   debug-boot build'de stack zero-fill, periyodik watermark check
```

`cargo-call-stack` sınırlamaları:
- Indirect call'ları yakalayamaz → Safe Native Profile dynamic dispatch
  yasakladığı için bu sorun olmuyor
- External symbol stack size'ı bilinmez → no FFI yasağı
- Recursion → açık waiver gerekli (bound annotation)

Referans: https://github.com/japaric/cargo-call-stack

### 17.8 CFI Roadmap: Verifier → Hardware → CHERI

```
Faz             Mekanizma                Runtime cost    Hardware ger.
─────────────────────────────────────────────────────────────────────
v1.7 (SAFE)    Binary verifier          0c              -
               (forbidden jalr targets)

v2.5           Zicfilp landing pad      ~1-2c per jalr  CVA6-CFI fork
               Zicfiss shadow stack     ~2-3c per ret   yoksa skip

v3.0+          Software-emulated CFI    ~5-10c          (fallback)
               for non-CFI hardware

vfuture        CHERIoT hardware caps    deterministic   CHERIoT-Ibex
                                        memory safety   veya custom
```

**Bugün karar:** v1.7'de binary verifier indirect call target tarama
**SADECE manifest-allowlisted** olur. Hardware CFI v2.5'te değerlendirilir
(CVA6-CFI çalışması olgunlaşırsa).

Referanslar:
- RISC-V CFI spec: https://docs.riscv.org/reference/isa/unpriv/unpriv-cfi.html
- CVA6-CFI 2026: https://arxiv.org/abs/2602.04991
- CHERIoT: https://www.microsoft.com/en-us/research/publication/cheriot-complete-memory-safety-for-embedded-devices/

### 17.9 Runtime Cost Table

| Madde | Build cost | Runtime cost | Net hız etkisi |
|-------|------------|--------------|----------------|
| Safe Native Profile | doc + task-lint | 0 | 0 |
| Binary Verifier | yeni tool ~1500 LOC | 0 | 0 |
| Task Certificate | yeni tool ~500 LOC | ed25519 boot-time (mevcut) | 0 |
| **Static Cap Table** | manifest codegen | **-5..-395c (faster)** | **HIZLANDIRIR** |
| Typed IPC | codegen tool | aynı (kernel ipc_send) | 0 |
| Stack Analyzer | CI integration | 0 | 0 |
| CFI verifier (v1.7) | binary verifier ext | 0 | 0 |
| sfence.vma (v0.5 fix) | yok | +3-5c per context switch | minor regression |
| **TOPLAM** | ~25-32 gün dev | **NET HIZLANIR** | + |

**Sonuç:** SNTM-SAFE güvenlik substantial artırırken runtime'da **net
hızlanma** sağlıyor. Tek küçük regression `sfence.vma` (zaten spec
compliance, SNTM'den bağımsız).

### 17.10 CI Gates

`scripts/sntm_safe_gate.sh` (yeni, U-22.5 sprint gate'in genişletilmiş hali):

```bash
[1/10] cargo check (her task)
[2/10] task-lint (Safe Native Profile uygulama)
[3/10] cargo +nightly build --release (her task)
[4/10] riscv-bin-verify (forbidden opcode + section + relocation)
[5/10] cargo-call-stack (stack bound ≤ manifest)
[6/10] sntm-validate (manifest invariants — region overlap, PMP budget)
[7/10] Static cap table generated codegen check
[8/10] Typed IPC API codegen check
[9/10] Task certificate generate + ed25519 sign
[10/10] Image assemble (kernel + tasks + cert) + final ed25519
```

Tüm 10 gate PASS → image deploy edilebilir. Tek bir gate FAIL → image
imzalanmaz, deploy bloklanır.

### 17.11 SNTM Certificate Mesh — Tek Source-of-Truth

`sipahi.toml` manifest'inden 10 artifact üretilir:

```
sipahi.toml manifest
  ↓ sntm-validate
  ↓
  ├─ 1. Rust constants (src/common/config_generated.rs)
  ├─ 2. PMP_PROFILES (src/kernel/pmp/generated.rs — §4.5.4)
  ├─ 3. LOCAL_CAP_TABLE (src/kernel/capability/cap_generated.rs)
  ├─ 4. Typed sipahi_api (sipahi_api/src/channels.rs — §17.6)
  ├─ 5. Per-task linker scripts (task_layouts/*.ld)
  ├─ 6. Binary verifier rules (verify_rules.toml)
  ├─ 7. Kani harness templates (verify/tasks/*.rs)
  ├─ 8. TLA+ constants (Tla+/SipahiSNTM_generated.cfg)
  ├─ 9. Task certificates (one per task, *.cert.bin)
  └─10. Blackbox metadata schema (blackbox_schema.toml)
```

**Tek manifest hatası → en az bir artifact mismatch → build fail.**

Validation matrisi:

| Hata | Yakalandığı yer |
|------|-----------------|
| Region overlap | sntm-validate compile fail |
| PMP budget overflow | sntm-validate compile fail |
| Unsafe code (safe tier task) | task-lint fail |
| Forbidden opcode in binary | binary verifier fail |
| Stack > manifest bound | stack analyzer fail |
| Wrong IPC endpoint | generated API compile fail |
| Capability mismatch | Kani proof fail OR runtime PMP fault |
| Channel size mismatch | typed IPC compile fail |
| Toolchain hash mismatch | certificate signature fail |
| Runtime corruption | PMP shadow / blackbox detect |

Bu, AMCI doctrine "tool reaktif, requirement proaktif"in SNTM
implementasyonu. Her hata sınıfının bir tool'u var, hiçbir hata
runtime'a kadar gelmez.

### 17.12 Phased Rollout (§12 ile bağ)

| Faz | Sürüm | İçerik | Süre tahmini |
|-----|-------|--------|--------------|
| Base SNTM | v1.5 | §4-§9: WASM out + native task çekirdeği | 3-4 hafta |
| SAFE-1 | v1.6 | §17.2 Safe Native Profile + task-lint | ~1 hafta |
| SAFE-2 | v1.7 | §17.5 Static Cap Table + §17.6 Typed IPC | 1.5 hafta |
| SAFE-3 | v1.8 | §17.3 Binary Verifier + §17.4 Task Certificate | 2-3 hafta |
| SAFE-4 | v1.9 | §17.7 Stack Analyzer + §17.10 CI Gates full | 1 hafta |
| CFI | v2.5+ | §17.8 hardware Zicfilp/Zicfiss (CVA6 olgunluğa) | TBD |

**Niye phased:** Base SNTM (v1.5) tek başına ~3 hafta + risk. SAFE
katmanlarını v1.5'e sıkıştırmak iki sprint'i karıştırır, regression
yakalanması zorlaşır. SAFE-2 (static cap table) en yüksek değer/efor
oranı → v1.7'de öncelik.

### 17.13 SNTM-SAFE'in Sınırları (Dürüst Beyan)

```
SNTM-SAFE NE DEĞİL:
─────────────────────────────────────────────────────────────
× Arbitrary native binary sandbox (≠ WASM)
× Untrusted code execution platform
× WASM kadar formal-verified isolation (interpreter TCB yok ama
  binary verifier TCB var — küçük ama mevcut)
× Hardware-rooted memory safety (CHERIoT v3.0+ hedefi)
× Multi-vendor task ecosystem (build pipeline single-source)

SNTM-SAFE NE:
─────────────────────────────────────────────────────────────
✓ Build-time certified Safe Rust task partitioning
✓ PMP-isolated execution + binary-verified privileges
✓ Static capability + typed IPC + bounded stack
✓ DAL-A sertifikasyon-dostu artifact chain
✓ Net runtime hızlanma (static cap table)
✓ Sipahi v1.0 doctrine ile %100 uyumlu
✓ Codex review'da onaylanan sertleştirme yolu
```

**Hedef workload:** Single-vendor safety-critical avionics. Çoklu-tedarikçi
açık ekosistem için SNTM-SAFE uygun **değildir** — o senaryoda WASM veya
CHERIoT daha doğru.

---

## 18. SNTM Sprint Completion Gate (YENİ — v0.7)

> **Amaç:** SNTM 8+ hafta sürecek multi-sprint geçiş. "Sprint bitti sandık
> ama yarım kalmış" durumunu mekanik olarak engelle.
> **Operasyonel ref:** `scripts/sntm_sprint_gate.sh` (U-22 gate extend).

### 18.1 Definition of Done — Per Task Type

| Task tipi | Tamam sayılma koşulu |
|-----------|----------------------|
| **Kod (kernel)** | `make check` 0 warn + Kani proof eklediyse PASS + production smoke NF=0 |
| **Kod (task)** | `cargo build -p <task>` PASS + task-lint PASS (SNTM-SAFE faz) + QEMU boot ediyor |
| **Tool (host)** | `cargo build -p <tool>` + integration fixture geçiyor + manifest örneği round-trip |
| **Feature gate** | Default-off: production build smoke aynı (NF=0, FATAL=0); Default-on: yeni test'ler PASS |
| **Manifest** | sntm-validate ROUND-TRIP geçiyor (parse → validate → codegen → recompile) |
| **Doküman** | Kod referansları grep ile bulunabiliyor; SADECE doc değişikliği commit'i KABUL DEĞİL (test eşliği şart) |
| **Kani proof** | `cargo kani` PASS + count regression yok (≥ baseline) |
| **TLA+ spec** | `bash Tla+/run_tlc.sh` 7/7 PASS (yeni spec eklendiyse 8/8) |

### 18.2 Required Commands

Mevcut **U-22 sprint gate** (`scripts/sipahi_sprint_gate.sh`) zorunlu — değişmez.
SNTM sprint'lerinde **+** ek komutlar (`scripts/sntm_sprint_gate.sh`):

```bash
# Baseline (mevcut, değişmez):
bash scripts/sipahi_sprint_gate.sh
  [1/8] cargo check
  [2/8] make check (clippy -D warnings)
  [3/8] cargo kani (200+/200+ PASS)
  [4/8] make build
  [5/8] production NF/FATAL check
  [6/8] self-test ALL TESTS PASSED + [FAIL] grep clean
  [7/8] no new TODO/FIXME/HACK/XXX (git diff)
  [8/8] version banner consistency

# SNTM ek (yeni, scripts/sntm_sprint_gate.sh):
  [E1] cargo build -p sipahi-api (varsa)
  [E2] cargo build -p task_*  --target riscv64imac (varsa)
  [E3] task-lint her safe-tier task için (SAFE-1+)
  [E4] sntm-validate --manifest sipahi.toml (SNTM v1.5+)
  [E5] sntm-pack --manifest sipahi.toml (SNTM v1.5+)
  [E6] sntm-bin-verify target/tasks/*.elf (SAFE-3+)
  [E7] cargo-call-stack max ≤ manifest stack_size (SAFE-4+)
  [E8] timeout 8s make run-sntm — production SNTM boot, NF=0
  [E9] Negative test'ler ALL PASS (E.4 listesi)
```

**Graceful degrade:** Her ek komut sprint phase'ine göre **opsiyonel**.
SAFE-3'ten önce `sntm-bin-verify` yoksa SKIP, sprint sonu raporda not.

### 18.3 Feature Gate Policy

İki umbrella flag yeterli (Codex'in 4-flag öneri reddedildi — granular spam):

```toml
# Cargo.toml
[features]
sntm       = []           # SNTM v1.5 base — task loader, typed IPC, manifest
sntm-safe  = ["sntm"]     # SAFE-1..4 katmanları — task-lint enforce, verifier, cert
```

**Kurallar:**

1. **SNTM kapalı (default):** Production build U-22 baseline'ı **tıpatıp** üretir
   - sandbox/ kaldırıldıktan sonra bile WASM-free production aynı
   - SNTM kodu compile-out, binary size regression yok
2. **SNTM açık ama yarım:** Default features'ta YASAK. Sadece self-test/dev build'de
3. **`scripts/feature_matrix.sh`** her sprint sonu güncellenir: yeni sntm/sntm-safe kombinasyonu eklenir, 8+ kombinasyon PASS
4. **Sub-feature flag** sadece **gerçekten bağımsız toggle edilebilen** parça için
   (örn. `cfi-hardware` Zicfilp olduğunda). Önceden flag yaratma

### 18.4 Negative Test Requirement — MECHANICAL ENFORCEMENT (v0.7)

> **Önceki gap (kabul edilmiş):** §18.4 v0.7'nin ilk halinde sadece textual
> rule'du. Lazy developer yeni feature ekleyip test eklemeyebilir, gate
> "self-test PASS" görüyor diye geçerdi. **v0.7 fix:** `coverage.toml`
> mechanical enforcement.

**Coverage.toml — feature ↔ test/proof mapping:**

Repo root'taki `coverage.toml` her feature flag için zorunlu test/proof
isimlerini listeler:

```toml
[feature.fast-crypto]
required_negative_tests = [
    "test_token_owner_mismatch_neg",
    "test_cross_task_pointer_rejected",
]
required_kani_proofs = [
    "token_owner_mismatch_always_rejected",
    "ct_eq_16_same_input_true",
]

# Henüz test'i olmayan feature için explicit deferred + reason + target:
[feature.fast-sign]
deferred         = "test+proof"
deferred_reason  = "Ed25519 ed25519-dalek crate'inde, FPGA negative test sprint'ine ertelendi"
deferred_target  = "v2.0 secure boot integration sprint"
```

**`scripts/check_coverage.sh` 4 invariant enforce eder:**

1. **Symmetry:** Cargo.toml'daki her `[features]` flag → `coverage.toml`'da
   `[feature.NAME]` entry olmalı. **Yoksa FAIL.**
2. **Stale guard:** coverage.toml'da entry var ama Cargo.toml'da feature
   yok → stale entry. **FAIL.**
3. **Name existence:** `required_negative_tests`/`required_kani_proofs`
   isimleri repo'da `fn NAME(` olarak bulunmalı. **Yoksa FAIL.**
4. **Deferred discipline:** `deferred = "..."` field'ı varsa
   `deferred_reason` + `deferred_target` **zorunlu**. Yoksa FAIL.

**Bu gate'in sınırları (dürüst beyan):**

> Bu **GERÇEK COVERAGE KANITI DEĞİL**. İsim-tabanlı mekanik guard'dır.
> Amacı: lazy bypass yakalamak (feature eklendi, test ismi listede yok →
> patla). **Test body adequacy** = manuel review işi. Test'in gerçekten
> ne tested ettiği, edge case'leri kapsayıp kapsamadığı bu gate'in
> kapsamı dışı.

**Test edildi (v0.7 baseline):**

- ✓ Symmetry check: fake feature eklenince yakalandı
- ✓ Stale check: coverage entry sil → fail
- ✓ Name existence: yanlış proof ismi → fail
- ✓ Deferred discipline: reason/target eksikse → fail
- ✓ Mevcut Sipahi v1.0 baseline'ı PASS ediyor (12 feature mapped:
  2 active + 7 deferred + 3 non-safety)

**Her yeni primitive için zorunlu kalan textual kural:**

| Primitive | Pozitif test | Negative test (ZORUNLU) |
|-----------|--------------|--------------------------|
| Task PMP region | Task kendi region'ında çalışır | Diğer task'ın region'ına yazma → trap |
| local_cap_check | Allow path PASS | Deny path REJECT (CAP_TABLE NONE) |
| typed IPC channel | Producer send → consumer recv | Wrong producer task → compile fail |
| Binary verifier | Clean ELF PASS | Forbidden opcode injection → reject |
| sntm-validate | Valid manifest → codegen | Region overlap → fail |
| Task certificate | Valid sig → boot | Tampered binary → boot halt |
| Stack analyzer | Bounded stack PASS | recursion → analyzer fail |

Negative test PASS = `[FAIL]` marker self-test'te **GÖRÜLMEZ** (grep clean) +
expected reject path log mesajı **GÖRÜLÜR** (`[OK] Negative: ...` pattern).

**U-21 disciplini:** Test-first — negative test fix'ten **ÖNCE** yazılır, RED görür,
fix sonrası GREEN olur. U-22 G1-G6 regression test'leri bu modelin örneği.

### 18.5 Carry-Forward Template

Sprint sonu raporu **standart format** (markdown bloğu kopyala):

```markdown
## Sprint <NAME> — Final Report

### Completed (with evidence)
- G<N>: <task> — <file:line> + test path + commit hash
- ...

### Partially completed
- G<N>: <task> — <% complete> [<file:line>]
  - Neden yarım: <root cause, not symptom>
  - Feature flag arkasında mı: YES (<flag>) / NO
  - Production etkisi: NONE / DEGRADED (<which path>)
  - Sonraki sprint: <name> görev <ID>

### Not implemented
- G<N>: <task>
  - Sebep: scope cut / dependency / blocker
  - Taşındı: <future sprint name>

### Known broken (deferred fix)
- <Issue>: <one-line>
  - Test var mı: <yes/no, path>
  - Tracking ID: <issue ref or "no tracker">
  - Workaround: <if any>
  - Fix sprint hedefi: <sprint name>

### No-Go check (zorunlu)
□ Production NF-free
□ Self-test ALL TESTS PASSED
□ Kani proof count ≥ baseline
□ Yarım feature default-off
□ Manifest validator bypass yok
□ Yeni unsafe task var ise bin-verify çıktısı eklendi
```

**Sprint complete sayılmaz** eğer:
- Carry-forward bölümü doldurulmadı, **veya**
- No-go check'in bir maddesi işaretlenemedi.

### 18.6 No-Go Conditions

Aşağıdakilerin **herhangi biri** geçerliyse sprint **NO-GO**, image deploy
edilmez, v1.X tag atılmaz:

```
NO-GO-1: Production boot fail
         → timeout 8s qemu output'unda NF marker veya FATAL
         → CI: production-smoke job FAIL

NO-GO-2: Self-test [FAIL] marker
         → grep '\[FAIL\]' /tmp/selftest.log nonzero
         → CI: qemu-test job FAIL

NO-GO-3: Kani proof count regression
         → Yeni sprint'te kanıt sayısı düşmüş (silinen proof'lar yerine
           yeni eklenmiş yoksa)
         → CI: kani job sayım kontrolü FAIL

NO-GO-4: SNTM feature yarım ama production'da default-on
         → Cargo.toml default = [..., "sntm", ...] ve sntm = [] yarım
         → CI: feature-matrix job production smoke FAIL

NO-GO-5: Manifest validator bypass
         → İki ayrı invalid manifest sntm-validate PASS dönüyor
         → Negative test: invalid_manifest_rejected MISSING/FAIL

NO-GO-6: Binary verifier eksik + unsafe task kabul ediliyor
         → SAFE-3+ sprint'lerinde unsafe_count > 0 task var, ama
           riscv-bin-verify çıktısı yok veya stale
         → CI: bin-verify job FAIL veya MISSING
```

**No-Go fail** → sprint kapatma engellenmez ama:
- Tag atılmaz (`git tag v1.X` bloklanır)
- CHANGELOG.md'de "Known Issues" bölümü zorunlu
- Sonraki sprint'in **birinci görevi** bu no-go'yu kapatmaktır

### 18.7 Proof/Test Quality Gate (YENİ — v0.7)

> **Tek cümle kural:**
> Bir test/proof, hangi requirement'ı doğruladığını, hangi production
> fonksiyonunu çağırdığını ve hangi hatalı implementasyonda fail edeceğini
> söylemiyorsa **coverage sayılmaz**.

**Üç zorunlu yorum (her yeni test/proof için):**

```rust
// VERIFIES: SNTM-Rx (veya SIPAHI-Rx)
// CALLS:    production_function_names (comma-separated)
// FAILS-IF: hangi hatalı implementasyonda bu test/proof fail eder
#[kani::proof]
fn sntm_kernel_overlap_rejected() {
    // ...
}
```

**Niye 3 yorum:**
- `VERIFIES`: hangi invariant'a bağlı → requirement traceability (DAL-A audit)
- `CALLS`: gerçekten production kod çağırıyor mu (fake/stub proof değil mi)
- `FAILS-IF`: fault model dokümante → testin gerçekten guard'ı koruduğunu beyan

**`coverage.toml`'da requirement bloğu (placeholder şema, SNTM v1.5'te aktif):**

```toml
[requirement.SNTM-R1]
description     = "Task regions must not overlap kernel regions"
required_tests  = ["test_task_kernel_overlap_rejected"]
required_proofs = ["sntm_kernel_overlap_rejected_for_any_region"]
fault_model     = "regions_overlap() always returns false → silent accept"
```

`scripts/check_coverage.sh` doğrular:
1. `required_tests/required_proofs` source'da var mı (mevcut §18.4)
2. **Yeni v0.7:** Grandfather list'te değilse 3-yorum (VERIFIES/CALLS/FAILS-IF) zorunlu
3. **Yeni v0.7:** `[requirement.X]` bloğu için: source'da en az bir `// VERIFIES: X` yorumu olmalı
4. **Yeni v0.7:** `description` + `fault_model` zorunlu field

**Grandfather list (§18.7 muafiyet):**

Pre-2026-05-13 baseline test/proof isimleri `coverage.toml` `[grandfather]`
bölümünde listelenir, 3-yorum kuralından **muaftır**. Sebep: U-18 quality
audit'i geçtiler, kategori sınıflandırması mevcut. Yeni isimler buraya
eklenmez — onlar 3-yorum kuralına tabi.

**Mevcut grandfather (v0.7 baseline, 7 isim):**
- 4 proof: `token_owner_mismatch_always_rejected`, `ct_eq_16_same_input_true`,
  `ct_eq_16_single_byte_diff_false`, `bump_allocator_offsets_never_overlap`
- 3 test: `test_token_owner_mismatch_neg`, `test_cross_task_pointer_rejected`,
  `test_allocator_overflow`

**Light tautology detector — `scripts/check_proof_quality.sh`:**

Mekanik olarak yakalanabilen 7 bariz tautoloji pattern'ını arar:
- `assert!(true)` / `assert!(false)` literal
- `assert!(N == N)` / `assert!(N != M)` sabit literal
- `assert_eq!(X, X)` aynı identifier
- `assert_eq!(N, N)` aynı sabit
- `kani::assume(false)` — proof her zaman skip

Grandfather'da olmayan proof'larda bu pattern'lar warning üretir.
**Sprint gate'i fail etmez** (informational), ama sprint owner FAIL'e
escalate edebilir.

**Bu gate'in sınırları:**

> Bu detector **GERÇEK QUALITY KANITI DEĞİL** — yalancı yorum yine
> mümkün. Amacı: lazy bypass + bariz tautoloji'yi mekanik yakalamak;
> test/proof'un gerçekten meaningful olduğu **manuel review** işi.

**Test edildi (v0.7 baseline):**
- ✓ Mevcut 200 Kani proof PASS (4 grandfather + 196 hiçbiri tautoloji)
- ✓ Fake proof injection (5 tautoloji pattern içeren) — hepsi yakalandı
- ✓ Coverage gate'te yeni isim eklendi 3-yorum yok → FAIL

### 18.8 Implementation Note

`scripts/sntm_sprint_gate.sh` E0/E0b adımları:

```bash
[E0]  Coverage map (lazy bypass + 3-yorum quality)
        bash scripts/check_coverage.sh
[E0b] Proof quality light scan (informational)
        bash scripts/check_proof_quality.sh
[BASELINE]  U-22 sprint gate (mevcut 8-step)
[E1..E9]    SNTM-spesifik (graceful degrade)
```

Script SNTM v1.5'in **G0 öncesi** zorunlu adımı (pre-G0).

---

*Sipahi SNTM v0.7 — Native Rust task'lar + multi-region PMP donanım izolasyonu*
*+ SNTM-SAFE build-time certified partition + Sprint Completion Gate.*
*Build-time validated manifest, kernel ELF parse etmez, IOPMP gelecek-aware.*
*WASM'ın yerini alan, Codex 3-round review + SAFE proposal + operational DoD ile sertleştirilmiş model.*
