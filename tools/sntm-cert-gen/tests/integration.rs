//! sntm-cert-gen integration tests — Section 8 CR-8 doctrine.
//!
//! Kani crypto kanıtı YAPMAZ (stub false döner, tautology); real crypto
//! property cargo test fixture'ları ile doğrulanır:
//!   - Sign + verify roundtrip (positive)
//!   - Signature tamper → verify FAIL (negative)
//!   - Wrong pubkey → verify FAIL (negative)
//!   - Cert blob tamper → sig verify FAIL (negative)
//!   - Cert deterministic (idempotent rebuild bayt-eşit) — T5 determinism
//!   - ABI version pin — CERT_SIZE drift detect

use sntm_cert_gen::cert::{Range64, TaskCertificate, ABI_VERSION, CERT_SIZE};
use sntm_cert_gen::chain::{
    blake3_bytes, build_cert, sign_cert, verify_cert,
};

fn dev_keypair() -> (String, String) {
    // Test-only deterministic ed25519 keypair generated via openssl
    // (committed once for reproducible CI; production HSM key SAFE-4).
    let priv_pem = std::fs::read_to_string(
        std::env::var("CARGO_MANIFEST_DIR").map(|d| format!("{}/../../keys/dev-image.priv", d))
            .unwrap_or_else(|_| "../../keys/dev-image.priv".into())
    ).expect("keys/dev-image.priv missing — run scripts/gen_dev_key.sh");
    let pub_pem = std::fs::read_to_string(
        std::env::var("CARGO_MANIFEST_DIR").map(|d| format!("{}/../../keys/dev-image.pub", d))
            .unwrap_or_else(|_| "../../keys/dev-image.pub".into())
    ).expect("keys/dev-image.pub missing");
    (priv_pem, pub_pem)
}

fn sample_cert() -> TaskCertificate {
    build_cert(
        2,
        "task_hello",
        [0xAA; 32],
        [0xBB; 32],
        [0xCC; 32],
        [0xDD; 32],
        0x3F,
        [0xFF; 8],
        [Range64 { base: 0, size: 0 }; 4],
        8192,
        1,
        0,
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        [0u32; 16],
    )
}

#[test]
fn cert_abi_version_pin() {
    // ABI v1; struct field eklenince ABI breaking, +1 yap.
    assert_eq!(ABI_VERSION, 1);
    let cert = sample_cert();
    assert_eq!(cert.abi_version, ABI_VERSION);
}

#[test]
fn cert_size_invariant() {
    // CR-8 K8 cross-crate: declared CERT_SIZE == actual size_of.
    assert_eq!(CERT_SIZE, core::mem::size_of::<TaskCertificate>());
    let cert = sample_cert();
    let bytes = cert.as_bytes();
    assert_eq!(bytes.len(), CERT_SIZE);
}

#[test]
fn cert_roundtrip_bytes() {
    let cert = sample_cert();
    let bytes = cert.as_bytes();
    let back = TaskCertificate::from_bytes(&bytes).unwrap();
    assert_eq!(cert, back);
}

#[test]
fn cert_from_bytes_wrong_size() {
    let short = vec![0u8; CERT_SIZE - 1];
    assert!(TaskCertificate::from_bytes(&short).is_none());
    let long = vec![0u8; CERT_SIZE + 1];
    assert!(TaskCertificate::from_bytes(&long).is_none());
}

#[test]
fn cert_deterministic_rebuild() {
    // Section 9.2 T5: aynı input → aynı cert bayt-eşit.
    let a = sample_cert().as_bytes();
    let b = sample_cert().as_bytes();
    assert_eq!(a, b);
}

#[test]
fn cert_sign_verify_roundtrip() {
    // Section 8 CR-8 positive: real crypto property.
    let (priv_pem, pub_pem) = dev_keypair();
    let cert = sample_cert();
    let bytes = cert.as_bytes();
    let sig = sign_cert(&bytes, &priv_pem).expect("sign");
    verify_cert(&bytes, &sig, &pub_pem).expect("verify roundtrip");
}

#[test]
fn cert_verify_tampered_signature_fails() {
    // Section 8 CR-8 negative: signature byte flip → verify FAIL.
    let (priv_pem, pub_pem) = dev_keypair();
    let cert = sample_cert();
    let bytes = cert.as_bytes();
    let mut sig = sign_cert(&bytes, &priv_pem).expect("sign");
    sig[0] ^= 0x01;   // flip one bit
    let result = verify_cert(&bytes, &sig, &pub_pem);
    assert!(result.is_err(), "tampered signature should fail verify");
}

#[test]
fn cert_verify_tampered_blob_fails() {
    // Section 8 CR-8 negative: cert byte flip → verify FAIL.
    let (priv_pem, pub_pem) = dev_keypair();
    let cert = sample_cert();
    let bytes = cert.as_bytes();
    let sig = sign_cert(&bytes, &priv_pem).expect("sign");
    let mut tampered = bytes;
    tampered[100] ^= 0xFF;   // flip a byte in the cert body
    let result = verify_cert(&tampered, &sig, &pub_pem);
    assert!(result.is_err(), "tampered cert should fail verify");
}

#[test]
fn cert_field_change_breaks_signature() {
    // T8 drift simulation: even small task_id change → different sig required.
    let (priv_pem, pub_pem) = dev_keypair();
    let mut cert = sample_cert();
    let bytes_a = cert.as_bytes();
    let sig_a = sign_cert(&bytes_a, &priv_pem).expect("sign");
    cert.task_id = 3;   // mutate one byte
    let bytes_b = cert.as_bytes();
    assert_ne!(bytes_a, bytes_b);
    // Old signature must not verify the new blob.
    let result = verify_cert(&bytes_b, &sig_a, &pub_pem);
    assert!(result.is_err(), "old sig should not verify modified cert");
}

#[test]
fn cert_task_name_hash_is_blake3() {
    // Spot-check: BLAKE3(task_name) field matches blake3_bytes.
    let cert = build_cert(
        2, "task_hello", [0; 32], [0; 32], [0; 32], [0; 32],
        0, [0xFF; 8], [Range64 { base: 0, size: 0 }; 4],
        0, 0, 0, [0; 32], [0; 32], [0; 32], [0u32; 16],
    );
    assert_eq!(cert.task_name_hash, blake3_bytes(b"task_hello"));
}

// ─── SAFE-4 (sprint-u33) Section 8 CR-4 stack report cert flow ──────

/// SAFE-4 CR-4 positive: cert with --call-stack-report → parsed PASS value
/// goes into max_stack_bytes. Manifest stack_size NEVER written.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-4 cert max_stack_bytes refinement —
//           parsed sntm-stack report value → cert field; manifest stack_size
//           fallback YASAK).
// CALLS:    sntm-cert-gen --call-stack-report; stackreport::parse_max_stack_or_unknown.
/// FAILS-IF: cert max_stack_bytes hardcoded 8192 (pre-SAFE-4) or fallback bug.
#[test]
fn cli_cert_with_stack_report_uses_parsed_value() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-cert-gen");
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("sipahi.toml");
    std::fs::write(&manifest, b"[kernel]\nname=\"x\"\nversion=\"1\"\nbinary=\"\"\nstack_size=4096\n[platform]\ntarget=\"riscv64\"\nmachine=\"qemu\"\npmp_entries=16\nram_base=0x80000000\nram_size=0x20000000\n").unwrap();

    let report = tmp.path().join("task.stack.txt");
    std::fs::write(&report, b"SNTM-STACK v1.0\nstatus: PASS\nmax_stack_bytes: 144\n").unwrap();

    let priv_key = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../../keys/dev-image.priv", d))
        .unwrap_or_else(|_| "../../keys/dev-image.priv".into());
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../..", d))
        .unwrap_or_else(|_| "../..".into());

    let cert_path = tmp.path().join("task.cert.bin");
    let sig_path  = tmp.path().join("task.cert.sig");
    let out = Command::new(bin)
        .arg("--repo-root").arg(&repo_root)
        .arg("--manifest").arg(&manifest)
        .arg("--task-name").arg("task_hello")
        .arg("--task-id").arg("2")
        .arg("--signing-key").arg(&priv_key)
        .arg("--out-cert").arg(&cert_path)
        .arg("--out-sig").arg(&sig_path)
        .arg("--call-stack-report").arg(&report)
        .output().unwrap();
    assert!(out.status.success(), "cert-gen should PASS, stderr:\n{}",
        String::from_utf8_lossy(&out.stderr));

    let cert_bytes = std::fs::read(&cert_path).unwrap();
    assert_eq!(cert_bytes.len(), CERT_SIZE);
    let cert = TaskCertificate::from_bytes(&cert_bytes).unwrap();
    assert_eq!(cert.max_stack_bytes, 144, "cert should pick up parsed report value");
}

/// SAFE-4 CR-4 negative: cert WITHOUT --call-stack-report → UNKNOWN sentinel.
/// Manifest stack_size 8192 is **NOT** used as fallback.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-4 — report absent → UNKNOWN_SENTINEL
//           0xFFFF_FFFF; manifest stack_size cert'e ASLA yazılmaz).
// CALLS:    sntm-cert-gen (no --call-stack-report); STACK_UNKNOWN_SENTINEL.
/// FAILS-IF: cert silently uses manifest stack_size or any non-sentinel value.
#[test]
fn cli_cert_without_stack_report_emits_unknown_sentinel() {
    use std::process::Command;
    use sntm_cert_gen::stackreport::UNKNOWN_SENTINEL;
    let bin = env!("CARGO_BIN_EXE_sntm-cert-gen");
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("sipahi.toml");
    std::fs::write(&manifest, b"[kernel]\nname=\"x\"\nversion=\"1\"\nbinary=\"\"\nstack_size=8192\n[platform]\ntarget=\"riscv64\"\nmachine=\"qemu\"\npmp_entries=16\nram_base=0x80000000\nram_size=0x20000000\n").unwrap();

    let priv_key = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../../keys/dev-image.priv", d))
        .unwrap_or_else(|_| "../../keys/dev-image.priv".into());
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../..", d))
        .unwrap_or_else(|_| "../..".into());

    let cert_path = tmp.path().join("task.cert.bin");
    let sig_path  = tmp.path().join("task.cert.sig");
    let out = Command::new(bin)
        .arg("--repo-root").arg(&repo_root)
        .arg("--manifest").arg(&manifest)
        .arg("--task-name").arg("task_hello")
        .arg("--task-id").arg("2")
        .arg("--signing-key").arg(&priv_key)
        .arg("--out-cert").arg(&cert_path)
        .arg("--out-sig").arg(&sig_path)
        .output().unwrap();
    assert!(out.status.success());

    let cert = TaskCertificate::from_bytes(&std::fs::read(&cert_path).unwrap()).unwrap();
    assert_eq!(cert.max_stack_bytes, UNKNOWN_SENTINEL,
        "no report → UNKNOWN sentinel (CR-4 doctrine; manifest stack_size fallback YASAK)");
}

/// SAFE-4 CR-4: --call-stack-report with FAIL status → UNKNOWN sentinel.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-4 — FAIL-status report dosyası bile
//           olsa cert max_stack_bytes UNKNOWN sentinel; status değeri kabul YOK).
// CALLS:    sntm-cert-gen --call-stack-report (FAIL); parse_max_stack_or_unknown.
/// FAILS-IF: cert reads FAIL-status max_stack_bytes value as truth.
#[test]
fn cli_cert_with_failed_stack_report_emits_unknown_sentinel() {
    use std::process::Command;
    use sntm_cert_gen::stackreport::UNKNOWN_SENTINEL;
    let bin = env!("CARGO_BIN_EXE_sntm-cert-gen");
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("sipahi.toml");
    std::fs::write(&manifest, b"[kernel]\nname=\"x\"\nversion=\"1\"\nbinary=\"\"\nstack_size=8192\n[platform]\ntarget=\"riscv64\"\nmachine=\"qemu\"\npmp_entries=16\nram_base=0x80000000\nram_size=0x20000000\n").unwrap();

    let report = tmp.path().join("bad.stack.txt");
    std::fs::write(&report, b"SNTM-STACK v1.0\nstatus: FAIL\nreason: indirect\nmax_stack_bytes: 0xFFFFFFFF\n").unwrap();

    let priv_key = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../../keys/dev-image.priv", d))
        .unwrap_or_else(|_| "../../keys/dev-image.priv".into());
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../..", d))
        .unwrap_or_else(|_| "../..".into());

    let cert_path = tmp.path().join("task.cert.bin");
    let sig_path  = tmp.path().join("task.cert.sig");
    let out = Command::new(bin)
        .arg("--repo-root").arg(&repo_root)
        .arg("--manifest").arg(&manifest)
        .arg("--task-name").arg("task_hello")
        .arg("--task-id").arg("2")
        .arg("--signing-key").arg(&priv_key)
        .arg("--out-cert").arg(&cert_path)
        .arg("--out-sig").arg(&sig_path)
        .arg("--call-stack-report").arg(&report)
        .output().unwrap();
    assert!(out.status.success());

    let cert = TaskCertificate::from_bytes(&std::fs::read(&cert_path).unwrap()).unwrap();
    assert_eq!(cert.max_stack_bytes, UNKNOWN_SENTINEL);
}

/// SAFE-4 CR-4: cert tamper max_stack_bytes byte → verify FAIL (forensics chain).
// VERIFIES: SNTM-SAFE-R6 (cert max_stack_bytes signature kapsamı altında —
//           tamper detect zorunlu, drift sign vs verify YASAK).
// CALLS:    sign_cert, verify_cert, build_cert, TaskCertificate::from_bytes.
/// FAILS-IF: signature doesn't cover the stack field (drift between sign + verify).
#[test]
fn cert_tamper_max_stack_bytes_fails_verify() {
    let (priv_pem, pub_pem) = dev_keypair();
    let cert = build_cert(
        2, "task_hello", [0; 32], [0; 32], [0; 32], [0; 32],
        0x3F, [0xFF; 8], [Range64 { base: 0, size: 0 }; 4],
        128, 1, 0,
        [0; 32], [0; 32], [0; 32], [0u32; 16],
    );
    let bytes = cert.as_bytes();
    let sig = sign_cert(&bytes, &priv_pem).expect("sign");

    // Flip a byte in the max_stack_bytes region. Cert layout: max_stack_bytes
    // sits after allowed_mmio array; locate via re-parse + struct field write.
    let mut tampered = bytes;
    // Re-parse to find the offset deterministically.
    let parsed = TaskCertificate::from_bytes(&tampered).unwrap();
    assert_eq!(parsed.max_stack_bytes, 128);
    // Tamper: change to UNKNOWN_SENTINEL bit pattern in-place. Scan bytes for
    // little-endian 128 (0x80 0x00 0x00 0x00) and flip one — coarse but works
    // because we constructed cert with all other fields non-zero or non-128.
    let needle = 128u32.to_le_bytes();
    let pos = tampered.windows(4).position(|w| w == needle)
        .expect("max_stack_bytes location should appear once");
    tampered[pos] ^= 0xFF;

    let result = verify_cert(&tampered, &sig, &pub_pem);
    assert!(result.is_err(), "tampered max_stack_bytes should fail verify");
}
