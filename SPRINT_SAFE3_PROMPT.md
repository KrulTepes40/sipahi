# SPRINT-U32 / SAFE-3 — Binary Verifier + Task Certificate + Image Signature

> Hedef sürüm: **v1.8.0** · Süre tahmini: 2-3 hafta · Risk: YÜKSEK (yeni
> ~1500 LOC tool, certificate ABI sabitleme, build-time signing key yönetimi,
> image format değişikliği). Net WIN: build-time forbidden-opcode reject
> (LFI'nin %7 runtime overhead'ini SIFIR'a indirir, §17.3), kanıt paketi
> tek dosya (DAL-A audit "kim/ne/nasıl" sorularına single-source answer,
> §17.4).

## 0. Referans + Mevcut Durum

**Tasarım dökümanı (uyulacak — sapma için Codex review gerekçesi şart):**
- `SIPAHI_SNTM_DESIGN.md` §17.3 (SNTM Binary Verifier)
- `SIPAHI_SNTM_DESIGN.md` §17.4 (SNTM Task Certificate)
- `SIPAHI_SNTM_DESIGN.md` §17.10 [4/10] + [9/10] + [10/10] gate aktivasyonu
- `SIPAHI_SNTM_DESIGN.md` §17.11 Certificate Mesh artifact 6 + 9 + 10
- LFI ASPLOS'24: https://zyedidia.github.io/papers/lfi_asplos24.pdf

**Mevcut taban (HEAD = SAFE-2 sonrası, v1.7.0 candidate — onaylanırsa
tag'lenir, değilse SAFE-2 commit hash):**
- `tools/sntm-pack` — ELF parser (`object` crate `=0.36.5`), her task için
  `text/rodata/data` ayrı `.bin` üretir
  ([tools/sntm-pack/src/pack.rs](tools/sntm-pack/src/pack.rs)) — bu sprint
  için **ELF parse pipeline kanıtlanmış substrate**.
- `src/hal/secure_boot.rs::Ed25519Provider::verify(public_key, message, sig)`
  — ed25519-compact `=2.2.0`, no_std + no_alloc, RFC8032 wire-bit-eşit
  (kernel side mevcut).
- `tools/sntm-validate` — manifest validation + codegen (PMP_PROFILES,
  cap_generated, channels.rs). Bu sprint cert generation buraya VEYA yeni
  tool'a eklenir.
- `target/native/task_{hello,world}.{text,rodata,data}.bin` 0..244 byte
  arası raw section dump'lar. Bu sprint sonrası bu blob'lar **cert hash
  girdisi** olur.
- `sipahi.toml` `[[task]]` her task için `binary` path + `task_id` mevcut.
  Cert ABI version + signing key path eklenir.
- `ed25519-compact` deps zaten kernel tree'sinde. **Tool tarafı için yeniden
  yüklemeye gerek yok**, ama host tool'da farklı feature gerekebilir
  (`std` veya `default`).
- Safe gate `[4]/[9]/[10]` DEFER işaretli — bu sprintte 3 gate aktive.
- **Kani 196, TLA 8/8, sntm-validate 25/25, task-lint 18/18** (SAFE-2 sonu
  baseline; commit edilmemiş olsa bile state bu).
- **Coverage 14F + 17R symmetric** (R1 SAFE-1, R2/R3 SAFE-2).

**Kapsam DIŞI (scope creep ALARM):**
- ❌ Hardware Zicfilp/Zicfiss CFI (v2.5+, hardware-dependent)
- ❌ `cargo-call-stack` stack analyzer (SAFE-4)
- ❌ Production HSM key provisioning (sadece test-keys + DAL-D-grade
  build-time signing; production OTP/HSM SAFE-4 sonrası ayrı sprint)
- ❌ Runtime certificate parsing (kernel certificate **opaque blob** —
  sadece ed25519 image sig verify, certificate field'ları forensics tool'a)
- ❌ Forward-edge CFI full jalr target whitelist (v1.7 binary verifier
  sadece kernel-range jalr reject yapar; full CFI v2.5)
- ❌ Trust on First Use (TOFU) / certificate rotation
- ❌ Multi-signer / threshold sig

---

## 1. Sprint Hedefleri

### H1 — `riscv-bin-verify` host tool (§17.3)

Yeni crate: `tools/riscv-bin-verify/` (~1500 LOC tahmini)

```
tools/riscv-bin-verify/
├── Cargo.toml         # workspace = [], object/serde/toml deps
├── src/
│   ├── main.rs        # CLI: --elf <task.elf> --manifest sipahi.toml
│   ├── parser.rs      # ELF parse (object crate, sntm-pack pattern)
│   ├── decoder.rs     # RV64IMAC instruction decoder (32-bit + RVC 16-bit)
│   ├── opcodes.rs     # forbidden opcode tables (CSR, mret, sfence, F/D)
│   ├── sections.rs    # forbidden section/relocation check
│   ├── regions.rs     # manifest cross-ref (symbol → region)
│   └── lib.rs         # public API for sntm-validate to call (integration)
└── tests/
    └── integration.rs # ≥10 fixture: synthetic ELF with each violation class
```

CLI (Section 8 FIX-F karar noktası):

```
$ riscv-bin-verify --elf target/.../task_hello \
                   --manifest sipahi.toml \
                   --task-name task_hello
PASS: task_hello (1842 bytes, 0 forbidden opcodes, all sections allowed)

$ riscv-bin-verify --elf target/.../bad_task --manifest sipahi.toml
FAIL: bad_task — 3 violations:
  [opcode]   csrw at 0x80600014 (offset 0x14, privileged CSR write)
  [section]  .init_array present (global runtime init forbidden)
  [region]   symbol 'foo' (0x80700000) outside task_hello regions
```

Exit codes: 0 PASS, 1 violation(s), 2 IO/parse error.

Reject kuralları (§17.3'ten birebir, **SOFT-FAIL alternatifi YOK** — her
ihlal HARD-FAIL):

| Kategori | Detay | Test fixture |
|----------|-------|--------------|
| **Privileged ops** | csrr/csrw/mret/sret/sfence.vma/sfence.w.inval/wfi | synthetic `csrr a0, mstatus` ELF |
| **F/D float** | flw/fsw/fadd.s/fsub.d/fcvt.s.* (vd) | synthetic `fadd.s f0, f1, f2` ELF |
| **Forbidden sections** | .got/.got.plt/.plt/.eh_frame/.eh_frame_hdr/.init_array/.fini_array/.ctors/.dtors | synthetic ELF with `link_section = ".init_array"` |
| **W^X violation** | Writable + Executable section flag combo | synthetic ELF with mismatched `SHF_WRITE|SHF_EXECINSTR` |
| **Relocation residue** | R_RISCV_RELAX kalıntısı, PIC/PIE relocations | normal release ELF (not stripped) — verifier ALSO checks `-r` post-link relocations cleaned |
| **Region boundary** | symbol addr ∉ manifest [[task.region]] | synthetic ELF symbol @0x80999999 |
| **Indirect call CFI** | jalr hedef kernel range [0x80000000, 0x80100000) | synthetic ELF `jalr ra, 0(t0)` where t0 holds kernel addr (best-effort static; jalr full CFI v2.5+) |

**Indirect call CFI scope (Section 8 FIX-A karar noktası):**
- v1.8 scope: jalr/jal hedef immediate-resolvable veya register-tracked
  basit case'lerde kernel-range reject. Kompleks indirect call full CFI
  v2.5+. Bu sprint için sadece **immediate jal kernel-range** check yeter
  (jalr register-tracked best-effort raporlu warning, FAIL değil).

### H2 — `TaskCertificate` schema + generator (§17.4)

Cert struct iki tarafta tanımlı:

- **Kernel-side opaque blob** (`src/kernel/cert/mod.rs` — yeni mod):
  - `pub const CERT_ABI_VERSION: u32 = 1;`
  - `pub const CERT_SIZE: usize = ...;` (compile-time const, footprint sabit)
  - Kernel **parse etmez** — sadece ed25519 verify boot-time
  - Helper: `verify_cert_signature(cert_blob, pubkey, sig) -> bool` —
    mevcut `Ed25519Provider::verify` wrapper

- **Host tool struct** (`tools/sntm-cert-gen/src/cert.rs` — yeni binary tool
  veya `sntm-validate` extension; Section 8 FIX-B):

```rust
#[repr(C)]
pub struct TaskCertificate {
    // §17.4'ten birebir — ABI v1 dondurulur, breaking change → v2.

    // Kimlik
    pub task_id:        u8,
    pub _pad1:          [u8; 7],  // align(8) for next u8[32]
    pub task_name_hash: [u8; 32], // BLAKE3(task name)

    // Tedarik zinciri
    pub source_commit:  [u8; 32], // git rev-parse HEAD
    pub toolchain_hash: [u8; 32], // BLAKE3(rust-toolchain.toml)
    pub manifest_hash:  [u8; 32], // BLAKE3(sipahi.toml)

    // Build-time invariants
    pub pmp_profile_hash:   [u8; 32], // BLAKE3(PMP_PROFILES[task_id] bytes)
    pub allowed_syscalls:   u8,        // bitmap: 6 syscall × bit (SYS_*=0..5)
    pub _pad2:              [u8; 7],
    pub allowed_channels:   [u8; 8],   // channel id list (FF = empty slot)
    pub allowed_mmio:       [Range64; 4],
    pub max_stack_bytes:    u32,        // SAFE-4'te call-stack; v1.8 manifest.stack_size
    pub forbidden_opcode_scan: u8,      // 1 = pass, 0 = fail (build-time verdict)
    pub unsafe_count:       u16,        // task-lint result
    pub _pad3:              u8,

    // Binary section hashes (BLAKE3, sntm-pack output ile bit-eşit)
    pub text_hash:   [u8; 32],
    pub rodata_hash: [u8; 32],
    pub data_hash:   [u8; 32],

    // Kani proof IDs (task-specific harness sembol hash'leri)
    pub kani_proof_ids: [u32; 16],

    // Format version (must = CERT_ABI_VERSION)
    pub abi_version: u32,
    pub _pad4:       u32, // align(8) tail
}

#[repr(C)]
pub struct Range64 { pub base: u64, pub size: u64 }
```

Generator pipeline (`scripts/regen_task_certs.sh` yeni):

```
sipahi.toml + git HEAD + rust-toolchain.toml + task ELF + task .bin
    ↓ sntm-cert-gen (yeni binary)
    ↓
target/native/task_<name>.cert.bin     (TaskCertificate raw bytes)
target/native/task_<name>.cert.sig     (ed25519 signature, 64 bytes)
```

Drift guard: `sntm_safe_gate.sh [9/10]` regen + git diff. Cert dosyaları
**generated, commit edilir** (PMP_PROFILES pattern); CI'da regen + git
diff = empty.

### H3 — Image format + final ed25519 (§17.10 [10/10])

Sipahi image artifact format (CR Section 8 FIX-C karar noktası):

```
sipahi-image-v1.bin layout:
  ┌─────────────────────────────────┐
  │ Header (64 bytes)               │ magic "SIPI1" + version + manifest_hash
  ├─────────────────────────────────┤
  │ kernel.elf  (offset 0x40)       │ kernel binary (current target/...sipahi)
  ├─────────────────────────────────┤
  │ task_hello.text  (aligned 0x40) │ per-task sections, packed
  │ task_hello.rodata               │
  │ task_hello.data                 │
  ├─────────────────────────────────┤
  │ task_world.text  (aligned 0x40) │
  │ ... (per [[task]] sırasıyla)    │
  ├─────────────────────────────────┤
  │ task_hello.cert  (aligned 0x40) │ TaskCertificate blob + 64-byte sig
  │ task_world.cert                 │
  ├─────────────────────────────────┤
  │ Image sig (64 bytes)            │ ed25519(SHA-512(header..last_cert))
  └─────────────────────────────────┘
```

Yeni binary tool veya sntm-pack extension: `tools/sntm-image/` (Section 8
FIX-D — sntm-pack rename'i mi yeni tool mu?):

```
$ sntm-image --manifest sipahi.toml \
             --kernel target/.../sipahi \
             --signing-key keys/dev-image.priv \
             --output target/sipahi-image.bin
PASS: sipahi-image.bin (245 KB, kernel 240 KB, 2 tasks, sig OK)
```

Build-time signing key (Section 8 FIX-E):
- DEV path: `keys/dev-image.priv` (ed25519 32-byte seed, gitignored).
  Repo'da DEĞİL; `scripts/gen_dev_key.sh` ile tek seferlik üretilir, local
  geliştirme + CI test için.
- PROD path: HSM / OTP-provisioned, SAFE-4 sonrası ayrı sprint scope.
- `test-keys` feature: hardcoded test private key — sadece QEMU smoke test
  (kullanıcı doctrine'i biliyor, asla production'a sızmaz).

### H4 — Safe gate [4]/[9]/[10] aktivasyonu

`scripts/sntm_safe_gate.sh` güncellenir:

```bash
# [4/10] riscv-bin-verify — forbidden opcode + section + relocation
echo "[4/10] riscv-bin-verify (forbidden opcode + section + region)..."
RBVERIFY="tools/riscv-bin-verify/target/$HOST/release/riscv-bin-verify"
for task in task_hello task_world; do
    "$RBVERIFY" --elf "target/riscv64imac-unknown-none-elf/release/$task" \
                --manifest sipahi.toml --task-name "$task" || {
        echo "  FAIL: riscv-bin-verify($task)"
        exit 1
    }
done
echo "  PASS"

# [9/10] task certificate ed25519 sign + drift guard
echo "[9/10] task certificate ed25519 sign..."
bash scripts/regen_task_certs.sh > /tmp/safe3-certs.log 2>&1 || {
    echo "  FAIL: regen_task_certs.sh"
    tail -20 /tmp/safe3-certs.log
    exit 1
}
if ! git diff --quiet target/native/*.cert.bin target/native/*.cert.sig; then
    echo "  FAIL: cert drift — manifest/source/toolchain changed"
    git --no-pager diff --stat target/native/*.cert.* | head -10
    exit 1
fi
echo "  PASS"

# [10/10] image assemble + final ed25519
echo "[10/10] image assemble + final ed25519..."
SNTM_IMG="tools/sntm-image/target/$HOST/release/sntm-image"
"$SNTM_IMG" --manifest sipahi.toml \
            --kernel target/riscv64imac-unknown-none-elf/release/sipahi \
            --signing-key keys/dev-image.priv \
            --output target/sipahi-image.bin || {
    echo "  FAIL: sntm-image"
    exit 1
}
# Verify the signature we just produced (roundtrip sanity, drift guard)
"$SNTM_IMG" --verify target/sipahi-image.bin \
            --pubkey keys/dev-image.pub || {
    echo "  FAIL: sntm-image verify"
    exit 1
}
echo "  PASS"
```

Bu sprintte aktif gate sayısı **9/10**; [5] `cargo-call-stack` SAFE-4'e
bırakılır.

---

## 2. Gate Yapısı (G0–G9)

### G0.0 — MANDATORY PRE-FLIGHT FIX (Section 8 CR-1 + CR-2)

> Sprint G0'a geçmeden önce iki HIGH severity audit finding düzeltilir.
> Bunlar SAFE-3 substrate'ini etkiler — kirli baseline üzerine certificate
> hash chain kurulamaz.

**CR-1 fix (sipahi_api Error ABI alignment):**
- [sipahi_api/src/lib.rs:23-55](sipahi_api/src/lib.rs#L23) — `Error` enum
  variant'ları kernel `SyscallResult::to_raw()` ile bit-eşit hizala:
  - `InvalidSyscall = 0`, `NoCapability = 1`, `IpcFull = 2`, `IpcEmpty = 3`,
    `InvalidArg = 4`, `BufferFull = 5`.
  - Orphan variant'lar (`Permission`, `RateLimited`, `Internal`) SİL veya
    `BufferFull` ile birleştir (kernel emit yok şu an).
- `from_kernel` mapping kernel raw → variant inverse.
- Kani harness `syscall_error_abi_alignment` ekle (K8 cross-crate).
- Doğrulama: `cargo build` task crates clean + kernel self-test
  ALL PASS regression.

**CR-2 fix (sntm-validate KERNEL_SIZE):**
- [tools/sntm-validate/src/validate.rs:27](tools/sntm-validate/src/validate.rs#L27):
  `KERNEL_SIZE = 0x60_0000` (6MB, gerçek `_end ≤ 0x80600000`).
- **Daha iyi**: dinamik field `[kernel] reserved_size = 0x600000`
  sipahi.toml + sntm-validate okur. Drift invariant: `NATIVE_TASK_BASE ==
  kernel.base + kernel.reserved_size` yeni check.
- Integration test ek fixture: `kernel_overlap_at_1MB_rejected` (0x80100000
  region şu an silent geçiyor, fix sonrası REJECT olmalı).
- Doğrulama: `sntm-validate --manifest sipahi.toml` PASS (mevcut),
  yeni fixture FAIL test.

**CR-6 + CR-7 pre-flight setup (cert artifact + key gitignore):**
- `.gitignore` ek satırlar:
  ```
  # SAFE-3 (CR-6): cert artifacts ephemeral (git HEAD circular dep önler)
  target/native/*.cert.bin
  target/native/*.cert.sig
  target/sipahi-image.bin
  target/sipahi-image.sig

  # SAFE-3 (CR-7): private signing key ASLA repo'da
  keys/*.priv
  ```
- `keys/.gitignore` yeni (rules.rs allow `.pub` only):
  ```
  *
  !.gitignore
  !*.pub
  ```
- `scripts/gen_dev_key.sh` yeni (ephemeral keypair, idempotent local dev):
  ```bash
  #!/usr/bin/env bash
  set -eo pipefail
  cd "$(dirname "$0")/.."
  mkdir -p keys
  if [ ! -f keys/dev-image.priv ]; then
      openssl genpkey -algorithm ED25519 -out keys/dev-image.priv
      openssl pkey -in keys/dev-image.priv -pubout > keys/dev-image.pub
      echo "[gen_dev_key] new ephemeral keypair at keys/dev-image.{priv,pub}"
  else
      echo "[gen_dev_key] keys/dev-image.priv exists (skip)"
  fi
  ```

**CR-8 pre-flight setup (Kani crypto scope shrink):**
- Şu an `Ed25519Provider::verify(public_key, message, signature) -> bool`
  Kani context stub `false` döner ([src/hal/secure_boot.rs:75-83](src/hal/secure_boot.rs#L75)).
  Bu G0.0'da değişmez — SAFE-3 sprint içinde Kani harness'lerinde
  signature reject proof'ları yazmaktan kaçınılır (CR-8 doctrine).
- Pre-flight: tek bir Kani harness ekle `verify_cert_signature_bounded`
  → `kani::any()` input için no-panic. Stub `false` döndüğü için
  "bounded" yeterli — gerçek crypto cargo test fixtures'ta (G8'de).

**G0.0 exit criteria:**
- `cargo build` kernel + tasks PASS
- `cargo kani --harness syscall_error_abi_alignment --harness verify_cert_signature_bounded`
  → 2/2 PASS
- `cargo test sntm-validate` ≥27 PASS (25 baseline + 2 yeni CR-2 fixture)
- `make run-self-test` ALL TESTS PASSED (regression check)
- `cat .gitignore | grep cert.bin` non-empty (CR-6 enforce)
- `cat keys/.gitignore | grep priv` non-empty (CR-7 enforce)
- `bash scripts/gen_dev_key.sh` PASS, `ls keys/dev-image.pub` exists,
  `git check-ignore keys/dev-image.priv` PASS (ignored)
- Commit: `cr1-cr8: pre-SAFE-3 audit fix (syscall ABI + KERNEL_SIZE +
  cert artifact gitignore + ephemeral key bootstrap + Kani scope)` —
  ayrı commit, SAFE-3 sprint commitlerinden bağımsız (cherry-pick edilebilir).

### G0 — Pre-flight
- G0.0 tamamlandı mı? Değilse DUR.
- `git status` clean? Sprint başlamadan önce SAFE-2 commit + tag (v1.7.0)
  durumu kontrol — SAFE-2 commit edilmediyse user'a sor.
- HEAD = v1.7.0 (SAFE-2) + G0.0 commit. Değilse DUR.
- `cargo kani` ≥197/≥197 (196 + abi_alignment), `bash Tla+/run_tlc.sh` 8/8,
  `bash scripts/sntm_safe_gate.sh` SAFE-2 6/10 active PASS — baseline green
  ise devam.
- `tools/riscv-bin-verify/` ve `tools/sntm-image/` dizinleri yok (greenfield).
- `keys/dev-image.priv` yok, `keys/.gitignore` yok (sprintte oluşur).
- TodoWrite ile 10 gate listesi açılır.

### G1 — riscv-bin-verify scaffold + ELF parser
Dosyalar:
- `tools/riscv-bin-verify/Cargo.toml` — sub-workspace, deps: `object =0.36.5`,
  `serde =1.0.228`, `toml =1.1.2`, `blake3 =1.8.4` (yalnız host-side).
  Eşleştir `tools/sntm-pack` pattern (cargo +stable, x86_64-unknown-linux-gnu).
- `tools/riscv-bin-verify/src/main.rs` — CLI arg parser (SAFE-1 task-lint
  pattern).
- `tools/riscv-bin-verify/src/parser.rs` — ELF parse (`object::File`,
  section headers, symbol table, relocations).
- `tools/riscv-bin-verify/src/lib.rs` — `pub fn verify_elf(elf_bytes, manifest, task_name) -> Result<VerifyReport, VerifyError>`
  → integration test bu API'yı çağırır.
- `tools/riscv-bin-verify/tests/integration.rs` — boş scaffold (G2-G5'te
  doldurulur fixture'larla).

Doğrulama: `cargo +stable build --release --target x86_64-unknown-linux-gnu`
clean PASS. CLI `--help` çıktı verir.

### G2 — RV64IMAC instruction decoder + forbidden opcode tables
Dosyalar:
- `tools/riscv-bin-verify/src/decoder.rs` — 32-bit + RVC 16-bit decode:
  - `pub fn decode_instruction(bytes: &[u8]) -> Option<DecodedInstr>`
  - `pub enum DecodedInstr { System(SystemFn), FloatOp(FloatFn), Csr(CsrFn), Branch, JumpAndLink, JumpAndLinkRegister, Other }`
  - Reference: RISC-V ISA manual Vol 1 Ch 24 + RVC Ch 26.
- `tools/riscv-bin-verify/src/opcodes.rs` — sabit tablolar:
  - `FORBIDDEN_SYSTEM_OPS` = ["csrrw", "csrrs", "csrrc", "csrrwi", ...,
    "mret", "sret", "sfence.vma", "sfence.w.inval", "wfi"]
  - `FORBIDDEN_FLOAT_OPS` = ["flw", "fsw", "fld", "fsd", "fadd.s", ...,
    "fcvt.s.w", "fcvt.d.l", ...]
- Lookup: `is_forbidden_opcode(decoded: &DecodedInstr) -> bool`.

Test (G7'de): minimum **3 negative + 3 positive** fixture per forbidden
class:
- `csrrw a0, mstatus, t0` → DETECT
- `mret` → DETECT
- `fadd.s f0, f1, f2` → DETECT (F extension)
- Plain `add a0, a1, a2` → OK
- `jal ra, label` (immediate) → OK if target in task region
- `jalr ra, 0(t0)` → WARN/best-effort (CR-A)

### G3 — Section + relocation + W^X check
Dosya: `tools/riscv-bin-verify/src/sections.rs`
- `pub fn check_sections(elf: &Elf, task_name) -> Vec<Violation>`
- Denylist: `.got|.got.plt|.plt|.eh_frame|.eh_frame_hdr|.init_array|.fini_array|.ctors|.dtors`
- W^X: `sh_flags & (SHF_WRITE | SHF_EXECINSTR)` == both → REJECT
- Relocation: `R_RISCV_RELAX` kalıntısı → REJECT
- PIC/PIE: `e_type == ET_DYN` → REJECT (statik tek ELF kabul)

### G4 — Region boundary cross-check
Dosya: `tools/riscv-bin-verify/src/regions.rs`
- Manifest yükle (`tools/sntm-validate` `Manifest` struct'ını crate-share OR
  duplicate — Section 8 FIX-G karar noktası: yeni crate `sntm-manifest`
  ortak struct, ikisi de import eder).
- Her ELF symbol için `st_value` adresi manifest `[[task.region]]` aralığına
  düşüyor mu?
- Out-of-region symbol → REJECT.
- Indirect jalr kernel range (0x80000000..0x80100000) → REJECT (immediate
  veya register-tracked basit case).

### G5 — riscv-bin-verify integration tests
Dosya: `tools/riscv-bin-verify/tests/integration.rs`
- **Sentetik ELF generator** (helper): minimal RV64 ELF bytes builder
  (assemble at fixture creation; veya `cargo build --release -p
  tests/fixture_*` script ile pre-built fixture). Section 8 FIX-H — ne
  kullanılır: byte-literal builder mı, `riscv64-unknown-elf-as` shell mı?
- ≥15 fixture (Section 9.2 T1-T8 doctrine):
  1. plain valid task PASS
  2. csrw forbidden FAIL
  3. mret forbidden FAIL
  4. sfence.vma forbidden FAIL
  5. wfi forbidden FAIL
  6. fadd.s F-extension FAIL
  7. fld D-extension FAIL
  8. .init_array section FAIL
  9. .got section FAIL
  10. W^X violation FAIL
  11. R_RISCV_RELAX residue FAIL
  12. PIE binary FAIL
  13. symbol out-of-region FAIL
  14. jal immediate kernel-range FAIL
  15. valid jalr register-tracked OK (warning only)

### G6 — TaskCertificate schema + sntm-cert-gen
Dosyalar:
- `src/kernel/cert/mod.rs` — opaque blob const + `verify_cert_signature`
  wrapper (no parsing, only verify).
- `tools/sntm-cert-gen/Cargo.toml` + `src/main.rs` + `src/cert.rs` —
  TaskCertificate struct (repr(C), explicit padding), generator:
  - git rev-parse HEAD
  - BLAKE3(rust-toolchain.toml)
  - BLAKE3(sipahi.toml)
  - PMP_PROFILES[task_id] hash from `src/kernel/pmp/generated.rs`
  - allowed_syscalls bitmap (manifest [[task]] syscall whitelist —
    sipahi.toml extension Section 8 FIX-I)
  - allowed_channels — manifest [[channel]] producer/consumer ile match
  - allowed_mmio — manifest [[task.region]] perm=R/RW + base/size
  - max_stack_bytes — manifest stack_size (SAFE-4 cargo-call-stack ile
    refinement)
  - forbidden_opcode_scan — riscv-bin-verify exit code
  - unsafe_count — task-lint output parse
  - text_hash/rodata_hash/data_hash — sntm-pack .bin BLAKE3
  - kani_proof_ids — `cargo kani --list` parse + sembol hash
- ed25519 sign — keys/dev-image.priv ile imzala.
- `scripts/regen_task_certs.sh` — generator çağırır, çıktı `target/native/`.
- Integration tests (≥8 fixture):
  - manifest deterministic → cert deterministic (idempotent rebuild)
  - git commit değişimi → cert hash değişir
  - rust-toolchain.toml değişimi → cert hash değişir
  - text.bin değişimi → cert hash değişir
  - signature verify (positive)
  - signature tamper (flip 1 byte) → verify fail (NEGATIVE)
  - missing field default → reject
  - ABI version mismatch → reject

### G7 — sntm-image (image assembler + final ed25519)
Dosyalar:
- `tools/sntm-image/Cargo.toml` + `src/main.rs` + `src/format.rs` — image
  layout (header + kernel + tasks + certs + sig).
- `--verify` flag: image dosyasından header + signature check (kernel
  boot-time equivalent simülasyonu).
- Integration tests (≥6 fixture):
  - assemble → verify round-trip PASS
  - tamper kernel byte → verify FAIL
  - tamper cert → verify FAIL
  - tamper header magic → verify FAIL
  - wrong pubkey → verify FAIL
  - missing cert → assemble FAIL

### G8 — Kani harness'leri (Section 9.1 K1-K8 doctrine)
Dosya: `src/verify.rs` ek harness'lar.

Minimum **≥6 yeni proof** (toplam Kani ≥**202** = 196 + 6):

1. `cert_abi_version_pin` (K1+K8) — kernel `CERT_ABI_VERSION` const ve
   `CERT_SIZE` const arasında uyum (cross-crate). Tautology değil:
   `CERT_SIZE` raw `size_of::<TaskCertificate>()` ile compare; struct field
   eklenince fail.
2. `cert_signature_verify_bounds` (K3+K6) — `verify_cert_signature(blob,
   pubkey, sig)` `kani::any()` input için no-panic, false return on tamper.
3. `cert_signature_rejects_wrong_pubkey` (K5 negative) — pubkey != signer's
   pubkey için return false.
4. `cert_signature_rejects_tampered_blob` (K5 negative) — blob[i] = blob[i] ^ 1
   sonrası verify false.
5. `image_header_magic_invariant` (K7) — SIPI1 magic byte sequence kernel-side
   const ile imager const arasında bit-eşit.
6. `image_signature_verify_full_pipeline` (K8 cross-crate) — image bytes
   header + body + sig için verify pipeline'ı simüle eder (Kani sembolik
   input, ed25519-compact-stub mode).

### G9 — TLA+ + safe gate aktivasyonu + coverage R4
Dosyalar:
- `Tla+/SipahiSecureBoot.tla` (yeni) — image verify state machine:
  ```
  States: Unverified | HeaderValid | SigValid | Booted | HaltedFail
  Init   = Unverified
  Actions: VerifyHeader | VerifySignature | StartKernel | FailOnInvalid
  Invariants:
    StartedImpliesValid: Booted => SigValid
    NoFalseAccept: HeaderValid /\ tamper => SigValid = FALSE
    AtomicVerify: SigValid is atomic, no partial state
  ```
- 4 invariant + 1 liveness (Section 9.3 S1-S7).
- `Tla+/run_tlc.sh` 9/9 PASS (8 baseline + 1 SipahiSecureBoot).
- `Tla+/SipahiSNTM.tla` ChannelOwnership invariant (SAFE-2 carry-forward
  check — değişmez).
- `scripts/sntm_safe_gate.sh` `[4]/[9]/[10]` aktivasyonu (yukarıda H4
  detayı). Aktif gate **9/10** olur.
- `coverage.toml` `SNTM-SAFE-R4` + `SNTM-SAFE-R5` ekle:
  - R4: riscv-bin-verify build-time enforcement
  - R5: TaskCertificate + image signature pipeline
- `bash scripts/check_coverage.sh` 14F + 19R symmetric.

### G10 — Final verification (Section 9.4 template)
SAFE-2 G9 pattern aynen. Tüm 9.1-9.3 kriterleri için satır satır
[OK]/[SKIP]/[FAIL] rapor. Beklenenler:

- `make check` clippy clean
- `make build` production kernel
- `cargo test` task-lint 18/18 + sntm-validate 25/25 + riscv-bin-verify ≥15/15 +
  sntm-cert-gen ≥8/8 + sntm-image ≥6/6
- `task-lint --manifest` PASS
- `sntm-validate --manifest` PASS
- `bash scripts/regen_safe_codegen.sh && git diff --exit-code` clean
- `bash scripts/regen_task_certs.sh && git diff --exit-code` clean
- `bash Tla+/run_tlc.sh` 9/9 (SipahiSecureBoot ≤200 distinct state)
- `cargo kani` (default format) ≥202/≥202
- `bash scripts/sntm_safe_gate.sh` 9/10 active, [5] DEFER
- `bash scripts/check_coverage.sh` 14F + 19R
- `make run-self-test` ALL TESTS PASSED + image verify positive
- `bash scripts/check_cross_isolation.sh` 4-gate PASS
- `make run` production 30s NF-free + FATAL-free

---

## 3. Doktrin Hatırlatmaları (DEĞİŞMEZ)

| Kural | Niye |
|-------|------|
| **NO auto-commit, NO auto-tag** | Sprint approval ≠ blanket commit yetkisi — her commit için ayrı onay |
| **`git config` değiştirme** | Doctrine — Kullanıcı PATH yönetir |
| **`--no-verify` YASAK** | Hook fail = root cause fix, bypass değil |
| **Kernel certificate parse ETME** | §17.4 doctrine: cert opaque blob; sadece ed25519 verify. Parse forensics tool'a |
| **`validate_full` REMOVE etme** (carry-forward) | SAFE-2 doctrine: cross-hart/HSM/external token MAC YOLU AYNEN KALIR |
| **`sys_cap_invoke` bit-7 ABI değişmez** | SAFE-2 sabit ABI; certificate'da `allowed_syscalls` bitmap formu |
| **Production private key repo'da YOK** | `keys/.gitignore` zorunlu; sadece `keys/dev-image.pub` commit (verify için) |
| **`test-keys` build feature production'a sızmaz** | `cfg(feature = "test-keys")` compile-time gate, production build hatalı; SAFE-1 lesson |
| **`forbid(unsafe_code)` source-level attribute kullanma** | rustc 1.82+ no_mangle çakışması (SAFE-1 lesson) |
| **`cargo +stable` host tools için** | nightly serde_core ICE bypass (SAFE-1 lesson) |
| **Codegen + cert dosyaları `.gitignore` DEĞİL** | PMP_PROFILES pattern — drift guard CI'da `git diff --exit-code`. Yalnız PRIVATE KEY gitignored |
| **`unsafe` waiver tek tier** | SAFE-1 invariant 5: trusted_unsafe + demo_feature_waivers combine YASAK |
| **`object` crate `=0.36.5` exact pin** | Supply chain doctrine; ELF parse semantic değişimi → silent verifier bug |
| **`env -u RUSTFLAGS` host-tool subshell'lerinde** | CI fix (build_native_tasks.sh patterned): kernel linker flag'i host build'e sızmaz |

---

## 4. Codex Review Checkpoint'leri

Aşağıdaki kararların gerekçesi prompt'ta net olmalı; Codex review push-back
yapacak — alternatif analiz + kanıt hazır olsun:

| FIX | Konu | Karar | Niye |
|-----|------|-------|------|
| **FIX-A** | jalr indirect call CFI scope | v1.8 = immediate kernel-range reject + register-tracked best-effort warning; full CFI v2.5 hardware | Static CFI complete = compiler/linker support gerekir; v1.8 scope creep önler |
| **FIX-B** | sntm-cert-gen yeni binary mı, sntm-validate extension mı? | **Yeni binary** (`tools/sntm-cert-gen/`) | sntm-validate manifest-focused; cert generation farklı responsibility (BLAKE3 + ed25519 + git/toolchain inspection). SRP. Ortak `sntm-manifest` crate ile struct paylaşımı (FIX-G) |
| **FIX-C** | Image format — header/body/sig vs CPIO/TAR vs ELF wrapper? | **Custom flat layout** (SIPI1 magic + offset table + body + tail sig) | Self-contained, kernel boot loader basit, no archive parser dep. Alternatif (ELF wrapper) kernel-side parse ekler — istemiyoruz |
| **FIX-D** | sntm-pack rename mı sntm-image yeni mi? | **Yeni `tools/sntm-image/`** | sntm-pack already does ELF→.bin (single-task). sntm-image üst seviye image assembly (multi-task + cert + sig). İki ayrı responsibility |
| **FIX-E** | Build-time signing key (dev vs prod) | DEV: `keys/dev-image.priv` repo gitignored, `scripts/gen_dev_key.sh` ile üret. PROD: HSM/OTP **SAFE-4 sonrası ayrı sprint** | DEV path immediately usable; PROD scope creep önlenir, OTP integration ayrı |
| **FIX-F** | riscv-bin-verify CLI: per-task çağrı mı, batch mı? | Per-task (`--task-name X` zorunlu) | Manifest cross-ref task-specific region check ister. Batch tek-task'ta fail vs all-fail karışıklığı önlenir. CI for-loop |
| **FIX-G** | Manifest struct paylaşımı: sntm-validate ile riscv-bin-verify | **Yeni `tools/sntm-manifest/` lib crate** (struct-only, no validation) | Duplicate struct → drift risk. Lib crate her iki tool için ortak Manifest/TaskEntry/RegionEntry parse |
| **FIX-H** | Synthetic ELF fixture builder mı, pre-built ELF mı? | **Pre-built fixture binaries** (`tools/riscv-bin-verify/tests/fixtures/*.elf`) committed, generation script: `tools/riscv-bin-verify/tests/gen_fixtures.sh` (one-time, host tool) | Byte-literal builder fragile + RISC-V ABI değişimine duyarlı. Pre-built ELF deterministic test |
| **FIX-I** | manifest `[[task]] allowed_syscalls` field ekle | **EVET** — explicit syscall whitelist per task | Cert kapsam doğru olsun. Şu an default tüm syscall açık; SAFE-3'te per-task narrowing başlar |
| **FIX-J** | Image boot-time verify kernel-side ne kadar? | Şu an **sadece ed25519 image sig**; cert field forensics tool tarafında | §17.4 doctrine. Kernel runtime cost = sadece mevcut Ed25519Provider::verify (~3ms) |

---

## 5. Beklenen Çıktı

**Yeni kod (~LOC tahmini):**

| Modül | LOC | Açıklama |
|-------|-----|----------|
| `tools/sntm-manifest/{Cargo,src/lib}.rs` | ~150 | Shared Manifest/TaskEntry/RegionEntry struct |
| `tools/riscv-bin-verify/Cargo.toml` | ~20 | Sub-workspace, exact pin deps |
| `tools/riscv-bin-verify/src/{main,lib,parser,decoder,opcodes,sections,regions}.rs` | ~1500 | Binary verifier core (FIX-A scope) |
| `tools/riscv-bin-verify/tests/integration.rs` + fixtures | ~300 | ≥15 fixture, gen_fixtures.sh |
| `tools/sntm-cert-gen/Cargo.toml` + `src/{main,cert}.rs` | ~400 | TaskCertificate generator + ed25519 sign |
| `tools/sntm-cert-gen/tests/integration.rs` | ~200 | ≥8 fixture |
| `tools/sntm-image/Cargo.toml` + `src/{main,format}.rs` | ~300 | Image assembler + verify |
| `tools/sntm-image/tests/integration.rs` | ~150 | ≥6 fixture |
| `src/kernel/cert/mod.rs` | ~80 | Opaque blob const + verify wrapper |
| `src/verify.rs` | +100 | ≥6 yeni Kani harness |
| `Tla+/SipahiSecureBoot.tla` + `.cfg` | ~150 | Image verify state machine |
| `scripts/regen_task_certs.sh` | +40 | Cert regen pipeline |
| `scripts/sntm_safe_gate.sh` | +50 | [4]/[9]/[10] active |
| `scripts/gen_dev_key.sh` | +20 | Dev signing key bootstrap |
| `keys/.gitignore` | +5 | Private key gitignore |
| `keys/dev-image.pub` | 32 | Dev public key (committed) |
| `sipahi.toml` | +20 | allowed_syscalls per [[task]] |
| `coverage.toml` | +30 | R4 + R5 entries |
| `CHANGELOG.md` | +35 | v1.8.0 entry |
| **Toplam** | **~3500 LOC** | (SAFE-2 = ~2100 LOC, SAFE-3 ~1.7× scope) |

**Test/proof beklenenler:**
- `cargo test riscv-bin-verify` ≥15/15
- `cargo test sntm-cert-gen` ≥8/8
- `cargo test sntm-image` ≥6/6
- `cargo test task-lint` 18/18 (değişmez)
- `cargo test sntm-validate` ≥25/25 (değişmez; FIX-G shared manifest struct
  migration regression check)
- `cargo kani` ≥202 harness (196 + 6)
- TLA+ **9/9** PASS (SipahiSecureBoot yeni)
- self-test ALL PASS + image verify smoke
- coverage 14F + 19R (15+R4+R5+R6 mi 17+R4+R5 mi? Section 8 CR-6 tartış —
  prompt 14F+19R hedef, sapma G10 raporda açıklamalı)

**Sürüm + tag (kullanıcı onayı sonrası):**
- `v1.8.0` — sprint-u32/SAFE-3 — binary verifier + cert + image sig

---

## 6. Carry-Forward Listesi (SAFE-4'e)

Bu sprintte bilerek ertelenenler — SAFE-4 prompt'una eklenecek:

- `cargo-call-stack` integration (manifest stack_size ≤ analyzer max)
- safe gate `[5/10]` aktivasyonu
- TaskCertificate `max_stack_bytes` field refinement (cargo-call-stack çıktısı)
- Stack scribble/watermark debug-boot opt-in
- HSM / OTP production key provisioning (post-SAFE-4 ayrı sprint)
- Full forward-edge CFI v2.5+ Zicfilp/Zicfiss (hardware-dependent)
- sipahi_api `[[support_crate]]` task-lint scope expansion (SAFE-2 CR-4
  carry-forward, hâlâ pending)
- TaskCertificate ABI v2 migration plan (sprint backlog)

---

## 7. Sprint Başlamadan Önce Codex Review İçin Hazır Soru

1. **G6 cert ABI:** `repr(C)` + manuel padding mu, yoksa `bincode` /
   `postcard` serde framework mü? (Şu an manuel: drift guard kolay, dep
   minimum)
2. **G7 image format:** Tail signature SHA-512 over (header+body) mu, BLAKE3
   over header+sig-of-each-cert+kernel-hash mu? (`ed25519-compact` doğrudan
   message-bytes ile çalışır → SHA-512 internal)
3. **G8 Kani ed25519 verify:** Kani'ye ed25519 sembolik input gerçek
   verification cycle 10^4+ → unwind şişer. Stub mode (test-keys) ile
   yapısal test mi, full crypto verify Kani'ye değil?
4. **FIX-G shared manifest crate:** Yeni `tools/sntm-manifest/` lib crate
   sntm-validate'in `manifest.rs` modülünden çıkarılırken sntm-validate
   tests regression riski (struct path değişimi). Şimdi yapılsın mı,
   SAFE-4'e bırakılsın mı?

Codex review sonrası plan değişebilir — değişiklik gerekçesi prompt'a
**EKLENİR** (Section 8), mevcut metin SİLİNMEZ.

---

## 8. Codex Review v1 — Entegre Edilen Düzeltmeler (2026-05-18)

> Codex SAFE-2 sonrası 5 bulgu raporladı; 5'i de **lokal doğrulandı** ve
> SAFE-3'e entegre edildi. CR-1 ve CR-2 **HIGH** severity — sprint G0
> öncesi **MANDATORY PRE-FLIGHT FIX** olarak işaretli (G0.0 pre-check).
> CR-3/CR-4/CR-5 normal gate'ler içine dağıtıldı. Section 4 FIX'leri
> audit trail; Section 8 BAĞLAYICI.

### CR-1 — sipahi_api Error::from_kernel kernel ABI ile MISALIGNED (HIGH)

**Bulgu:**
[sipahi_api/src/lib.rs:42-54](sipahi_api/src/lib.rs#L42) `from_kernel`
mapping kernel [dispatch.rs:35-46](src/kernel/syscall/dispatch.rs#L35)
`SyscallResult::to_raw()` ile bit-uyumsuz:

| Raw value | Kernel `to_raw()` | sipahi_api `from_kernel()` | Sonuç |
|-----------|-------------------|----------------------------|-------|
| `usize::MAX`     | InvalidSyscall | **InvalidArg**     | ❌ task wrong error name |
| `usize::MAX - 1` | NoCapability   | NoCapability       | ✓ |
| `usize::MAX - 2` | IpcFull        | IpcFull            | ✓ |
| `usize::MAX - 3` | IpcEmpty       | IpcEmpty           | ✓ |
| `usize::MAX - 4` | InvalidArg     | **Permission**     | ❌ task wrong error name |
| `usize::MAX - 5` | BufferFull     | **InvalidSyscall** | ❌ task wrong error name |
| `usize::MAX - 6` | (yok)          | RateLimited        | (orphan variant) |
| `usize::MAX - 7` | (yok)          | Internal           | (orphan variant) |

**Etki:** Task'lar 3 syscall hatasını yanlış isimle alıyor. Üstelik tasks
`cap_invoke` veya `ipc_send` `Err(Internal)` görürse, gerçek kernel sebebi
`InvalidArg` olabilir → debugging için tamamen yanıltıcı.

**Pre-flight fix (sprint G0 öncesi):**
1. sipahi_api `Error` enum kernel'in `SyscallResult` ile bit-eşit hizalanır:
   - `InvalidSyscall = 0`, `NoCapability = 1`, `IpcFull = 2`, `IpcEmpty = 3`,
     `InvalidArg = 4`, `BufferFull = 5`
   - `from_kernel` mapping kernel raw value ile birebir.
2. Orphan variants (`Permission`, `RateLimited`, `Internal`):
   - Kernel henüz emit etmediği için **şu anda SİL** veya `BufferFull`'a
     map et (gelecekte kernel eklerse, kernel emit'e + API hizalamasına
     **AYNI commit'te** gelir, drift yasak).
3. Kani harness ekle: `syscall_error_abi_alignment` (Section 9.1 K8 cross-crate):
   ```rust
   #[kani::proof]
   fn syscall_error_abi_alignment() {
       // SyscallResult::to_raw() ile sipahi_api::Error::from_kernel
       // raw → variant mapping inverse — round-trip identity.
       use crate::kernel::syscall::dispatch::SyscallResult;
       use sipahi_api::Error;
       // (kernel + api crate boundary cross-check)
       assert_eq!(SyscallResult::InvalidSyscall.to_raw(), usize::MAX);
       assert!(matches!(Error::from_kernel(usize::MAX), Some(Error::InvalidSyscall)));
       // ... 5 variant aynısı
   }
   ```
4. Integration test: `tasks/task_hello/src/main.rs`'de bilinçli bad syscall
   ile error name observation (smoke). Bu test SAFE-3 sprint scope DIŞINDA
   ama doctrine'i sağlamak için pre-flight'ta yazılır.

**Section etkisi:**
- G0 → G0.0 yeni pre-flight step
- G7 Kani K8 listesine `syscall_error_abi_alignment` eklenir → toplam ≥7
  yeni Kani proof (önceki ≥6 hedef + 1)
- §5 LOC tahmini: ~30 LOC değişiklik (sipahi_api/src/lib.rs + verify.rs)

### CR-2 — sntm-validate KERNEL_SIZE = 1MB STALE (HIGH)

**Bulgu:**
[tools/sntm-validate/src/validate.rs:27](tools/sntm-validate/src/validate.rs#L27)
`KERNEL_SIZE = 0x10_0000` (1MB). Gerçek kernel `_end ≤ 0x80600000`
([sipahi.ld:129](sipahi.ld#L129)), `NATIVE_TASK_BASE = 0x80600000`
([src/common/config.rs:31](src/common/config.rs#L31)) → kernel image
**6MB** kullanıyor.

**Etki:** Manifest'te task region `0x80100000..0x80600000` arasında bir
yere yazılırsa validator silent KABUL EDER. Bu region kernel
`.task_stacks` (NOLOAD MAX_TASKS×8KB = 64KB), `.wasm_arena` (NOLOAD self-test 4MB),
`.bss` ile çakışır → **PMP isolation kırılması, silent overwrite riski**.

SNTM manifest güvenliğinin bel kemiği validator (sntm_safe_gate.sh [6/10]),
bu invariant'ın stale olması en güçlü gate'i delik bırakıyor.

**Pre-flight fix (sprint G0 öncesi):**
1. validate.rs `KERNEL_SIZE = 0x60_0000` (6MB) güncelle.
2. **Daha iyi**: const yerine **dinamik manifest field** ekle —
   `[kernel] reserved_size = 0x600000` sipahi.toml'da, validate.rs okur.
   Drift guard: `NATIVE_TASK_BASE == kernel.base + kernel.reserved_size`
   invariant'ı sntm-validate yeni check.
3. Integration test: yeni fixture `kernel_overlap_at_1MB_rejected` — task
   region 0x80100000..0x80104000 → şu an silent geçiyor, fix sonrası
   REJECT olmalı.
4. Mevcut `kernel_task_overlap_rejected` fixture: 0x80080000 region
   kernel range kullanıyor, **fix sonrası da PASS olur** (regression check).

**Section etkisi:**
- G0 → G0.0 pre-flight step (CR-1 ile birlikte)
- §1 H1 öncesi G0.0 + G0.1 (CR-2 + manifest reserved_size field)
- sntm-validate integration tests ≥27 (25 baseline + 1 yeni positive fixture
  + 1 yeni negative fixture)
- §5 LOC tahmini: ~50 LOC (validate.rs + manifest.rs + test fixture)

### CR-3 — TLA ChannelOwnershipInvariant TYPE-OK SEVİYESİ (MEDIUM)

**Bulgu:**
[Tla+/SipahiSNTM.tla:218](Tla+/SipahiSNTM.tla#L218) yeni invariant:
```tla
ChannelOwnershipInvariant ==
    \A c \in Channels :
        channels[c] = "NONE" \/ channels[c] \in Tasks
```

Bu sadece "channel value Tasks ∪ {NONE} üyesi mi?" — TypeOK'a çok yakın.
Gerçek **ownership** invariant'ı şunları içermeli:
- Producer ≠ Consumer
- Channel ID < MAX_IPC_CHANNELS
- Sealed sonrası producer/consumer immutable
- BOOT_CHANNELS table = channels state (consistency)

Codex haklı: rapor'da "channel ownership kanıtlandı" iddiası abartılı.

**Sprint içi fix (G9'da TLA update):**
1. ChannelOwnershipInvariant **güçlendir**:
   ```tla
   StrongChannelOwnership ==
       /\ \A c \in Channels :
           channels[c] = "NONE" \/ channels[c] \in Tasks  \* eski TypeOK
       \* SAFE-3: sealed sonrası channels immutable
       /\ (sealed = TRUE) =>
           \A c \in Channels : channels[c] = channelsAtSeal[c]
       \* SAFE-2 BOOT_CHANNELS: producer != consumer at-rest invariant
       /\ \A c \in Channels :
           channels[c] /= "NONE" =>
               \E p \in Tasks : channels[c] = p
       \* Liveness: assigned channel never reverts to "NONE" before seal
       \* (TLC eventually-stable check)
   ```
2. Mevcut `SealedAtomicityInvariant` zaten 2. maddeyi kapsıyor — bu
   redundancy DEĞİL, **explicit ownership semantic** olarak yeniden
   yazılır (ayrı invariant, ayrı theorem).
3. Sprint sonu raporda "type-level → semantic ownership" upgrade not.

**Section etkisi:**
- G9 TLA+ minimum set: 5 invariant + 1 liveness (önceden 4 + 1) — SipahiSecureBoot ile birlikte
- SipahiSNTM state count baseline check; ownership upgrade fail durumunda
  SAFE-2 carry-forward audit gerekli

### CR-4 — `syscall_ids_valid` STALE (LOW-MEDIUM)

**Bulgu:**
[src/verify.rs:100-104](src/verify.rs#L100) `syscall_ids_valid` proof:
```rust
let ids = [SYS_CAP_INVOKE, SYS_IPC_SEND, SYS_IPC_RECV, SYS_YIELD, SYS_TASK_INFO];
for &id in &ids {
    assert!(id <= 4);
}
```

Gerçek syscall set 6 adet: `SYS_EXIT = 5` U-23'te eklendi
([config.rs:183](src/common/config.rs#L183)). Yeni `syscall_id_set_complete`
proof bu 6'lı kontrolü doğru yapıyor, ama eski `syscall_ids_valid` hâlâ
duruyor ve `<= 4` check'i SYS_EXIT'i hariç tutuyor. Test arrays SYS_EXIT
içermediği için tautological pass — false-PASS pattern.

**Sprint içi fix (G7 Kani update):**
1. Eski `syscall_ids_valid` proof **SİL** (yeni `syscall_id_set_complete`
   tüm 6'lı set'i doğru kapsıyor; duplicate audit yükü).
2. Alternative: `syscall_ids_valid`'i SIL yerine `assert!(id <= 5)` +
   `ids = [...SYS_EXIT]` ekle. Codex önerisi: SİL daha temiz.
3. coverage.toml'da `syscall_ids_valid` referansı YOKSA → temiz.
   Varsa → kaldır.

**Section etkisi:**
- G7 Kani harness eklerken EK olarak **eski `syscall_ids_valid` silinir**
- Final harness count: 196 baseline - 1 (syscall_ids_valid eski) + 6 (SAFE-3
  yeni) + 1 (CR-1 abi_alignment) = **≥202** (önceki hedef değişmedi, ama
  net analiz)

### CR-5 — `cap_invoke` BIT-7 FOOTGUN (LOW-MEDIUM)

**Bulgu:**
[sipahi_api/src/lib.rs:91-101](sipahi_api/src/lib.rs#L91) `cap_invoke`
yorumu:
> `token` MUST have bit 7 clear (token id < 0x80). Bit 7 reserved as
> SAFE-2 path discriminant; if `token >= 0x80`, kernel kernel-side
> routes to `local_cap_invoke` semantics — likely DENY since lower
> bits don't match a valid resource_id.

Ama wrapper bu invariant'ı **enforce etmiyor**. Caller `cap_invoke(0x80, res, action)`
çağırırsa silent şekilde local-cap path'e düşer. "Likely DENY" iyimser
beklenti — eğer lower 7 bit ve resource yanlışlıkla geçerli kombinasyonu
yakalarsa false-grant olur (CR-1 SyscallResult mismatch ile birleşince
audit zorlaşır).

**Sprint içi fix (G3 sipahi_api refactor ile birlikte):**
1. `cap_invoke` wrapper'a guard ekle:
   ```rust
   pub fn cap_invoke(token: u8, resource: u16, action: u8) -> Result<(), Error> {
       // SAFE-2 ABI: bit 7 = local-cap path discriminant (CR-1 doctrine).
       // Legacy MAC token wrapper bit 7 set caller'ı path drift'ten korur.
       if token & 0x80 != 0 {
           return Err(Error::InvalidArg);
       }
       let ret = unsafe { ecall3(SYS_CAP_INVOKE, token as usize, ...) };
       ...
   }
   ```
2. Doctrine note `local_cap_invoke` documentation'da: "if you want the
   local path, use `local_cap_invoke`; `cap_invoke` is **strictly** MAC
   token path".
3. Integration test (sipahi_api lib test ekle veya kernel self-test'te):
   `cap_invoke(0x80, 0, 0) == Err(InvalidArg)` → bit 7 guard test.
4. Kani harness opt (CR-5 negative): `cap_invoke_rejects_bit7_token`
   sembolik input token bit 7 set → wrapper Error::InvalidArg.

**Section etkisi:**
- G3 sipahi_api refactor + ek 1 fixture (sipahi_api standalone test)
- §5 LOC tahmini: ~15 LOC (wrapper guard + doc + 1 test)
- Kani harness count: ≥202 hedef korunur (CR-5 opt ekleme yapılırsa +1
  → ≥203, sapma G10 raporda OK)

---

### CR-6 — Cert artifact commit + git HEAD CIRCULAR DEPENDENCY (HIGH)

**Bulgu:**
Prompt G6 + Section 1 H2: "cert dosyaları **generated, commit edilir**
(PMP_PROFILES pattern); CI'da regen + git diff = empty." Aynı zamanda
cert içeriği `source_commit = git rev-parse HEAD` içerir.

**Circular dependency:**
1. Cert generate (commit X üzerinden) → cert içinde `source_commit = X`
2. Cert commit edilir → yeni commit Y oluşur
3. Cert artık stale (X gösteriyor, HEAD = Y)
4. CI regen → cert artık `source_commit = Y` üretir → git diff ≠ 0 → safe gate FAIL
5. Düzeltmek için yeni cert commit Z oluşur → 1-4 sonsuz döngü

PMP_PROFILES pattern bu sorunu YAŞAMIYOR çünkü içeriği commit hash'i
referanslamaz — sadece sipahi.toml address constants. Cert chain `source_commit`
sebebiyle TEMELDEN FARKLI artifact sınıfı.

**Fix (G6 + G9 [9/10] revize):**
1. `target/native/*.cert.bin` ve `*.cert.sig` **COMMIT EDİLMEZ**. `.gitignore`'a
   `target/native/*.cert.*` eklenir.
2. CI ephemeral build artifact olur (her CI run kendi cert'ini üretir).
3. Drift guard [9/10] artık `git diff --exit-code` değil; bunun yerine
   **roundtrip sign+verify**:
   ```bash
   bash scripts/regen_task_certs.sh > /tmp/safe3-certs.log 2>&1 || exit 1
   # Verify each cert's signature against committed dev-image.pub
   for cert in target/native/*.cert.bin; do
       sig="${cert%.bin}.sig"
       "$SNTM_IMG" --verify-cert "$cert" --sig "$sig" \
                   --pubkey keys/dev-image.pub || exit 1
   done
   ```
4. **Golden hashes alternatif** (Codex önerisinde geçiyor): committed
   `target/native/cert_hashes.txt` — cert binary'lerin BLAKE3 hash'leri
   (kuşkusuz Cargo.lock değişmediği sürece deterministic). Drift guard
   bu dosyayı diff'ler, cert binary'lerini değil. Bu sprintte SCOPE
   GENİŞ kalır — golden hash dosyası carry-forward SAFE-4'e.

**Section etkisi:**
- §1 H2 cert dosyaları açıklaması REVİZE (commit edilmez)
- G6 generator pipeline diagram: "→ target/native/*.cert.* (gitignored)"
- G9 [9/10] drift guard mantığı: git diff yerine sign+verify roundtrip
- §3 doctrine: `target/native/*.cert.*` gitignore zorunlu, golden hash
  defer SAFE-4'e

### CR-7 — Private key + CI determinism + drift guard ayrımı (HIGH)

**Bulgu:**
Prompt G7 + Section 1 H3: "keys/dev-image.priv repo gitignored". CI ephemeral
nasıl regenerate edip git diff clean görür? Göremez — her CI run farklı key
üretir → farklı signature → mismatch.

Daha geniş sorun: signed runtime artifacts asla "source-of-truth drift
guard"a sokulmamalı. Drift guard **source/codegen** için anlamlı (kernel
const'larının manifest ile sync olması). Signed binary için "her run aynı
sig versin" enforcement non-deterministic key path'i ile imkansız.

**Fix (G7 + H3 + [10/10] revize):**
1. CI her run **ephemeral dev key** üretir (`scripts/gen_dev_key.sh` yeni,
   bir defalık çalıştırılır local dev için; CI'da her run yeni key):
   ```bash
   # scripts/gen_dev_key.sh — ed25519 random keypair generator
   if [ -z "${SIPAHI_DEV_KEY:-}" ] && [ ! -f keys/dev-image.priv ]; then
       openssl genpkey -algorithm ED25519 -out keys/dev-image.priv
       openssl pkey -in keys/dev-image.priv -pubout > keys/dev-image.pub
   fi
   ```
2. CI'da `keys/.gitignore` zorunlu (sadece `*.pub` izinli, `*.priv` ASLA).
3. CI workflow ek step (Clippy + Build job veya yeni job):
   ```yaml
   - name: Generate ephemeral dev signing key
     run: bash scripts/gen_dev_key.sh
   ```
4. Drift guard mantığı (CR-6 ile birlikte):
   - **SOURCE drift** (manifest → codegen): `cap_generated.rs`, `channels.rs` —
     `git diff --exit-code` mevcut SAFE-2 pattern AYNEN korunur.
   - **SIGNED ARTIFACT** (cert + image sig): sign+verify roundtrip
     (artifact ephemeral, sig deterministic değil ama verify deterministic).
5. Production sig: HSM-provisioned key SAFE-4 sonrası ayrı sprint; v1.8.0
   sadece "dev signing pipeline çalışıyor + roundtrip verify" iddiası.

**Section etkisi:**
- §1 H3 build-time signing key bölümü tamamen revize: "DEV path CI'da
  ephemeral, repo'da ASLA priv key; PROD HSM SAFE-4 sonrası"
- §3 doctrine satırı eklendi: "Drift guard source/codegen için; signed
  artifact için sign+verify roundtrip"
- G9 safe gate [10/10] script revize (verify-only, regen yok)

### CR-8 — Kani ed25519 crypto KANITI ABARTI (HIGH)

**Bulgu:**
Prompt G8 Kani harness'leri:
- `cert_signature_rejects_wrong_pubkey` (K5 negative)
- `cert_signature_rejects_tampered_blob` (K5 negative)
- `image_signature_rejects_tampered_body` (K5 negative)

[src/hal/secure_boot.rs:75-83](src/hal/secure_boot.rs#L75) `cfg(kani)`
stub şöyle:
```rust
#[cfg(kani)]
fn verify(_public_key: &[u8; 32], _message: &[u8], _signature: &[u8; 64]) -> bool {
    false  // pessimistic Kani stub
}
```

Yani Kani context'inde `verify` **her zaman false** döner. "tampered blob
→ verify false" assertion tautology — stub zaten false dönüyor, gerçek
crypto'yu test etmiyoruz.

Section 9.1 K1 yasak desen: tautology proof.

**Fix (G8 revize, Section 9 doktrinine uyum):**
1. Kani harness scope **DARALT** — sadece structural:
   - `cert_abi_version_pin` (cross-crate const compare) ✓
   - `cert_size_invariant` (size_of::<TaskCertificate>() compile assert) ✓
   - `image_header_magic_invariant` (3-source byte compare) ✓
   - `verify_cert_signature_bounded` (no-panic on `kani::any` input, sembolik
     bounds; Kani stub sonucu doğrulamaz, sadece "panic yok") ✓
2. **Cert signature reject** proof'ları **KALDIR** — gerçek crypto stub
   altında anlamsız.
3. Real crypto pozitif + negatif test cargo test ile yapılır
   (`src/tests/mod.rs` veya `tools/sntm-cert-gen/tests/integration.rs`):
   - RFC 8032 test vector positive (ed25519 reference doc + 5 sıralı
     pair, public + message + sig → verify true)
   - Tamper fixture: signature byte flip, blob byte flip, pubkey byte flip
     → her biri verify false
   - Real crypto, real ed25519-compact, gerçek pozitif/negatif.
4. Self-test marker zaten var: `Ed25519 RFC8032 TV1 [OK]` + `Ed25519
   tampered sig RED [OK]` (mevcut U-29). SAFE-3'te ek olarak cert-level
   tamper fixture.

**Section etkisi:**
- G8 Kani harness listesi 6 → 5 (3 signature reject SİL, 2 structural
  EKLE: verify_cert_signature_bounded yeniden formüle, cert_size eklendi
  zaten)
- G8 cargo test ek fixture (sntm-cert-gen integration ≥10, +2 RFC + tamper)
- Section 9.1 K1+K8 yorum güncellendi — ed25519 verify crypto Kani scope
  DIŞINDA, real test cargo test
- Final Kani harness count beklentisi: 196 baseline + 1 CR-1 + 5 SAFE-3
  structural - 1 CR-4 (syscall_ids_valid sil) = **≥201** (önceki ≥202'den
  hafif düşüş, doğru analiz)

### CR-9 — `allowed_syscalls` FAKE SECURITY riski (MEDIUM)

**Bulgu:**
Prompt §1 H2 TaskCertificate:
```rust
pub allowed_syscalls: u8,    // bitmap: 6 syscall × bit
pub allowed_channels: [u8; 8],
pub allowed_mmio: [Range64; 4],
```

Kernel `cert` opaque blob doctrine'ı (§17.4): kernel parse etmez, sadece
image sig verify. Yani `allowed_syscalls` bitmap **runtime'da hiçbir şey
enforce etmiyor**. Task SYS_TASK_INFO whitelist dışındaki bir syscall
yaparsa, kernel kabul eder. Cert sadece **forensics metadata** — DAL audit'inde
"task bu syscall'i yapmamalı" diye not.

**Fake security:** bitmap görüntüsü "kontrol var" iddiası yaratıyor; ama
yok. Audit hata yapabilir.

**Fix opsiyonları:**
- **A (önerilen, SAFE-3 scope):** Cert doktrini AÇIK YAZ — "allowed_syscalls
  / allowed_channels / allowed_mmio = **forensics metadata ONLY**, kernel
  runtime enforcement YAPMAZ. Build-time forbidden opcode reject zaten
  riscv-bin-verify ile var (forbidden CSR/mret/F/D). Whitelist enforcement
  CFI roadmap v2.5+ scope".
- **B (Codex önerisi, SAFE-3 scope crossing risk):** riscv-bin-verify build-time
  ecall enforcement — safe-tier task'larda `ecall` öncesi `li a7, X`
  instruction'ı statik analiz; X manifest `allowed_syscalls` bitmap dışı
  ise REJECT. Bu scope büyür (~300 LOC ek decoder logic), SAFE-3 zaten
  ~3500 LOC.

**Karar (A önerilen, B carry-forward SAFE-4):**
1. Cert dokümanı + sipahi.toml field yorum + audit trail'de **NETLEŞTİR**:
   "forensics metadata only — runtime enforcement riscv-bin-verify build-time
   (CSR/mret/sfence/wfi reject); syscall whitelist v2.5+ CFI".
2. README + ARCHITECTURE doc sync: cert field roller belge edilir.
3. CR-10 (next) ile birleşip riscv-bin-verify'ın enforcement scope'unu
   da netleştirir.

**Section etkisi:**
- §1 H2 TaskCertificate struct comment: "forensics metadata" label.
- Section 8 CR-9 doctrine: cert opaque blob, runtime enforcement
  riscv-bin-verify ile sadece **forbidden opcode** kapsamında.
- SAFE-4 carry-forward listesi: B opsiyonu (build-time ecall whitelist)
  + cargo-call-stack ile birlikte değerlendir.

### CR-10 — Binary verifier instruction scope PRECISION (MEDIUM)

**Bulgu:**
Prompt §1 H1 reject kuralları:
```
| **Privileged ops** | csrr/csrw/mret/sret/sfence.vma/sfence.w.inval/wfi |
```

`ecall` listede YOK; ama Codex "SYSTEM opcode komple yasak gibi uygulanırsa
task'lar kırılır" diyor — `ecall` SYSTEM major opcode (0x73) ile aynı sınıfta
ama task API'sinin GEREKLİ yapı taşı. `ebreak` (debug breakpoint) listede yok,
production'da olmamalı. Compressed FP (c.fld/c.fsd/c.flw/c.fsw/c.fldsp/
c.fsdsp/c.flwsp/c.fswsp) tablodan eksik — F/D float ailesinin 16-bit varyantı.

**Fix (G2 opcodes.rs + G5 test fixture ek):**

Güncellenmiş reject tablosu (G2 `FORBIDDEN_*` tabloları):

| Sınıf | İzinli (ALLOW) | Yasak (REJECT) |
|-------|----------------|----------------|
| **SYSTEM (0x73)** | `ecall` (task syscall API zorunlu) | `ebreak`, `csrrw`, `csrrs`, `csrrc`, `csrrwi`, `csrrsi`, `csrrci`, `mret`, `sret`, `uret`, `sfence.vma`, `sfence.w.inval`, `wfi` |
| **F/D float 32-bit** | (yok, RV64IMAC izin yok) | `flw`, `fsw`, `fld`, `fsd`, `fadd.s/d`, `fsub.s/d`, `fmul.s/d`, `fdiv.s/d`, `fsqrt.s/d`, `fcvt.s.*`, `fcvt.d.*`, tüm F + D opcode family |
| **F/D compressed 16-bit** | (yok) | `c.fld`, `c.fsd`, `c.flw`, `c.fsw`, `c.fldsp`, `c.fsdsp`, `c.flwsp`, `c.fswsp` |
| **Branches/jumps** | `jal` (immediate ∈ task region) | `jal` (immediate ∈ kernel range 0x80000000..0x80600000) |
| **Indirect (jalr)** | register-tracked best-effort | jalr target trackable kernel range |

Decoder.rs implementasyon notu (G2):
- SYSTEM opcode (0x73) funct12+rs1+rd ile alt-instruction parse zorunlu;
  `ecall = (imm12=0x000, rs1=0, rd=0)`, `ebreak = (imm12=0x001, rs1=0, rd=0)`.
  Geriye kalan funct12 değerleri CSR ailesi → reject.
- F/D detection: major opcode `OP-FP = 0x53`, load `0x07`, store `0x27`,
  compressed `c.fld/c.fsd/...` opcode 0b00/0b10 major class altında.

Yeni test fixture (G5):
- `ecall_allowed_in_task_code_pass` — minimal task ELF with `li a7, 3; ecall`
  PASS (SYS_YIELD).
- `ebreak_forbidden_fail` — ELF with `ebreak` → REJECT.
- `compressed_fld_forbidden_fail` — ELF with `c.fld` → REJECT.
- `jal_to_kernel_range_fail` — ELF `jal 0x80000000` → REJECT.

**Section etkisi:**
- §1 H1 reject tablosu güncellenmiş (yukarıda)
- G2 opcodes.rs: SYSTEM funct12 parse + `ecall` allow allowlist
- G5 test fixture sayısı: 15 → **18** (+3 yeni: ecall pass, ebreak fail,
  compressed FP fail)
- §5 LOC: ~50 LOC ek decoder logic

### CR-11 — ELF symbol region check FALSE POSITIVE riski (MEDIUM)

**Bulgu:**
Prompt §1 H1 region boundary: "symbol addr ∉ manifest [[task.region]]
→ REJECT". Ama ELF symbol table'da:
- `STT_FILE` symbols (debug file names) — st_value = 0, region check anlamsız
- `STT_SECTION` symbols (section headers symbolic alias)
- `SHN_ABS` symbols (absolute constants, manifest region'ı ile alakasız)
- `SHN_UNDEF` symbols (external references, defined değil)
- `.debug_*` section symbols — runtime'da kullanılmaz, region irrelevant
- Linker-emitted symbols (`__bss_start`, `_end`, `_etext`) — bunlar
  manifest region'ın **kenar adresleri** kabul edilmeli (off-by-one tehlikesi)

Naive "her symbol region içinde" check **false positive üretir**:
- debug build'da `STT_FILE` symbol için fail
- Optimization linker-emitted `_etext` adresinin region end ile birebir
  olması "boundary edge" — > vs >= comparison hassaslığı

**Fix (G4 regions.rs precision):**

Symbol filter logic (yalnız bunlar region check'e tabi):
1. **Defined ve allocated symbols**:
   - `st_shndx != SHN_UNDEF` (external değil)
   - `st_shndx != SHN_ABS` (absolute constant değil)
   - `st_shndx < SHN_LORESERVE` (reserve dışı)
   - Section'ın `sh_flags & SHF_ALLOC != 0` (runtime yüklenir)
2. **Type filter**:
   - `STT_FUNC` veya `STT_OBJECT` (code/data symbols)
   - `STT_FILE` SKIP (debug only)
   - `STT_SECTION` SKIP (section alias)
3. **Linker-emitted edge symbols**:
   - `__bss_start`, `_end`, `_etext`, `_data_start`, vs. — name pattern
     allowlist; region boundary EQUALITY izinli (< instead of <=)
4. **Empty st_value**:
   - `st_value == 0` SKIP (relocation pending veya placeholder)

Implementation note (G4):
```rust
fn should_check_symbol(sym: &Symbol, sections: &[Section]) -> bool {
    if sym.st_value == 0 { return false; }
    if matches!(sym.st_type, SymbolType::File | SymbolType::Section) { return false; }
    if matches!(sym.st_shndx, SectionIdx::Undef | SectionIdx::Absolute) {
        return false;
    }
    let section = match sections.get(sym.st_shndx) {
        Some(s) => s,
        None => return false,  // reserved index
    };
    if section.sh_flags & SHF_ALLOC == 0 { return false; }
    // Skip well-known linker-emitted edge symbols
    if EDGE_SYMBOLS.contains(&sym.name.as_str()) { return false; }
    true
}

const EDGE_SYMBOLS: &[&str] = &[
    "_end", "_etext", "_edata", "__bss_start", "__bss_end",
    "_data_start", "_data_end", "_rodata_start", "_rodata_end",
];
```

Yeni test fixture (G5):
- `debug_file_symbol_ignored` — ELF with `STT_FILE` symbol @ random
  address → PASS (filtered out)
- `linker_edge_symbol_at_region_end_pass` — `_etext` adresi region.end
  ile birebir → PASS
- `defined_function_oob_fail` — `STT_FUNC` symbol @ 0x80999999 (region
  dışı) → REJECT (gerçek positive case)

**Section etkisi:**
- G4 regions.rs symbol filter eklendi
- G5 test fixture: 18 → **20** (+2 false-positive guard testleri)
- §5 LOC: ~80 LOC ek filter + EDGE_SYMBOLS allowlist

---

### Section 8 güncellenmiş özet — G0 öncesi MANDATORY pre-flight + Sprint içi

| CR | Severity | Etkilenen | Aksiyon | Gate |
|----|----------|-----------|---------|------|
| **CR-1** | HIGH | sipahi_api/src/lib.rs + dispatch.rs | Error variant kernel ABI ile bit-eşit hizala + Kani K8 cross-crate proof | **G0.0 pre-flight** |
| **CR-2** | HIGH | tools/sntm-validate/src/validate.rs + sipahi.toml | KERNEL_SIZE 1MB → 6MB + manifest reserved_size field + drift invariant | **G0.0 pre-flight** |
| **CR-3** | MEDIUM | Tla+/SipahiSNTM.tla | ChannelOwnership type-level → semantic upgrade | G9 |
| **CR-4** | LOW-MED | src/verify.rs | syscall_ids_valid SİL (syscall_id_set_complete kapsıyor) | G7 |
| **CR-5** | LOW-MED | sipahi_api/src/lib.rs | cap_invoke bit-7 guard + InvalidArg + test | G3 |
| **CR-6** | HIGH | scripts/regen_task_certs.sh + .gitignore + safe_gate [9/10] | Cert artifact COMMIT EDİLMEZ (circular dep); drift guard sign+verify roundtrip | G6 + G9 |
| **CR-7** | HIGH | scripts/gen_dev_key.sh + ci.yml + keys/.gitignore | CI ephemeral dev key; private key ASLA repo'da; signed artifact drift guard'a sokulmaz | G7 + ci.yml |
| **CR-8** | HIGH | src/verify.rs + sntm-cert-gen tests | Kani ed25519 crypto KANITI YOK (stub tautology); real crypto cargo test fixtures (RFC 8032 + tamper) | G8 |
| **CR-9** | MEDIUM | TaskCertificate struct yorumu + ARCHITECTURE.md | allowed_syscalls/channels/mmio = **forensics metadata only**, runtime enforcement riscv-bin-verify build-time kapsamında | G6 + doc |
| **CR-10** | MEDIUM | tools/riscv-bin-verify/src/opcodes.rs + decoder.rs | ecall ALLOW, ebreak REJECT, F/D + compressed FP family REJECT precision; SYSTEM funct12 parse | G2 + G5 |
| **CR-11** | MEDIUM | tools/riscv-bin-verify/src/regions.rs | Symbol filter: STT_FILE/STT_SECTION/SHN_ABS/SHN_UNDEF/empty/edge SKIP; sadece defined alloc STT_FUNC/STT_OBJECT check | G4 + G5 |

**Genel doctrine güncellemesi:**

1. **"Generated commit edilir" SADECE source-derived artifact için**
   (PMP_PROFILES, cap_generated, channels) — bunlar manifest+config sabit
   girdileri. **Time-dependent artifact (cert, sig)** asla commit edilmez
   (circular dep / non-deterministic).
2. **Drift guard iki kategori**:
   - **Source/codegen drift**: `git diff --exit-code` (mevcut SAFE-2 pattern)
   - **Signed runtime artifact**: `sign + verify` roundtrip (artifact ephemeral)
3. **Kani scope**: ABI bounds + no-panic + structural invariant. **Crypto
   doğruluğu Kani scope DIŞINDA** — real test cargo test fixtures (RFC vector +
   tamper).
4. **Cert opaque blob doctrine** (CR-9): kernel parse YOK; cert field'ları
   forensics metadata. Build-time enforcement (forbidden opcode) riscv-bin-verify
   ile. Runtime whitelist enforcement v2.5+ CFI carry-forward.

**CR-1 + CR-2 + CR-6 + CR-7 + CR-8 sprint başlamadan önce uygulanır
(G0.0).** CR-3/CR-4/CR-5/CR-9/CR-10/CR-11 normal gate akışına entegre.
Section 8 BAĞLAYICI; Section 4 FIX-A..J audit trail.

---

### Section 8 yeni LOC + test sayısı tahmini

| CR | LOC etkisi | Yeni test |
|----|-----------|-----------|
| CR-1 | +30 (lib.rs + verify.rs) | +1 Kani harness |
| CR-2 | +50 (validate.rs + manifest.rs + test) | +2 sntm-validate fixture |
| CR-3 | +20 (SipahiSNTM.tla) | (TLA invariant strengthen) |
| CR-4 | -10 (silinen syscall_ids_valid) | -1 Kani harness (cleanup) |
| CR-5 | +15 (sipahi_api wrapper guard) | +1 Kani (cap_invoke_rejects_bit7_token) |
| CR-6 | +20 (.gitignore + regen script revise) | (G9 [9/10] revise) |
| CR-7 | +30 (gen_dev_key.sh + ci.yml step) | (CI ephemeral key gen) |
| CR-8 | -30 (kaldırılan Kani crypto proofs) +100 (cargo test RFC vector + 4 tamper fixture) | -3 Kani, +5 cargo test |
| CR-9 | +20 (doc comment) | (doc only) |
| CR-10 | +50 (opcodes.rs SYSTEM funct12 + compressed FP) | +3 riscv-bin-verify fixture |
| CR-11 | +80 (regions.rs symbol filter + EDGE_SYMBOLS) | +2 riscv-bin-verify fixture |
| **Toplam** | **+375 LOC** | **+8 cargo test, +1 net Kani (1 - 3 + 1 + 2 = net +1)** |

**Final SAFE-3 expectations güncellendi:**
- Kani harness count: 196 baseline + 1 (CR-1) - 1 (CR-4) - 3 (CR-8 kaldırılan) +
  4 (structural eklenen) + 1 (CR-5) = **≥198/≥198** PASS (önceki ≥202'den
  düşüş, ama her biri **gerçek property** kanıtlıyor — K1 tautology yasak doktrinine
  uygunluk)
- riscv-bin-verify integration tests: 15 → 18 (CR-10) → 20 (CR-11)
- sntm-cert-gen integration tests: 8 → 10 (CR-8 RFC + tamper)
- sntm-validate integration tests: 25 → 27 (CR-2 fixture)
- cargo test toplam: ≥84 (önceki ≥69'dan artış, real crypto test cargo
  test'e taşındı — Section 9.2 T1+T2 doktrinine uyum)

CR-3/CR-4/CR-5/CR-9/CR-10/CR-11 normal gate akışına entegre. Section 8
BAĞLAYICI; Section 4 FIX-A..J önerileri SAFE-3 kararları için (audit trail),
bu Section 8 ile çakışmazlar — farklı boyutlar.

---

## 9. Verification Doctrine — Proof + Test + Spec hepsi GÜÇLÜ

> **Kullanıcı doktrini (zorunlu, U-27/U-30/SAFE-2 ders):** "Eğer sprint'te
> proof, test veya spec varsa hepsi de **GÜÇLÜ** olacak." Zayıf verification
> = false PASS = production'a geçen bug. SAFE-2'de doktrinleştirildi,
> SAFE-3'te aynen geçerli. G10 final check'te her kriter satır satır
> doğrulanır.

### 9.1 Proof gücü kriterleri (Kani)

| Kriter | Açıklama | SAFE-3 örnek |
|--------|----------|--------------|
| **K1** Tautology yasak | `assert!(0 == 0)` veya tek satırlık `assert!(true)` proof yazılmaz | cert_abi_version_pin — kernel const + sntm-cert-gen const compare (drift detect) |
| **K2** Production const'ları çağır | Proof generated/production code'un GERÇEK sembollerini import etsin | image_header_magic_invariant: kernel SIPI1 const + sntm-image emit kontrol |
| **K3** Unbounded `kani::any()` | Input domain açık (`kani::any::<u8>()`) — concrete value test'tir, proof değil | cert_signature_verify_bounds: blob + pubkey + sig u8 array kani::any |
| **K4** Reachability check | `assume(false)` tek satır erken-exit YASAK (SAFE-1 lesson) | Tüm path traverse; ed25519 verify sembolik input için kompleks → stub mode opsiyonu (Codex Q3) |
| **K5** Negative ≥ positive | Her positive proof için **karşı negative** zorunlu | cert_signature_rejects_wrong_pubkey + cert_signature_rejects_tampered_blob + image tamper varyantı |
| **K6** Unwind = N+1 | Array size N için unwind(N+1) (off-by-one yakalar) | kernel CERT_SIZE → unwind(CERT_SIZE+1); image header parse 64-byte → unwind 65 |
| **K7** No dead arms | match arm'lar reachable; `_ => unreachable!()` ispatlanmalı | DecodedInstr 7 variant + forbidden lookup tüm dalı exercise eder |
| **K8** Cross-crate drift | Birden fazla crate'i bağlayan property birden fazla crate import | cert_abi_version_pin: kernel CERT_ABI_VERSION + host TaskCertificate::ABI_VERSION + sntm-image const ÜÇÜ compare |

**SAFE-3 Kani minimum set:**
1. `cert_abi_version_pin` — kernel + host cert ABI version 3-source compare
2. `cert_size_invariant` — `size_of::<TaskCertificate>() == CERT_SIZE` (compile-time `const _: () = assert!(...)` cross-validated runtime)
3. `cert_signature_rejects_wrong_pubkey` — wrong pubkey → false
4. `cert_signature_rejects_tampered_blob` — blob[i] flip → false
5. `image_header_magic_invariant` — SIPI1 byte sequence 3-source compare
6. `image_signature_rejects_tampered_body` — body byte flip → verify false
7. (bonus) `verify_cert_signature_bounded` — kani::any input no-panic

Toplam **≥6 yeni proof** → toplam ≥202.

### 9.2 Test gücü kriterleri (cargo test)

| Kriter | Açıklama | SAFE-3 örnek |
|--------|----------|--------------|
| **T1** Pozitif + negatif çift | Her check için 1 PASS + 1+ FAIL fixture | riscv-bin-verify 15 fixture: 1 valid + 14 violation class |
| **T2** Error mesaj assert | `is_err()` YETMİYOR; mesaj string assertion zorunlu | sntm-cert-gen test: `assert!(err.contains("tamper"))`, `assert!(err.contains("ABI version mismatch"))` |
| **T3** Realistic fixture | Minimal değil; gerçek manifest + pre-built ELF (riscv-bin-verify FIX-H) | Pre-built ELF fixtures committed (deterministic test, RISC-V ABI değişimine dayanıklı) |
| **T4** Temp dir izolasyon | Her test kendi `tempfile::tempdir()` | sntm-image test: temp dir + kernel.bin + cert.bin + sig roundtrip |
| **T5** Determinism | Random/timestamp/order yok | cert hash deterministic: aynı manifest+git+toolchain → aynı cert bayt-eşit (T1+T5 birleşik test) |
| **T6** Cross-platform | Windows/WSL + Linux native ikisinde geçer | SAFE-2 wsl bash -lc pattern |
| **T7** Hot path | Performans regression detect (opsiyonel) | image verify <50ms (kernel boot-time bütçe; criterion opsiyonel) |
| **T8** Drift fail simulation | Codegen drift CI'da yakalanır | cert.bin manuel düzelt → `git diff --exit-code` ≠ 0 → safe gate [9] fail (manual smoke test scripti) |

**SAFE-3 cargo test minimum set:**
- `riscv-bin-verify` ≥15 fixture (G5'te listelenen)
- `sntm-cert-gen` ≥8 fixture:
  1. valid manifest → cert generates
  2. cert deterministic (idempotent rebuild bayt-eşit)
  3. git commit change → cert hash change
  4. rust-toolchain change → cert hash change
  5. text.bin change → cert hash change
  6. signature verify positive
  7. signature tamper (flip byte) → verify FAIL
  8. ABI version mismatch → reject
- `sntm-image` ≥6 fixture (G7'de listelenen)
- `sntm-validate` ≥25 (mevcut, regression)
- `task-lint` 18 (değişmez)

### 9.3 Spec gücü kriterleri (TLA+)

| Kriter | Açıklama | SAFE-3 örnek |
|--------|----------|--------------|
| **S1** Invariant şart | Sadece type/syntax değil; semantik invariant | StartedImpliesValid: Booted state ↔ SigValid |
| **S2** Liveness property | Deadlock yok | `<>[]Booted \/ <>[]HaltedFail`: verify ya geçer ya fail eder, döngüde kalmaz |
| **S3** State count baseline | Beklenen state sayısı yazılır, regression detect | SipahiSecureBoot ≤50 state (basit state machine), >100 fazla abstraction |
| **S4** Counterexample reprod. | TLC fail → state trace `Tla+/states/` commit | Mevcut pattern; SAFE-3'te `SipahiSecureBoot-*.st` |
| **S5** Negative simulation | Deliberately broken model → spec catch | Side branch: NoFalseAccept invariant'ı kasten kırılır (tamper → SigValid=TRUE), TLC fail → confirm spec çalışıyor |
| **S6** Symmetry minimum | Over-abstraction false PASS yaratır | Tasks={t0,t1}, Channels={c0}, Bytes={normal, tampered} explicit + comment |
| **S7** Bağımlı invariant link | Yeni invariant mevcutlarla non-conflict | StartedImpliesValid ⊥ SipahiSNTM.RunningIsCurrent (paralel valid, TLC fail yok) |

**SAFE-3 TLA+ minimum set:**
- `Tla+/SipahiSecureBoot.tla`:
  - `TypeOK`
  - `StartedImpliesValid` (Booted ⊆ SigValid)
  - `NoFalseAccept` (tampered → SigValid=FALSE)
  - `AtomicVerify` (no partial verify state)
  - `<>[]Booted \/ <>[]HaltedFail` (liveness)
- 8 baseline (mevcut) + 1 yeni = **9/9 PASS**
- SipahiSNTM ChannelOwnership invariant (SAFE-2) — regression check.

### 9.4 G10 final check — doktrin enforcement template

SAFE-2 G9 pattern aynen. Her kriter satır satır [OK]/[SKIP]/[FAIL].
Bir kriter [FAIL] → gate yeniden açılır, root cause fix, tekrar G10.

### 9.5 Yasak desenler (SAFE-1+SAFE-2 lesson)

| Anti-pattern | Yasak |
|--------------|-------|
| `assert!(true)` veya `assert_eq!(0, 0)` | tautology |
| `#[ignore]` test | deferred ≠ silent skip |
| Yeni `TODO`/`FIXME` ekleme | sprint gate guard |
| Production `unwrap()` | crash risk (kernel side) |
| `unsafe` waiver olmadan | task-lint rule 1 (sipahi_api scope dışı doctrine korunur) |
| Cargo registry version range (`^x.y` veya `>=x.y`) | exact pin doctrine (`=X.Y.Z`) |
| Private key repo'da | `keys/dev-image.priv` git-ignored, sadece `.pub` commit |
| `--no-verify` git commit | hook bypass yasak |
| Codegen + cert dosyaları `.gitignore` | drift guard bypass |
| Kani `--output-format old` | reachability check false-FAIL (U-30.1 lesson) |
| Job-env `RUSTFLAGS` host-tool subshell'e sızması | SAFE-2 sonu CI fix lesson — `env -u RUSTFLAGS` zorunlu |

---

**Section 9 G0–G10 boyunca BAĞLAYICI. Implementation sırasında kriter
ihlali fark edilirse: durdur, fix uygula, gate baştan.**

---

**Sprint hazır. Onaylanırsa G0'dan başla.**
