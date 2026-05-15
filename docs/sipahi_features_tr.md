# Sipahi — Teknik Özellikler

Bu belge Sipahi deposunun güncel çalışma ağacındaki özellikleri özetler.
Amaç pazarlama yapmak değil; hangi özelliklerin gerçekten kodda bulunduğunu,
hangilerinin kısmi olduğunu ve hangilerinin yol haritasında kaldığını açık
şekilde ayırmaktır.

**Durum:** v1.1.1 + U-23 SNTM Phase 1 çalışma ağacı  
**Hedef:** RISC-V RV64IMAC, QEMU `virt`, single-hart  
**Dil:** Rust `no_std`, bare-metal  
**Doğrulama:** 198 Kani harness, 7 TLA+ model, self-test ve sprint gate scriptleri

Sipahi sertifikalı bir RTOS değildir. DO-178C/DAL-A tarzı bazı tasarım
prensiplerini uygular; ancak sertifikasyon, gerçek donanım WCET raporu,
bağımsız review, requirements traceability, safety case ve tool qualification
gibi ayrı kanıtlar gerektirir.

---

## 1. Çekirdek Mimari

- Kernel Machine mode'da çalışır.
- Task'lar User mode'a `mret` ile düşer.
- S-mode/MMU kullanılmaz; bellek ayrımı PMP ile yapılır.
- Boot, trap entry, timer interrupt ve context switch RISC-V assembly ile
  desteklenir.
- Kernel tek-hart varsayımıyla tasarlanmıştır. Multi-hart/AMCI çalışmaları
  ayrı tasarım dokümanlarında takip edilir; mevcut runtime single-hart'tır.

### Mevcut ana modüller

- `src/arch`: boot, trap, CSR, PMP, CLINT, UART, context switch
- `src/kernel/scheduler`: task tablosu, priority seçimi, budget, watchdog
- `src/kernel/syscall`: syscall ABI, dispatch table, WCET tracking
- `src/kernel/capability`: token, broker, cache
- `src/ipc`: SPSC IPC kanalları ve blackbox recorder
- `src/kernel/policy`: failure policy engine
- `src/sandbox`: feature-gated WASM prototype path
- `sipahi_api`: SNTM task-side API crate
- `tasks/task_hello`: standalone native task scaffold

---

## 2. Privilege ve Trap Modeli

### Var olan özellikler

- Kernel M-mode, task'lar U-mode.
- `mstatus.MPP = U` ve `mret` ile task entry.
- `mscratch` tabanlı trap stack swap.
- `task_trampoline` context switch sonrası fresh task'ı U-mode'a geçirir.
- U-18'de giderilen nested fault problemi sonrası trampoline `mscratch` ve
  user stack invariant'ını restore eder.
- U-19'da task entry öncesi caller-saved register temizliği eklendi.
- `mcounteren = 0`; U-mode timing counter erişimi kapalı.
- `medeleg/mideleg` sıfırlanır; M-only kernel modeli korunur.

### Sınırlar

- Gerçek donanımda trap latency ve PMP davranışı FPGA/silikon üzerinde ayrıca
  ölçülmelidir.
- Multi-hart trap/interrupt modeli henüz runtime'da yoktur.

---

## 3. PMP ve Bellek Koruması

### Var olan özellikler

- Kernel `.text`, `.rodata`, `.data+bss+kernel_stack` ve UART MMIO bölgeleri
  PMP ile korunur.
- Kernel bölgelerinde L-bit kullanılır.
- Task stack'leri `.task_stacks` alanındadır.
- Her context switch'te per-task NAPOT stack entry programlanır.
- Per-task PMP yazımından sonra `sfence.vma zero, zero` uygulanır.
- PMP shadow integrity kontrolü scheduler tick path'inde korunur.
- `is_valid_user_ptr(caller_task_id, ptr, size)` task'a özeldir:
  sadece çağıran task'ın kendi stack aralığını kabul eder.
- Dead/isolated/uninitialized task için pointer valid range yoktur.

### SNTM tarafı

- U-23 itibarıyla SNTM manifest scaffold (`sipahi.toml`) var.
- Multi-region PMP profile tablosu ve manifest'ten generate edilen
  `PMP_PROFILES` henüz yoktur.
- Runtime multi-region PMP reload U-25+ hedefidir.

---

## 4. Scheduler

### Var olan özellikler

- Fixed-priority preemptive scheduler.
- `MAX_TASKS = 8`.
- Task state modeli: Ready, Running, Suspended, Isolated, Dead.
- Priority selection helper'ları Kani proof'larıyla desteklenir.
- `schedule_timer_tick()` ve `schedule_yield()` ayrıdır:
  - timer tick path'i period, budget, watchdog, blackbox tick ve PMP integrity
    gibi state advance işlerini yapar.
  - yield path'i sadece priority select/context switch tarafını çalıştırır.
- Budget accounting `saturating_sub` ile yapılır.
- Watchdog sadece Running task için artar; Ready task CPU almadığı için
  watchdog timeout yemez.
- Degrade/recovery path'inde cooldown bulunur.
- `isolate_task()` task'ı Isolated yapar ve capability'lerini invalidate eder.
- `SYS_EXIT` handler'ı current task'ı isolate edip scheduler'a kontrol verir.

### Sınırlar

- WCET değerleri şimdilik tahmindir; gerçek cycle ölçümü FPGA/silikon üzerinde
  yapılmalıdır.
- Task migration veya multi-hart scheduling yoktur.

---

## 5. Syscall ABI

### Var olan syscall'lar

| ID | Syscall | Durum |
|---:|---|---|
| 0 | `cap_invoke` | capability kontrol |
| 1 | `ipc_send` | non-blocking IPC send |
| 2 | `ipc_recv` | non-blocking IPC receive |
| 3 | `yield` | scheduler'a kontrol bırakma |
| 4 | `task_info` | task state/priority/DAL bilgisi |
| 5 | `exit` | voluntary task termination |

`SYSCALL_COUNT = 6`.

### Var olan korumalar

- O(1) function pointer dispatch table.
- Geçersiz syscall ID -> `E_INVALID_SYSCALL`.
- Syscall return value kernel pointer gibi görünürse `E_INTERNAL` ile sanitize.
- IPC pointer'ları task-specific validation ve alignment check'ten geçer.
- `sys_cap_invoke` argüman truncation riskini kontrol eder.
- `rdcycle` ile syscall WCET last/max ölçümü tutulur.
- `check_wcet_limits()` 6 syscall için limit array'i kullanır.
- `print_wcet_stats()` `SYSCALL_COUNT` ile compile-time uyumlu isim tablosu
  kullanır.

---

## 6. Capability Sistemi

### Var olan özellikler

- 32-byte `Token` yapısı.
- BLAKE3 keyed MAC doğrulama path'i.
- `ct_eq_16` constant-time MAC karşılaştırması.
- 4-slot token validation cache.
- Per-task nonce replay guard.
- Token expiry kontrolü.
- Token owner enforcement: token başka task'a aitse MAC doğru olsa bile
  çağıran task kullanamaz.
- Cache invalidation by token/owner ve task isolate sırasında capability revoke.
- `production-otp` feature path'i production provisioning için stub/extern
  bekler; yanlışlıkla production build yapılmasını link-time engeller.
- `test-keys` development/CI default path'idir.

### Sınırlar

- BLAKE3 v1.x hızlı prototip MAC olarak kullanılıyor. SNTM/CNSA yönünde
  SHA-2/Zknh veya farklı imza/hash planları roadmap'tedir.
- Kriptografik güvenlik Kani ile kanıtlanmaz; Kani burada bounds, ordering ve
  API kullanım invariant'larını kontrol eder.

---

## 7. IPC

### Var olan özellikler

- 8 statik SPSC channel.
- Her kanal 16 slot ve 64-byte mesaj kullanır.
- `AtomicU16` head/tail, Release/Acquire ordering.
- `send` ve `recv` O(1), non-blocking.
- U-16'dan beri channel ownership tablosu vardır:
  producer/consumer boot'ta atanır, sonra `seal_channels()` ile kilitlenir.
- Atanmamış channel default-deny davranır.
- `can_send` / `can_recv` ownership check sağlar.
- CRC32 helper'ları (`set_crc`, `verify_crc`) bulunur.
- IPC send rate limiting tick başına uygulanır.

### Sınırlar

- CRC kullanımı kernel tarafından zorunlu kılınmaz; uygulama/helper çağrısı
  gerekir.
- SPSC model enforced edilir; MPMC yoktur.
- Typed IPC generator SNTM SAFE aşamalarına kalmıştır.

---

## 8. Policy Engine ve Degrade

### Var olan özellikler

- Pure `decide_action(event, restart_count, dal)` fonksiyonu.
- Failure mode seti:
  Restart, Isolate, Degrade, Failover, Alert, Shutdown.
- Failover v1.x'te gerçek hot-standby switch değil; Degrade fallback olarak
  uygulanır ve forensics için ayrıştırılır.
- Policy lockstep: karar fonksiyonu iki kez çalıştırılır; farklı sonuç
  Shutdown'a gider ve blackbox event bırakır.
- `black_box` input/output fence ile compiler CSE riski azaltılır.
- Restart counters saturating davranır.
- Degrade DAL-C/D task'ları durdurur, DAL-A/B önceliği korur.
- Recovery path cooldown ile flapping azaltır.

### Sınırlar

- Gerçek yedek task failover runtime'ı henüz yoktur.
- Degrade/failover davranışlarının sistem düzeyi etkisi uygulama mimarisiyle
  birlikte test edilmelidir.

---

## 9. Blackbox Flight Recorder

### Var olan özellikler

- 8 KB statik circular buffer.
- 128 kayıt, her kayıt 64 byte.
- CRC32-protected record format.
- Monotonic tick kaydı.
- Policy, PMP, watchdog, lockstep, POST warning ve benzeri event'ler loglanır.
- Write position bounds guard bulunur; bozuk pozisyon resetlenir ve OOB write
  engellenir.

### Sınırlar

- Kalıcı storage'a otomatik flush yoktur.
- Multi-hart aggregation yoktur.

---

## 10. WASM Sandbox Durumu

WASM artık ana gelecek yönü değildir; prototype/test path olarak tutulur.

### Var olan özellikler

- `wasm-sandbox` feature arkasında.
- `self-test` feature WASM'i test için açar.
- Wasmi 1.0.9 kullanımı.
- 4 MB arena/bump allocator.
- WASM magic/version/code-section kontrolleri.
- Float opcode rejection heuristic'i.
- 0xFC saturating truncation ve br_table skip tarafı için ek kontroller.

### Sınırlar

- Tam WASM grammar parser değildir.
- Production default build'de WASM path'i kapalıdır.
- SNTM tamamlandıkça WASM path'i azaltılacak/kaldırılacaktır.

---

## 11. SNTM Phase 1

### Var olan özellikler

- `sipahi_api` crate:
  - `Error` enum ve kernel return mapping.
  - 64-byte `ipc::Message`.
  - `cap_invoke`, `ipc_send`, `ipc_recv`, `yield_cpu`, `task_info`, `exit`
    syscall wrapper'ları.
- `SYS_EXIT = 5` kernel syscall handler.
- `tasks/task_hello` standalone native task scaffold:
  - `_start`
  - yield loop
  - panic -> `syscall::exit(255)`
  - task-scoped linker config
- `sipahi.toml` manifest scaffold.
- `sntm` ve `sntm-safe` feature flags default-off.
- Kernel crate `sipahi_api` dependency almaz; task crate doğrudan path
  dependency kullanır. Bu mimari ayrım kasıtlıdır.

### Henüz yok

- `sntm-validate` host tool.
- Generated PMP profile tables.
- Native task image packer/loader.
- Runtime native task boot.
- Multi-region PMP reload from manifest.
- Typed IPC generator.
- Binary verifier / task certificate flow.
- SNTM runtime behavior tests with booted native tasks.

---

## 12. Doğrulama Altyapısı

### Var olanlar

- 198 Kani harness.
- 7 TLA+ model.
- `make check` clippy gate.
- `make run-self-test` POST + integration/self-test path.
- `scripts/sipahi_sprint_gate.sh`.
- `scripts/sntm_sprint_gate.sh`.
- `scripts/check_coverage.sh`.
- `scripts/check_proof_quality.sh`.
- `scripts/feature_matrix.sh` ile 10 feature kombinasyonu.
- GitHub Actions: build, QEMU smoke/self-test, audit/deny, Kani, binary guards,
  constant-time helper inspection.

### Doğrulama sınırları

- Kani bounded model checking yapar; tüm concurrency/hardware davranışını
  kanıtlamaz.
- TLA+ modelleri soyut protokolleri kontrol eder; Rust implementation ile birebir
  refinement proof yoktur.
- Coverage check isim tabanlı mekanik guard'dır; test/proof kalitesi hâlâ
  review gerektirir.
- QEMU gerçek cache, bus contention, PMP timing ve FPGA platform etkilerini
  modellemez.

---

## 13. Bilinçli Sınırlar ve Roadmap

Bu özellikler dokümanlarda/planlarda yer alır ama mevcut runtime guarantee
değildir:

- AMCI multi-hart runtime.
- SPMP / WorldGuard / IOPMP production enforcement.
- CLIC entegrasyonu.
- Scratchpad/TCM optimizasyonu.
- CHERI research branch.
- Hardware CFI.
- SNTM-SAFE binary verifier.
- Task certificate flow.
- Real FPGA WCET database.

Bu ayrım özellikle önemlidir: Sipahi'de tasarım yönü güçlüdür, ama her tasarım
fikri mevcut kod özelliği değildir.
