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
