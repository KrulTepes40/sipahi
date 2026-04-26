# Sipahi `src/` Tam Kapsam Audit Raporu

Tarih: 2026-04-26  
Kapsam: `src/` altındaki 42 dosya, toplam 8.580 satır.  
Yöntem: Dosyalar tek tek okundu; kritik güvenlik, memory safety, scheduler semantics, syscall boundary, IPC ownership, capability modeli, WASM sandbox, HAL/arch assembly ve Kani proof yüzeyi ayrıca incelendi.

Bu rapor bir formal proof değildir. "Satır satır audit" burada şu anlama gelir: kaynak dosyaları tek tek okunup riskli satırlar, invariant boşlukları, proof/test kapsamı ve production readiness açısından değerlendirildi.

## Yönetici Özeti

`src/` ağacı junior seviyede değil. Gerçek bare-metal RISC-V, PMP, U-mode geçişi, custom trap/context assembly, capability cache, Kani proof'ları, QEMU self-test ve WASM sandbox bir arada. Bu kapsam senior sistem programlama seviyesini gösteriyor.

Ancak `src/` içinde üretim güvenliği açısından kritik boşluklar var. En önemlileri:

1. Syscall pointer validasyonu hem yanlış sınırı kullanıyor hem task-specific değil.
2. Capability full validation token sahibini caller ile bağlamıyor.
3. IPC channel erişimi owner/role kontrolü yapmıyor; SPSC unsafe contract sistem seviyesinde enforce edilmiyor.
4. Vanilla PMP Entry 5 kernel `.data/.bss` için U-mode RW izni veriyor.
5. Scheduler tek-task early return ile PMP/blackbox/watchdog path'lerini atlıyor.
6. Ready task watchdog sayacı artıyor; starvation ile hang karışıyor.
7. Blackbox `BB_WRITE_POS` bozulursa runtime OOB write riski var.
8. WASM allocator alignment hesabında `wrapping_add` overflow riski var.
9. Production/test binary ayrımı yok; self-test default boot path'te koşuyor.

Benim `src/` özelindeki çıplak hükmüm:

| Alan | Değerlendirme |
| --- | --- |
| Sistem programlama seviyesi | Senior |
| Security boundary olgunluğu | Orta, kritik açıklar var |
| Formal verification kültürü | Güçlü ama proof kalitesi karışık |
| Production readiness | Düşük-orta |
| Certification readiness | Düşük |
| Genel kod kalitesi | İyi, fakat büyük dosyalar ve unsafe invariant boşlukları var |

## Kapsam ve Satır Sayıları

| Grup | Dosya sayısı | Satır |
| --- | ---: | ---: |
| `kernel/` | 10 | 2.822 |
| `sandbox/` | 2 | 751 |
| `tests/verify` | 2 | 1.709 |
| `common/` | 10 | 631 |
| `hal/` | 5 | 503 |
| `arch/` | 9 | 910 |
| `ipc/` | 2 | 846 |
| Root `main.rs`, `boot.rs` | 2 | 198 |
| Toplam | 42 | 8.580 |

## P0 Bulgular

### P0-1: Syscall user pointer validasyonu gerçek task pointer'larını reddediyor

Dosya: `src/kernel/syscall/dispatch.rs`  
Satırlar: `kernel_end_addr()` 64, `is_valid_user_ptr()` 77, IPC call-site'ları 295 ve 357

`is_valid_user_ptr(ptr, size)` `_end` sembolünü kernel sonu sayıp `ptr < _end` ve `end < _end` durumlarını reddediyor. Linker düzeninde task stack'leri `_end` öncesinde olduğu için gerçek U-mode task stack pointer'ları reject edilebilir.

Ek risk: Planlanan basit `task_stacks_range()` fix'i de yeterli değil. Tüm task stack bölgesini kabul etmek Task A'nın Task B stack'ini syscall buffer olarak vermesine izin verir.

Doğru fix:

```rust
fn is_valid_user_ptr(caller_task_id: u8, ptr: usize, size: usize) -> bool
```

Bu fonksiyon sadece caller task'ın kendi stack/user memory aralığını kabul etmeli. Owner metadata yoksa "upper RAM" gibi geniş allow yapılmamalı.

### P0-2: Capability owner binding eksik

Dosya: `src/kernel/capability/broker.rs`  
Satırlar: `validate_full()` 100, `token.task_id` kullanımı 111, cache insert 135

`validate_full(token, caller_task_id)` token'ın MAC'ini ve nonce'unu doğruluyor, fakat `token.task_id == caller_task_id` kontrolü yapmıyor. Geçerli token başka bir task tarafından ele geçirilirse full validation path'inden geçirilip attacker caller id ile cache'e eklenebilir.

Doğru fix:

```rust
if token.task_id != caller_task_id {
    return false;
}
```

Bu kontrol nonce/MAC path'inden önce yapılmalı ve Kani/QEMU negatif test ile korunmalı.

### P0-3: IPC channel ownership yok

Dosya: `src/ipc/mod.rs`, `src/kernel/syscall/dispatch.rs`  
Satırlar: `unsafe impl Sync` 78, `get_channel()` 166, syscall send/recv 313 ve 368

`SpscChannel` güvenliği tek producer/tek consumer varsayımına dayanıyor. Ancak `get_channel(id)` herhangi bir caller'a herhangi bir channel'ı döndürüyor. Syscall tarafında producer, consumer, owner veya capability kontrolü yok.

Etkisi:

1. SPSC data-race/memory-safety varsayımı sistem seviyesinde bozulabilir.
2. Task A, Task B için tasarlanmış channel'dan send/recv yapabilir.
3. `unsafe impl Sync` gerekçesi sadece yorumda kalıyor.

Doğru fix: Her channel için fail-closed ownership tablosu gerekir.

```rust
producer_task_id
consumer_task_id
```

Unassigned channel default open olmamalı. Varsayılan davranış deny olmalı.

### P0-4: Vanilla PMP Entry 5 U-mode için kernel data RW açıyor

Dosya: `src/kernel/memory/mod.rs`  
Satır: Entry 5 config 108

Entry 5 `.data + .bss + kernel_stack` için `PMP_TOR | PMP_R | PMP_W | PMP_L` ile kuruluyor. Vanilla PMP'de L bit M-mode mutation/permission davranışını etkiler; U-mode erişimi R/W/X bitleriyle belirlenir. Bu nedenle bu entry U-mode'a kernel writable data için RW izni verir.

Etkisi: `MAC_KEY`, `PMP_SHADOW`, scheduler state gibi güvenlik-kritik global veriler U-mode tarafından okunabilir/yazılabilir hale gelebilir.

Kısa vadeli çözüm: Bunu açık `Known Limitation` olarak belgelemek yetmez; mümkünse kernel data entry U-mode erişimini engelleyecek PMP/Smepmp stratejisi veya memory layout ayrımı gerekir. Smepmp yoksa bu proje "strong U-mode isolation" iddiasında dikkatli olmalı.

### P0-5: Scheduler tek task durumunda safety path'lerini atlıyor

Dosya: `src/kernel/scheduler/mod.rs`  
Satır: 211

`schedule()` başında:

```rust
if *TASK_COUNT.get() < 2 {
    return;
}
```

Bu erken dönüş blackbox tick, PMP integrity check, period/budget/watchdog gibi path'leri atlıyor. Tek task production konfigürasyonu gerçekçi olabilir; safety mekanizmaları task sayısına bağlı kapanmamalı.

Fix: Sadece context-switch seçimi atlanmalı. Blackbox tick, PMP verify, budget ve watchdog tek taskta da çalışmalı.

### P0-6: Ready task watchdog timeout yiyebiliyor

Dosya: `src/kernel/scheduler/mod.rs`  
Satırlar: 269-270

Watchdog counter `Running || Ready` için artırılıyor. Ready ama CPU alamayan düşük öncelikli task hang etmiş sayılabilir.

Etkisi: Starvation ile task hang birbirine karışır. Bu, safety policy'yi yanlış tetikleyebilir.

Fix: Watchdog sadece `Running` task için veya açık activation/deadline modeli üzerinden ilerlemeli. IPC rate reset Ready task için kalabilir.

### P0-7: Blackbox write position corruption OOB write'a dönebilir

Dosya: `src/ipc/blackbox.rs`  
Satırlar: `BB_WRITE_POS` 139, read 220, buffer write 244

Kani proof'larında out-of-bounds write position için güvenli mantık var, fakat runtime `log()` içinde `pos >= BLACKBOX_MAX_RECORDS` guard yok. `BB_WRITE_POS` bozulursa `BB_BUFFER[pos]` OOB olabilir.

Fix:

```rust
let pos = vol_read!(BB_WRITE_POS -> u8) as usize;
if pos >= BLACKBOX_MAX_RECORDS {
    vol_write!(BB_WRITE_POS, 0u8);
    return;
}
```

### P0-8: WASM allocator alignment hesabında wrapping overflow var

Dosya: `src/sandbox/allocator.rs`  
Satır: 47

```rust
let aligned = old.wrapping_add(align - 1) & !(align - 1);
```

`old + align - 1` wrap ederse düşük bir `aligned` değerine dönebilir. Sonraki bounds check bunu yakalamayabilir.

Fix:

```rust
let aligned = match old.checked_add(align - 1) {
    Some(v) => v & !(align - 1),
    None => return core::ptr::null_mut(),
};
```

## P1 Bulgular

| ID | Bulgu | Dosya/Satır | Etki |
| --- | --- | --- | --- |
| P1-1 | Production/test binary ayrımı yok | `src/main.rs:89` | Default boot her zaman `tests::run_all()` çalıştırıyor. |
| P1-2 | İlk task priority seçimi yok | `src/kernel/scheduler/mod.rs:397` | `TASKS[0]` hardcoded Running oluyor. |
| P1-3 | `SingleHartCell::get_mut()` tekrarları aliasing açısından kırılgan | `src/common/sync.rs:30`, scheduler genelinde | Pratikte single-hart, ama audit ve Rust aliasing modeli açısından zayıf. |
| P1-4 | Production UART output fazla geniş | `main.rs`, `boot.rs`, `memory.rs`, `scheduler.rs` | WCET ve nested-trap riskini artırır. |
| P1-5 | `context.S` fixed trap-frame slot varsayımına dayanıyor | `src/arch/context.S:16-18`, `27-31`, `53-56` | Mevcut tek kernel stack modelinde çalışır, ama çok kırılgan invariant. |
| P1-6 | Secure boot/key provisioning production path placeholder | `src/boot.rs:35-54`, `src/hal/key.rs:59-64` | Production güvenlik iddiasını sınırlar. |
| P1-7 | Degrade recovery cooldown yok | `src/kernel/scheduler.rs` recovery block | Sistem hemen recover/flap yapabilir. |
| P1-8 | `check_wcet_limits()` SYS_TASK_INFO için scheduler tick limitini kullanıyor | `src/kernel/syscall/dispatch.rs` limits array | WCET metrikleri yanıltıcı olabilir. |

## P2 Bulgular

| ID | Bulgu | Dosya/Satır | Not |
| --- | --- | --- | --- |
| P2-1 | WASM host-call ABI eksik | `src/sandbox/mod.rs:229`, `307` | `compute_copy` NotImplemented, linker host funcs kaydetmiyor. |
| P2-2 | WASM float scanner eksik opcode aileleri içeriyor | `src/sandbox/mod.rs` | `0xfc`, `br_table`, trunc opcodes eklenmeli. |
| P2-3 | Arena yorumları stale | `src/sandbox/mod.rs:13`, `src/sandbox/allocator.rs:19` | Kod 4MB, bazı yorumlar 64KB diyor. |
| P2-4 | `print_u32` ve `print_hex` loop bound'ları savunmasız | `src/common/fmt.rs:16`, `37` | Pratikte tip sınırı güvenli, ama `print_u64` kadar defensive değil. |
| P2-5 | `pub mod ipc` gereksiz | `src/main.rs:15` | Binary crate yüzeyini gereksiz açıyor. |
| P2-6 | `print_hex` unused hack | `src/main.rs:94-98` | Cleanup. |
| P2-7 | `recv` tail increment tutarsız | `src/ipc/mod.rs` | `tail + 1` yerine `wrapping_add(1)` tutarlılığı iyi olur. |
| P2-8 | Kani proof kalitesi karışık | `src/verify.rs` | Bazıları gerçek implementation değil, paralel mantık/sanity check. |
| P2-9 | Host Rust test stratejisi yok | `src/tests/mod.rs`, root `tests/` | Gerçek testler QEMU boot içi. |

## Dosya Dosya Audit

### Root

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/main.rs` | Okundu | P1 | Entry sade. Ancak boot banner ve task UART output production path'te. `tests::run_all()` koşulsuz. `pub mod ipc` ve `print_hex` hack cleanup bekliyor. |
| `src/boot.rs` | Okundu | P1 | Boot sırası anlaşılır. Production secure boot/MAC key path placeholder. Task oluşturma sonrası IPC ownership assignment yok. |

### Common

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/common/mod.rs` | Okundu | Düşük | Basit modül toplayıcı. |
| `src/common/config.rs` | Okundu | P2 | Sabitler merkezi ve iyi. WCET değerleri estimated olarak yazılmış. `WCET_TASK_INFO` ayrı sabit değil. |
| `src/common/types.rs` | Okundu | Düşük | TaskState ve TaskConfig temiz. Newtype wrapper'lar henüz API'lere entegre değil. |
| `src/common/error.rs` | Okundu | Düşük | Açık error enum iyi. Caller-binding gibi yeni hata tipleri eklenebilir. |
| `src/common/fmt.rs` | Okundu | P2 | Basit UART formatter. `print_u32`/`print_hex` loop bound'ları `print_u64` kadar defensive değil. |
| `src/common/sync.rs` | Okundu | P1 | `SingleHartCell` merkezi unsafe abstraction. Single-hart için pratik, ama tekrarlı `get_mut()` kullanımı audit açısından zayıf. |
| `src/common/diagnostic.rs` | Okundu | Düşük | Placeholder trait/stats. Kullanım yüzeyi az. |
| `src/common/crypto/mod.rs` | Okundu | Düşük | Feature-gated provider seçimi iyi. CNSA path henüz yok. |
| `src/common/crypto/provider.rs` | Okundu | Düşük | Trait yüzeyi küçük ve anlaşılır. Constant-time iddiası implementasyona bağlı. |
| `src/common/crypto/blake3_impl.rs` | Okundu | P2 | Gerçek BLAKE3 path iyi. Kani stub proof isimleri gerçek BLAKE3 güvenliğini ima edebilir; rename iyi olur. |

### Arch

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/arch/mod.rs` | Okundu | Düşük | Assembly include düzeni net. |
| `src/arch/boot.S` | Okundu | Orta | BSS + task stacks + WASM arena clear gerçekçi. 4MB clear boot süresi dokümante edilmiş. |
| `src/arch/trap.S` | Okundu | P1 | `mscratch` swap iyi. Nested fault sessiz park ediyor, forensic byte/log yok. |
| `src/arch/context.S` | Okundu | P1 | Context switch gerçek ve dikkatli. Ancak `__stack_top - 16` sabit user_sp slot varsayımı güçlü invariant istiyor. |
| `src/arch/trap.rs` | Okundu | Orta | Trap dispatch iyi. U-mode ecall MPP check var. Global `0xFF` magic task id sabite alınmalı. |
| `src/arch/csr.rs` | Okundu | Düşük | CSR wrapper'lar basit. `dead_code` blanket var. |
| `src/arch/pmp.rs` | Okundu | P1 | PMP register API iyi. Vanilla PMP security limitation üst katmanda çözülmeli. |
| `src/arch/uart.rs` | Okundu | Orta | Bounded UART loop iyi. Production output gating şart. |
| `src/arch/clint.rs` | Okundu | Orta | Timer drift/overrun handling iyi. QEMU/real hardware frekans ayrımı dokümante edilmeli. |

### HAL

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/hal/mod.rs` | Okundu | Düşük | Küçük modül toplayıcı. |
| `src/hal/device.rs` | Okundu | Düşük | Static dispatch trait iyi. Kullanım az, daha çok gelecek API. |
| `src/hal/key.rs` | Okundu | P1 | Test key gating iyi; non-test production placeholder zero. |
| `src/hal/secure_boot.rs` | Okundu | P1 | Ed25519 verify gerçek. Production ROM/OTP/anti-rollback yok. |
| `src/hal/iopmp.rs` | Okundu | P2 | Stub açıkça belirtilmiş. Disabled mode all-access by design, production claim'e dikkat. |

### Kernel Memory

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/kernel/mod.rs` | Okundu | Düşük | Modül sınırı sade. |
| `src/kernel/memory/mod.rs` | Okundu | P0 | PMP setup ciddi. En büyük sorun Entry 5 U-mode RW etkisi ve production UART init log'ları. PMP integrity shadow iyi. |

### Kernel Capability

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/kernel/capability/mod.rs` | Okundu | Orta | Re-export ve Kani proofs geniş. Bazı proof'lar tekrar/sanity seviyesinde. |
| `src/kernel/capability/token.rs` | Okundu | Düşük | 32B fixed layout ve LE header iyi. |
| `src/kernel/capability/cache.rs` | Okundu | Orta | Owner-isolated cache lookup iyi. Full validation owner bug'ı cache'in güvenliğini zayıflatıyor. |
| `src/kernel/capability/broker.rs` | Okundu | P0 | MAC, nonce, cache mantığı iyi. Token owner binding eksikliği kritik. |

### Kernel Syscall

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/kernel/syscall/mod.rs` | Okundu | Orta | ECALL wrapper'lar anlaşılır. Kullanıcı task'lar şu an bu wrapper'ları demo path'te kullanmıyor. |
| `src/kernel/syscall/dispatch.rs` | Okundu | P0 | Dispatch table, WCET ve result sanitization iyi. Pointer validation ve IPC ownership eksikliği kritik. |

### Kernel Scheduler/Policy

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/kernel/scheduler/mod.rs` | Okundu | P0/P1 | Projenin ana ağırlığı. Fixed priority, budget, watchdog, policy, PMP birlikte. Kritik sorunlar: tek-task early return, Ready watchdog, task 0 initial dispatch, yoğun `TASKS.get_mut()`, degrade cooldown yok. |
| `src/kernel/policy/mod.rs` | Okundu | Orta | Pure `decide_action` güçlü. Lockstep iyi fikir. `SingleHartCell` erişimi daha temiz yapılmalı; CSE hardening eklenebilir. |

### IPC

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/ipc/mod.rs` | Okundu | P0 | SPSC ring teknik olarak iyi. Sistem seviyesinde producer/consumer enforcement yok. |
| `src/ipc/blackbox.rs` | Okundu | P0/P1 | Record layout/CRC iyi. Runtime `BB_WRITE_POS` guard eksik. |

### Sandbox

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/sandbox/mod.rs` | Okundu | P2 | WASM validation/fuel iyi. Float scanner eksikleri ve host-call ABI incomplete. |
| `src/sandbox/allocator.rs` | Okundu | P0/P2 | Bump allocator basit. `wrapping_add` overflow fix şart. Yorumlar stale. |

### Tests ve Verification

| Dosya | Durum | Risk | Not |
| --- | --- | --- | --- |
| `src/tests/mod.rs` | Okundu | P1 | QEMU self-test güçlü. Ama boot path'e gömülü, default production ayrımı yok, gerçek U-mode syscall negatif testleri eksik. |
| `src/verify.rs` | Okundu | P2 | Proof sayısı etkileyici. Bazı proof'lar güçlü, bazıları implementation'a bağlı olmayan sanity/tautology. Kani limitation dokümante edilmeli. |

## Test/Proof Kapsam Boşlukları

Eklenmesi gereken negatif testler:

1. Task A, Task B'nin stack pointer'ını syscall'a verirse reject.
2. Task A, Task B'nin token'ını `validate_full` ile kullanamaz.
3. Task A, Task B'ye ait IPC channel'dan send/recv yapamaz.
4. Tek task konfigürasyonunda PMP integrity ve blackbox tick çalışır.
5. Ready ama CPU almayan task watchdog timeout yemez.
6. `BB_WRITE_POS >= BLACKBOX_MAX_RECORDS` durumunda log drop/reset yapar, OOB yazmaz.
7. Allocator align overflow durumunda null döner.

Lean 4 için en değerli invariant adayları:

1. `NoCapabilityImpersonation`
2. `NoCrossTaskIpcAccess`
3. `NoCrossTaskPointerAccess`
4. `ReadyTaskDoesNotTripWatchdog`
5. `SingleRunningTask`
6. `PmpRegionSeparation`

## Öncelikli Fix Sırası

### U-16 İçin Zorunlu

1. `is_valid_user_ptr(caller_task_id, ptr, len)` task-specific hale gelsin.
2. `validate_full()` owner mismatch'i reddetsin.
3. IPC channel ownership fail-closed eklensin.
4. Watchdog sadece Running task için artsın.
5. Tek taskta schedule safety path'leri çalışsın.
6. Allocator `wrapping_add` yerine `checked_add` kullansın.
7. `BB_WRITE_POS` guard eklensin.
8. Production/test binary ayrımı yapılsın.

### U-17 İçin

1. Production UART gating.
2. `start_first_task()` priority seçimi.
3. Nested fault minimal forensic output.
4. WASM scanner `0xfc`, `br_table`, trunc opcodes.
5. CI `--locked` veya ayrı lock check.

### U-18 Sonrası

1. `SingleHartCell` kullanımını tek-borrow pattern'e yaklaştır.
2. Proof isimlerini ve tautology proof'ları temizle.
3. Kani limitation ve WCET measurement sınıflarını dokümante et.
4. Host-call ABI'yi tamamla.
5. `src` büyük dosyaları modüler böl.

## Son Hüküm

`src/` ağacı güçlü. Bu kadar şeyi tek projede çalışır hale getirmek junior işi değil. Ama güvenlik sınırları henüz iddia seviyesine tam yetişmemiş. Özellikle syscall boundary, capability owner binding, IPC ownership ve PMP U-mode permission modeli düzeltilmeden Sipahi'ye production-grade güvenli microkernel demek doğru olmaz.

Doğru tanım:

> Senior seviyede geliştirilmiş, formal verification kültürü olan, ama kritik enforcement boşlukları kapatılmadan production safety/security seviyesine çıkmamış bir RISC-V microkernel prototipi.

Bu audit sonrası fix planı artık net: yeni özellik değil, önce güvenlik sınırlarını sertleştirme.
