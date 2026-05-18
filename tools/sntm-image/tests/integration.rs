//! sntm-image integration tests — Section 8 CR-7 doctrine.
//!
//! Section 9.2 T1-T8:
//!   T1 ±çift: assemble+verify positive vs tampered negative
//!   T2 error msg assert
//!   T4 temp dir isolation
//!   T8 drift fail simulation: byte flip → verify fail

use sntm_image::format::{assemble, ImageInputs, TaskPayload, HEADER_SIZE, TAIL_SIG_SIZE};
use sntm_image::sign::{sign_image, verify_image};

fn dev_keypair() -> (String, String) {
    let priv_pem = std::fs::read_to_string(
        std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| format!("{}/../../keys/dev-image.priv", d))
            .unwrap_or_else(|_| "../../keys/dev-image.priv".into())
    ).expect("dev priv");
    let pub_pem = std::fs::read_to_string(
        std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| format!("{}/../../keys/dev-image.pub", d))
            .unwrap_or_else(|_| "../../keys/dev-image.pub".into())
    ).expect("dev pub");
    (priv_pem, pub_pem)
}

fn make_test_image() -> (Vec<u8>, String, String) {
    let (priv_pem, pub_pem) = dev_keypair();
    let inputs = ImageInputs {
        manifest_hash: [0xCC; 32],
        kernel: &[0x13; 1024],  // 1KB of valid (nop) instructions
        tasks: &[TaskPayload {
            name: "task_a".into(),
            task_id: 2,
            text_bin: vec![0xAA; 100],
            rodata_bin: vec![0xBB; 50],
            data_bin: vec![0xCC; 30],
            cert_bin: vec![0xDD; 424],
            cert_sig: vec![0xEE; 64],
        }],
    };
    let body = assemble(&inputs).unwrap();
    let sig = sign_image(&body, &priv_pem).unwrap();
    let mut image = body.clone();
    image.extend_from_slice(&sig);
    (image, priv_pem, pub_pem)
}

#[test]
fn image_assemble_verify_roundtrip() {
    let (image, _priv, pub_pem) = make_test_image();
    verify_image(&image, &pub_pem).expect("roundtrip verify");
}

#[test]
fn image_header_magic_present() {
    let (image, _, _) = make_test_image();
    assert_eq!(&image[0..5], b"SIPI1");
}

#[test]
fn image_tampered_body_fails() {
    // CR-8 negative: flip one byte in the kernel body → verify FAIL.
    let (mut image, _priv, pub_pem) = make_test_image();
    image[200] ^= 0xFF;   // somewhere in body
    let result = verify_image(&image, &pub_pem);
    assert!(result.is_err(), "tampered body must fail verify");
}

#[test]
fn image_tampered_sig_fails() {
    let (mut image, _priv, pub_pem) = make_test_image();
    let len = image.len();
    image[len - 5] ^= 0x01;   // flip a sig byte
    let result = verify_image(&image, &pub_pem);
    assert!(result.is_err(), "tampered sig must fail verify");
}

#[test]
fn image_tampered_magic_fails() {
    let (mut image, _priv, pub_pem) = make_test_image();
    image[0] = b'X';   // corrupt magic
    let result = verify_image(&image, &pub_pem);
    assert!(result.is_err(), "bad magic must fail verify");
    assert!(
        result.unwrap_err().contains("magic"),
        "error msg should mention magic"
    );
}

#[test]
fn image_too_short_fails() {
    let short = vec![0u8; HEADER_SIZE - 1];
    let (_priv, pub_pem) = dev_keypair();
    assert!(verify_image(&short, &pub_pem).is_err());
}

/// SAFE-3 Section 8 CR-14: missing `.text.bin` MUST fail (supply chain
/// doctrine — silent default empty section yasak).
/// VERIFIES: SAFE-3 CR-14 hard fail on missing task section.
/// FAILS-IF: sntm-image binary exit code 0 returns on missing .text.bin.
#[test]
fn image_missing_text_bin_hard_fails() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-image");
    let tmp = tempfile::tempdir().unwrap();

    // Create minimal manifest + kernel + cert/sig but NO text.bin.
    let manifest = tmp.path().join("sipahi.toml");
    std::fs::write(&manifest, b"[kernel]\nname=\"x\"\nversion=\"1\"\nbinary=\"\"\nstack_size=4096\n[platform]\ntarget=\"riscv64\"\nmachine=\"qemu\"\npmp_entries=16\nram_base=0x80000000\nram_size=0x20000000\n").unwrap();
    let kernel = tmp.path().join("kernel.bin");
    std::fs::write(&kernel, vec![0x13; 1024]).unwrap();

    let prefix = tmp.path().join("ghost_task");
    // Only cert + sig, NO text/rodata/data
    std::fs::write(format!("{}.cert.bin", prefix.display()), vec![0u8; 424]).unwrap();
    std::fs::write(format!("{}.cert.sig", prefix.display()), vec![0u8; 64]).unwrap();

    let priv_key = tmp.path().join("dev.priv");
    let dev_priv = std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/../../keys/dev-image.priv", d))
        .unwrap_or_else(|_| "../../keys/dev-image.priv".into());
    std::fs::copy(&dev_priv, &priv_key).unwrap();

    let output = tmp.path().join("image.bin");
    let out = Command::new(bin)
        .arg("--manifest").arg(&manifest)
        .arg("--kernel").arg(&kernel)
        .arg("--task").arg("ghost_task").arg(&prefix)
        .arg("--signing-key").arg(&priv_key)
        .arg("--output").arg(&output)
        .output().unwrap();
    assert!(
        !out.status.success(),
        "missing text.bin must hard fail, but exit code = {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("text.bin") || stderr.contains("missing"),
        "stderr should mention missing section: {}", stderr
    );
}

/// SAFE-3 Section 8 CR-15: missing required arg → ExitCode 2 (controlled),
/// NOT panic. Host tool doctrine.
/// VERIFIES: SAFE-3 CR-15 controlled exit on missing argv (assemble mode).
/// FAILS-IF: sntm-image panics or exits with code other than 2.
#[test]
fn image_missing_arg_returns_exitcode_2() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-image");
    let out = Command::new(bin)
        .arg("--manifest").arg("/dev/null")
        // intentionally omit --kernel/--signing-key/--output
        .output().unwrap();
    assert!(!out.status.success(), "missing args must fail");
    assert_eq!(
        out.status.code(), Some(2),
        "expected ExitCode 2, got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// SAFE-3 CR-15 (Codex post-audit): verify mode missing --pubkey → ExitCode 2,
/// NOT panic. Önceki implementation `unwrap_or_else(|| panic!(...))` kullanıyordu.
/// VERIFIES: SAFE-3 CR-15 verify mode controlled exit (Codex follow-up).
/// FAILS-IF: sntm-image --verify <path> without --pubkey panics or non-2 exit.
#[test]
fn image_verify_missing_pubkey_returns_exitcode_2() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-image");
    let out = Command::new(bin)
        .arg("--verify").arg("/dev/null")
        // intentionally omit --pubkey
        .output().unwrap();
    assert!(!out.status.success(), "missing --pubkey must fail");
    assert_eq!(
        out.status.code(), Some(2),
        "expected ExitCode 2, got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// SAFE-3 CR-15 (Codex post-audit): verify mode missing --verify image path
/// (only --pubkey) → ExitCode 2.
/// VERIFIES: SAFE-3 CR-15 verify mode controlled exit, symmetric.
/// FAILS-IF: panic or non-2 exit.
#[test]
fn image_verify_missing_image_returns_exitcode_2() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-image");
    // Pass --pubkey + value but no --verify. Need a way to enter verify mode
    // without --verify... cmd_verify is triggered by --verify being in argv.
    // To test "verify mode with missing --verify path", pass --verify alone
    // (no value) — should hit missing value arg_err.
    let out = Command::new(bin)
        .arg("--verify")
        // intentionally omit value AND --pubkey
        .output().unwrap();
    assert!(!out.status.success(), "no args must fail");
    assert_eq!(
        out.status.code(), Some(2),
        "expected ExitCode 2, got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn image_header_offsets_consistent() {
    let (image, _, _) = make_test_image();
    use sntm_image::format::Header;
    let header = Header::from_bytes(&image[..HEADER_SIZE]).unwrap();
    // Sanity: kernel_offset < body_offset < tail_sig_offset
    assert!(header.kernel_offset < header.body_offset);
    assert!(header.body_offset < header.tail_sig_offset);
    assert_eq!(image.len(), header.tail_sig_offset as usize + TAIL_SIG_SIZE);
}
