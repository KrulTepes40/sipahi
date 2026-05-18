# TaskCertificate ABI v2 — Migration Plan

> SAFE-4 (sprint-u33) G8 doctrine: **doc only**. Bu sprintte ABI değişmez;
> v2 alanları post-SAFE faz CFI sprint'ine planlanır. Migration prosedürü
> burada çizilir ki gerçek kesim **drift-free** olsun.

## Şu anki durum — ABI v1

`tools/sntm-cert-gen/src/cert.rs` `repr(C)` 424 byte layout, `abi_version = 1`.
Field tablosu (CR-8 K8 cross-crate pin):

| Offset | Size | Field                  | Anlam                            |
|-------:|-----:|------------------------|----------------------------------|
|   0    |  1   | `task_id`              | manifest task_id                 |
|   1    |  7   | `_pad1`                | alignment                        |
|   8    | 32   | `task_name_hash`       | BLAKE3(task name)                |
|  40    | 32   | `source_commit`        | git HEAD or zero sentinel        |
|  72    | 32   | `toolchain_hash`       | BLAKE3(rust-toolchain.toml)      |
| 104    | 32   | `manifest_hash`        | BLAKE3(sipahi.toml)              |
| 136    | 32   | `pmp_profile_hash`     | placeholder (SAFE-4 carry)       |
| 168    |  1   | `allowed_syscalls`     | 6-bit bitmap                     |
| 169    |  7   | `_pad2`                | alignment                        |
| 176    |  8   | `allowed_channels`     | 8 × u8 channel id                |
| 184    | 64   | `allowed_mmio`         | 4 × Range64                      |
| 248    |  4   | `max_stack_bytes`      | **SAFE-4 refine** (parsed or UNKNOWN) |
| 252    |  1   | `forbidden_opcode_scan`| riscv-bin-verify PASS bit        |
| 253    |  1   | `_pad3`                | alignment                        |
| 254    |  2   | `unsafe_count`         | task-lint sayım                  |
| 256    | 32   | `text_hash`            | BLAKE3(.text.bin)                |
| 288    | 32   | `rodata_hash`          | BLAKE3(.rodata.bin)              |
| 320    | 32   | `data_hash`            | BLAKE3(.data.bin)                |
| 352    | 64   | `kani_proof_ids`       | 16 × u32                         |
| 416    |  4   | `abi_version` = 1      | ABI marker                       |
| 420    |  4   | `_pad4`                | tail alignment (424 total)       |

## v2 hedefleri (post-SAFE CFI sprint)

### Yeni alanlar (öneri)

| Field                | Size  | Amaç                                                |
|----------------------|------:|-----------------------------------------------------|
| `cfi_landing_pads`   | 32 B  | BLAKE3(landing pad table) — Zicfilp v2.5+ scoped    |
| `cfi_shadow_stack`   | 32 B  | BLAKE3(shadow stack policy) — Zicfiss v2.5+         |
| `pq_sig_alg`         |  1 B  | post-quantum sig algorithm id (0 = ed25519, 1 = LMS) |
| `pq_sig_params`      |  3 B  | algorithm-specific parameter compact                |
| `_pad_v2`            | 60 B  | padding to keep 32-byte alignment + future room     |
| Total                | 128 B | + base 424 = 552 byte ABI v2                        |

### `abi_version = 2` davranışı

- Loader / verify-time tool **abi_version oku → switch**:
  - `= 1` → 424 byte parse (backwards compat)
  - `= 2` → 552 byte parse + v2 fields validated
  - Diğer → reject ("unsupported cert abi version")

## Migration prosedürü

### Step 1 — Schema compile-time

`tools/sntm-cert-gen/src/cert.rs`:
1. `pub const CERT_SIZE_V1: usize = 424;` korunur (eski binary'leri doğrulamak için).
2. Yeni `pub const CERT_SIZE_V2: usize = 552;`.
3. `TaskCertificate` → `TaskCertificateV1` rename + yeni `TaskCertificateV2` struct.
4. `const _: () = assert!(core::mem::size_of::<TaskCertificateV2>() == CERT_SIZE_V2);`
5. `ABI_VERSION` const **2** olur.

### Step 2 — Codegen tarafı

`sntm-cert-gen`:
1. `--abi-version <1|2>` CLI flag (default `2`).
2. v1 yazımı **deprecated warning** + dokümante (eski toolchain için).
3. Yeni v2 alanları: ya gerçek kaynak (Zicfilp landing pad scan) ya da
   zero-sentinel (CFI hardware mevcut değil → reject veya warn).

### Step 3 — Verify tarafı

`sntm-image`, kernel-side cert validator (varsa):
1. `abi_version` oku → branch.
2. v1 cert kabul edilirse **migration window** policy enforce et:
   `MIGRATION_DEADLINE_DATE` const → bu tarihten sonra v1 reject.
3. v2 sig algoritma seçimi `pq_sig_alg` → ed25519 yolu eski, LMS yolu yeni.

### Step 4 — TLA+ + Kani

- `cert_field_layout_pin` Kani harness'i v2 byte budget'a güncellenir (424 → 552).
- TLA+ tarafında `CertAbiVersion` invariant'ı v2 set'ine genişler.
- Negative test: v1 cert v2 verify ile reject (versiyon mismatch detect).

### Step 5 — Image format

`tools/sntm-image/src/format.rs`:
- v1 image: tek cert ABI v1 desteği. **DEPRECATED**.
- v2 image: header bayrak biti (`abi_version_min = 2`) + cert v2 only.
- Coexistence yok — image build aşamasında tek bir ABI sürümü.

## Risk + side-effect

| Risk | Mitigation |
|------|-----------|
| Eski cert blob'ları reject olur | Migration window 6 ay; CI + GHA otomatik regen |
| ABI v2 sig algoritma değişimi | `pq_sig_alg` flag ile dual-path; gradual migration |
| CFI hardware henüz yok | v2 `cfi_*` field'ları zero-sentinel + warn |
| Loader code path branch çoğalır | `abi_version` early reject (clear contract) |

## Carry-forward bağıntılar

- **CFI sprint** (post-SAFE): Zicfilp landing pad table, Zicfiss shadow stack
- **HSM/OTP sprint** (post-SAFE): production sig key — `pq_sig_alg` LMS path için
- **Shared `sntm-manifest` crate** (SAFE-2 FIX-G): cert struct shared crate'e
  taşınır → drift duplicate yok
- **Image registry sprint**: reproducible build, signed cert chain, multi-stage
  promotion → v2 ABI default

## Doc rev tarihçesi

- 2026-05-18 — sprint-u33 G8 ilk taslak, SAFE-4 doctrine içine bakım planı.
  v2 alanları + migration prosedürü çizildi; **kod değişikliği YOK**, sprint
  scope ABI v1 KORUR.
