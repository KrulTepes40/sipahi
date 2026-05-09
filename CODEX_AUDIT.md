# Sipahi v1.0 Codex Audit Report

## Ozet

- Toplam bulgu: 14
- CRITICAL: 0
- HIGH: 4
- MEDIUM: 6
- LOW: 4
- INFO: 0
- Calisma agaci: kaynak kod degismedi; bu rapor dosyasi yeni/degismis. Ayrica audit basinda mevcut untracked `claude code audit.md` goruldu, dokunulmadi.

## Threat Model Referansi

Severity A.0'a gore kalibre edildi. `ARCHITECTURE.md` Known Limitations icinde acikca belgelenen vanilla PMP / `MAC_KEY` okunabilirligi ayrica finding yapilmadi; ancak UART MMIO, scheduler tick yan etkisi, unknown trap ve ilk task register leak bu known limitation kapsamina girmiyor.

## CRITICAL Bulgular

Yok.

## HIGH Bulgular

```text
[SEV] HIGH
[KAT] Security
[DOS] src/kernel/memory/mod.rs:107, src/kernel/memory/mod.rs:113
[BUL] PMP Entry 7 UART'i R|W|L aciyor; hostile U-mode task syscall/capability olmadan 0x1000_0000 UART MMIO'ya dogrudan yazabilir.
[ONR] UART'i PMP eslesmesi disinda birakip M-mode'un unmatched erisimini kullanin veya Smepmp ile M-only MMIO tanimlayin.
[ETK] U-mode task UART gate/rate/policy katmanlarini bypass ederek output flood ve timing DoS yapar.
[KNT] nl -ba src/kernel/memory/mod.rs -> Entry 7 PMP_TOR | PMP_R | PMP_W | PMP_L; UART_BASE=0x1000_0000 src/common/config.rs:71.
```

```text
[SEV] HIGH
[KAT] Runtime / Security
[DOS] src/kernel/syscall/dispatch.rs:431, src/kernel/scheduler/mod.rs:230, src/kernel/scheduler/mod.rs:286
[BUL] SYS_YIELD dogrudan schedule() cagiriyor; schedule() her cagrida blackbox tick artiriyor ve IPC per-tick sayaclarini sifirliyor.
[ONR] Timer tick path ile voluntary-yield scheduling'i ayirin; tick/cooldown/rate reset sadece timer interrupt'ta yapilsin.
[ETK] U-mode task yield spam ile token expiry/cooldown zamanini sisirir ve IPC rate limiter'i tick disi resetleyerek DoS yuzeyi acar.
[KNT] nl -ba src/kernel/syscall/dispatch.rs | sed -n '428,435p' ve nl -ba src/kernel/scheduler/mod.rs | sed -n '229,289p'.
```

```text
[SEV] HIGH
[KAT] Runtime / Correctness
[DOS] src/arch/trap.rs:214, src/arch/trap.S:57
[BUL] Unknown exception path sadece debug loglayip 0 donduruyor; trap.S yalniz ecall icin mepc += 4 yaptigi icin faulting instruction'a geri donulur.
[ONR] Bilinmeyen U-mode exception'lari fail-closed handle_task_fault() / isolate path'ine yonlendirin.
[ETK] U-mode breakpoint/misaligned/unsupported exception surekli ayni PC'ye donerek local trap livelock / DoS uretir.
[KNT] trap.S satir 57-66 sadece mcause=8/11 advance ediyor; trap.rs satir 214-224 _ => 0.
```

```text
[SEV] HIGH
[KAT] Security
[DOS] src/kernel/scheduler/mod.rs:477, src/arch/context.S:107
[BUL] task_trampoline caller-saved register'lari temizliyor, fakat ilk U-mode gecisi start_first_task() dogrudan mret ediyor ve ayni scrub yok.
[ONR] start_first_task() icinde ra/a0-a7/t0-t6 temizleyin veya ortak trampoline yolunu kullanin.
[ETK] Ilk task kernel register kalintilarini, pointer/degerleri ve call-site state'ini U-mode'da gorebilir.
[KNT] context.S satir 107-128 register clear var; scheduler.rs satir 477-488 csrw...; mv sp; mret ile scrub yok.
```

## MEDIUM Bulgular

```text
[SEV] MEDIUM
[KAT] Security / Doc
[DOS] src/boot.rs:47, src/boot.rs:60, src/hal/secure_boot.rs:91
[BUL] In-kernel secure boot yalniz test-keys ile calisiyor ve bos mesaji dogruluyor; production make build yolunda kernel image/section dogrulamasi yok.
[ONR] Ya linker-delimited image hash'i dogrulayin ya da secure boot'u net bicimde external ROM-only kapsamina alin.
[ETK] Self-test "Secure boot OK" sonucu kernel .text/.rodata/.data butunlugunu kanitlamiyor.
[KNT] make build default features icinde test-keys yok; boot.rs line 52 secure_boot_check(&[], ...).
```

```text
[SEV] MEDIUM
[KAT] CI
[DOS] .github/workflows/ci.yml:132, .github/workflows/ci.yml:158
[BUL] QEMU CI ALL TESTS PASSED gorunce QEMU'yu olduruyor; post-test scheduler/NF regression ve production run kontrol edilmiyor.
[ONR] PASS sonrasi kisa scheduler soak yapin, ^NF$ grep'i ve ayri production make run smoke ekleyin.
[ETK] U-18 nested-fault regression'i testler gectikten hemen sonra olursa CI kacirir.
[KNT] CI lines 132-140 PASS polling + kill; final checks only BOOT HALTED, NF yok.
```

```text
[SEV] MEDIUM
[KAT] FV / CI
[DOS] Makefile:55
[BUL] make kani mevcut Kani 0.67.0 ile bozuk: --all-harnesses unsupported.
[ONR] Makefile hedefini CI ile ayni sekilde cargo kani yapin.
[ETK] Yerel formal verification hedefi false-fail veriyor; kullanici Makefile'a guvenirse FV calistirilamaz.
[KNT] make kani -> error: unexpected argument '--all-harnesses' found.
```

```text
[SEV] MEDIUM
[KAT] Security / CI
[DOS] Cargo.toml:22, .github/workflows/ci.yml:65
[BUL] Dependencies exact pin degil (wasmi = "1.0.9", blake3 = "1", ed25519-dalek = "2"), CI --locked kontrolu informational.
[ONR] Safety-critical release icin =x.y.z pin ve gating cargo build --locked kullanin.
[ETK] Lockfile drift veya registry update'i release davranisini degistirebilir.
[KNT] cargo tree --locked PASS; cargo audit PASS; ama Cargo.toml exact pin kullanmiyor.
```

```text
[SEV] MEDIUM
[KAT] FV / Security
[DOS] scripts/verify-ct-eq.sh:24, .github/workflows/ci.yml:71
[BUL] ct_eq_16 LTO ile inline olunca script "Manual review needed" deyip exit 0 veriyor; CI de continue-on-error.
[ONR] Call-site disassembly icin branch pattern gate'i ekleyin ve job'u release-blocking yapin.
[ETK] Timing side-channel regression otomatik yakalanmiyor.
[KNT] cargo nm --release | grep ct_eq_16 bos; bash scripts/verify-ct-eq.sh -> manual review mesaji.
```

```text
[SEV] MEDIUM
[KAT] Runtime / Doc / Security
[DOS] src/sandbox/mod.rs:352, src/tests/mod.rs:386, sipahi.ld:91
[BUL] Production build'de WASM sandbox icin loader/execute call-site yok ve .wasm_arena section boyutu 0; hostile-WASM threat model'i yalniz self-test/demo yolunda egzersiz ediliyor.
[ONR] v1.0'da WASM production surface isteniyorsa capability korumali loader/syscall yolu ekleyin ve arena/runtime'i release'te KEEP/used hale getirin; istenmiyorsa dokumanlarda WASM'i self-test/prototype kapsamina alin.
[ETK] AMCI oncesi "WASM sandbox escape" kontrolleri production binary'de gercek bir runtime yolunu temsil etmiyor; guvenlik iddiasi ile calisan binary arasinda drift olusuyor.
[KNT] rg -n WasmSandbox -> yalniz src/tests/mod.rs ve src/sandbox/mod.rs; cargo objdump --release -- --section-headers -> .wasm_arena 00000000, semboller yalniz __wasm_arena_start/end.
```

## LOW Bulgular

```text
[SEV] LOW
[KAT] Doc
[DOS] docs/sipahi_features_tr.md:265, src/ipc/blackbox.rs:68
[BUL] Dokuman blackbox seq alanini 2B ve data alanini 46B diyor; kod seq:u32 ve data:42.
[ONR] TR/EN feature docs'u SEQ:4, DATA:42 olarak guncelleyin.
[ETK] Post-mortem parser yazacak kisi yanlis layout kullanabilir.
[KNT] blackbox.rs satir 64-73 gercek byte layout'u gosteriyor.
```

```text
[SEV] LOW
[KAT] Quality / Linker
[DOS] sipahi.ld:30
[BUL] Linker script /DISCARD/ tanimlamiyor; production ELF'te .eh_frame 0x460 byte kaliyor.
[ONR] .eh_frame, .got, gereksiz unwind metadata icin discard ekleyin.
[ETK] no_std/release yuzeyi ve binary boyutu gereksiz buyuyor.
[KNT] cargo objdump --release -- --section-headers -> .eh_frame 00000460.
```

```text
[SEV] LOW
[KAT] Quality / Doc
[DOS] src/sandbox/mod.rs:16, src/sandbox/mod.rs:39, src/sandbox/mod.rs:281, src/common/config.rs:65
[BUL] WASM yorum/metrikleri stale: sandbox.rs 64KB ve CRC ~120c diyor, config gercekte WASM_HEAP_SIZE=4MB ve CRC WCET=1500c.
[ONR] Tek kaynak olarak config.rs sabitlerini referanslayan yorum kullanin; proof isim/yorumlarinda sabit sayi yerine `WASM_HEAP_SIZE` yazin.
[ETK] Kod okuyan kisi modul boyut limiti ve WCET maliyetini 30x/12x yanlis yorumlayabilir.
[KNT] rg -n '64KB|120c|WASM_HEAP_SIZE' src -> sandbox stale satirlari + config.rs:65/223-226.
```

```text
[SEV] LOW
[KAT] FV / Doc
[DOS] Tla+/SipahiScheduler.tla:5, Tla+/SipahiScheduler.cfg:17
[BUL] Scheduler TLA+ header'i "starvation freedom" verify edildigini soyluyor, cfg ise StarvationFreedom'i bilincli olarak devre disi birakiyor.
[ONR] Header'i "priority correctness/state invariants; starvation intentionally not verified for fixed-priority policy" diye daraltin.
[ETK] Formal verification kapsaminda yanlis guven olusur; DAL-D starvation tasarim kabulunun uzeri kapanabilir.
[KNT] Scheduler.cfg satir 17-24 StarvationFreedom'in verify edilmedigini acikca not ediyor ve PROPERTY listesine almiyor.
```

## Past-Bug Regression Matrix

| Sprint | Bug | Regression Guard | CI Catch? |
|---|---|---|---|
| U-16 | `is_valid_user_ptr` tum ptr kabul | Kani + self-test `cross_task_pointer_rejected` | Evet |
| U-16 | Token owner mismatch | Kani fast + self-test | Evet |
| U-16 | IPC default allow | Kani + wrong-owner self-test | Evet |
| U-16 | Ready task watchdog cezalandirma | self-test `[INFO] Task 1 watchdog_counter = 0` | Evet |
| U-16 | Tek task schedule security skip | Kod duzeltilmis; ozel CI guard zayif | Kismi |
| U-16 | Allocator wrapping_add | self-test `allocator_overflow_safe` + Kani | Evet |
| U-17 | Lockstep CSE optimize | Kani fast `decide_action_lockstep_pure` | Evet |
| U-18 | task_trampoline NF | CI NF grep / production smoke yok | Hayir |
| U-19 | task_trampoline reg leak | trampoline clear var; `start_first_task` guard yok | Hayir |

## Attack Scenario Walkthrough Sonuclari

- WASM sandbox escape: Float scanner, allocator bounds ve fuel self-test'te durduruldu; known limitation olarak byte-level heuristic devam ediyor.
- WASM production status: ACIK/DRIFT. Production ELF'te WASM loader call-site ve arena yok; hostile-WASM senaryosu self-test tarafinda dogrulaniyor, release runtime'da degil.
- Compromised U-mode task: Stack pointer ve IPC owner testleri durduruyor; ACIK kalanlar UART MMIO direct access, yield-tick inflation, unknown trap DoS, first-task register leak.
- Hardware glitching: PMP shadow, policy lockstep, NF marker mevcut; CI NF regression ve ct_eq disassembly gating zayif.

## Olumlu Bulgular - Senior Gostergeleri

- `make build`, `make check`, `cargo audit`, `cargo deny check`, `cargo kani`, 7/7 TLA+ PASS.
- QEMU self-test: `ALL TESTS PASSED`, 12 PASS, 0 FAIL; `NF` marker yok.
- Reproducible build: evet, SHA256 `b29ac5374d5a1931d241220e8de9aa3becd3537c0c8acbfd1860d70ec99783a4`.
- Defense-in-depth iyi: PMP shadow, per-task NAPOT stack, capability owner check, default-deny IPC, lockstep policy, fail-closed channel seal.

## Tam Dosya / Kod Kalitesi Guncellemesi

- Kapsam: 78 tracked dosya envanteri alindi; 42 `src/` dosyasi, linker, Cargo/Make/CI, 7 TLA+ spec + cfg, scriptler, docs ve placeholder test modulleri okundu.
- `make check` tekrar PASS (`cargo clippy ... -D warnings`); production yolunda `unwrap()/expect()` yok. Gozlenen `unwrap()` Kani proof icinde, `panic` yalniz panic handler mesajinda.
- Unsafe yogunlugu 123 blok / 9.1 K Rust LOC; buyuk cogunluk SAFETY yorumlu. Mevcut safety-audit script satir-bazli oldugu icin multi-line SAFETY bloklarini false-positive raporluyor; script CI gate'i olmaya hazir degil.
- Uretim kalitesi genel olarak senior seviyeye yakin: bounded loop tercihleri, fail-closed default'lar, `#[must_use]`, compile-time asserts, tek kaynak config sabitleri ve Kani/TLA baglantisi iyi.
- TCB minimizasyonu icin en buyuk kalite borcu: 64 adet `dead_code` allow ve 3 blanket allow (`src/common/config.rs`, `src/arch/csr.rs`, `src/sandbox/mod.rs`). Cogunun rasyoneli var; yine de v1.0 release icin `sandbox`, `iopmp`, HAL v2.0 ve diagnostic yuzeyini feature-gate etmek binary/TCB okurlugunu iyilestirir.
- `tests/fi/mod.rs`, `tests/integration/mod.rs`, `tests/unit/mod.rs` sadece placeholder yorum iceriyor; gercek self-test/FI kapsami `src/tests/mod.rs` icinde. Bu yanlis degil ama repo okuma kalitesini dusuruyor.

## Dusuk Maliyetli Guvenlik Kazanclari

- UART PMP entry'sini kaldirmak veya M-only yapmak: en yuksek fayda/dusuk maliyet; U-mode output flood ve timing bypass'i kapatir.
- `schedule_timer_tick()` ile `schedule_yield()` ayrimi: yield spam'in tick/cooldown/rate-limit state'ini bozmasini kapatir.
- Unknown exception default'unu `handle_task_fault()`/isolate yapma: trap livelock'u fail-closed davranisa cevirir.
- `start_first_task()` register scrub ekleme: U-19 trampoline hardening'ini ilk task icin de tamamlar.
- Production WASM ya gercek loader/capability path ile acilsin ya da feature kapali/dokuman self-test kapsamina cekilsin; gri alan birakmayin.
- Boot'ta `mcounteren=0`, `mcountinhibit` politikasi ve `medeleg/mideleg=0` yazimini acik hale getirin; side-channel ve privilege delegation yuzeyini ucuzca sertlestirir.
- CI'ya release-blocking `cargo objdump` guard'lari ekleyin: `.eh_frame == 0`, float instruction yok, `ct_eq` call-site branch-free, `.wasm_arena` beklenen moda gore 0 veya 4MB.

## Performance Baseline

QEMU gercek cycle degil. Mevcut config estimate baseline:

- Scheduler tick: 350c
- Context switch: 80c
- IPC send: 60c
- Capability invoke cache: 25c
- Full token validate: 400c

Self-test WCET olcumu QEMU TCG'de informational olarak `syscall 0 max=613854 limit=25` asti; FPGA olcumu hala gerekli.

## Build Determinism

`make clean && make build` iki kez byte-identical sonuc verdi.

- Reproducible: evet
- Production binary: 33,536 byte
- SHA256: `b29ac5374d5a1931d241220e8de9aa3becd3537c0c8acbfd1860d70ec99783a4`

## Onerilen v1.5/v2.0 Iyilestirmeler

- Smepmp/MML ile M-only text/data/MMIO.
- Timer-yield ayrimi.
- Production secure boot boundary netlestirme.
- CI production soak + NF grep.
- Exact dependency pins.
- ct_eq gating.
- `/DISCARD/` linker temizligi.
- WASM production kapsam karari: feature-gated prototype ya da gercek loader + release guard.
- v1.0 TCB temizlik: placeholder host tests ve v2.0 HAL/IOPMP/diagnostic scaffolding'i feature gate.

## Metrik Ozet

- LOC: 9,132 Rust + 321 ASM
- Kani: 200 harness, `cargo kani` PASS
- TLA+: 7/7 PASS, 35,770 distinct states
- Production binary: 33 KB
- Unsafe blok sayisi: 123
- Blanket `dead_code` allow: 3 dosya; toplam `dead_code` allow girdisi: 64
- Dependency audit: `cargo audit` PASS, `cargo deny` PASS with warnings
- Reproducible build: evet
