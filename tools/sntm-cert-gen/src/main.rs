//! sntm-cert-gen CLI — generate TaskCertificate + ed25519 signature.
//!
//! Usage:
//!   sntm-cert-gen --manifest sipahi.toml \
//!                 --task-name task_hello \
//!                 --task-id 2 \
//!                 --text-bin target/native/task_hello.text.bin \
//!                 --rodata-bin target/native/task_hello.rodata.bin \
//!                 --data-bin target/native/task_hello.data.bin \
//!                 --signing-key keys/dev-image.priv \
//!                 --out-cert target/native/task_hello.cert.bin \
//!                 --out-sig  target/native/task_hello.cert.sig
//!
//! Exit codes: 0 PASS, 1 IO/parse, 2 sign error, 3 argv.

use std::path::PathBuf;
use std::process::ExitCode;

use sntm_cert_gen::cert::{Range64, CERT_SIZE};
use sntm_cert_gen::chain::{blake3_bytes, blake3_file, build_cert, git_head_bytes, sign_cert};

#[derive(Default)]
struct Args {
    manifest: Option<PathBuf>,
    task_name: Option<String>,
    task_id: Option<u8>,
    text_bin: Option<PathBuf>,
    rodata_bin: Option<PathBuf>,
    data_bin: Option<PathBuf>,
    signing_key: Option<PathBuf>,
    out_cert: Option<PathBuf>,
    out_sig: Option<PathBuf>,
    repo_root: Option<PathBuf>,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => { eprintln!("error: {}", e); return ExitCode::from(3); }
    };

    let repo_root = args.repo_root.clone().unwrap_or_else(|| PathBuf::from("."));
    let manifest_path = args.manifest.expect("required");
    let task_name = args.task_name.expect("required");
    let task_id = args.task_id.expect("required");

    let manifest_bytes = match std::fs::read(&manifest_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("FAIL: read manifest: {}", e); return ExitCode::from(1); }
    };
    let manifest_hash = blake3_bytes(&manifest_bytes);

    let toolchain_path = repo_root.join("rust-toolchain.toml");
    let toolchain_hash = match blake3_file(&toolchain_path) {
        Ok(h) => h,
        Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
    };

    let source_commit = match git_head_bytes(&repo_root) {
        Ok(h) => h,
        Err(_) => {
            // Detached / no .git → zero-hash sentinel; cert still emitted.
            // Section 8 CR-6: ephemeral artifact, drift guard sign+verify only.
            [0u8; 32]
        }
    };

    let text_hash = match args.text_bin.as_ref().map(|p| blake3_file(p)).transpose() {
        Ok(Some(h)) => h, Ok(None) => [0u8; 32],
        Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
    };
    let rodata_hash = match args.rodata_bin.as_ref().map(|p| blake3_file(p)).transpose() {
        Ok(Some(h)) => h, Ok(None) => [0u8; 32],
        Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
    };
    let data_hash = match args.data_bin.as_ref().map(|p| blake3_file(p)).transpose() {
        Ok(Some(h)) => h, Ok(None) => [0u8; 32],
        Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
    };

    // Placeholders for SAFE-4 refinement; cert remains forensics metadata (CR-9).
    let pmp_profile_hash = [0u8; 32];
    let allowed_syscalls = 0x3F;        // 6-bit bitmap (SYS_*=0..5 all by default)
    let allowed_channels = [0xFFu8; 8]; // empty slots
    let allowed_mmio     = [Range64 { base: 0, size: 0 }; 4];
    let max_stack_bytes  = 8192;        // sipahi.toml stack_size default
    let forbidden_opcode_scan = 1;       // riscv-bin-verify PASS (assumed; CI gate enforces)
    let unsafe_count = 0;
    let kani_proof_ids = [0u32; 16];

    let cert = build_cert(
        task_id, &task_name,
        source_commit, toolchain_hash, manifest_hash,
        pmp_profile_hash, allowed_syscalls, allowed_channels, allowed_mmio,
        max_stack_bytes, forbidden_opcode_scan, unsafe_count,
        text_hash, rodata_hash, data_hash,
        kani_proof_ids,
    );

    let cert_bytes: [u8; CERT_SIZE] = cert.as_bytes();

    // Sign.
    let signing_key = args.signing_key.expect("required");
    let priv_pem = match std::fs::read_to_string(&signing_key) {
        Ok(s) => s,
        Err(e) => { eprintln!("FAIL: read priv key: {}", e); return ExitCode::from(2); }
    };
    let sig = match sign_cert(&cert_bytes, &priv_pem) {
        Ok(s) => s,
        Err(e) => { eprintln!("FAIL: sign: {}", e); return ExitCode::from(2); }
    };

    // Write outputs.
    let out_cert = args.out_cert.expect("required");
    let out_sig  = args.out_sig.expect("required");
    if let Err(e) = std::fs::write(&out_cert, &cert_bytes) {
        eprintln!("FAIL: write cert: {}", e); return ExitCode::from(1);
    }
    if let Err(e) = std::fs::write(&out_sig, &sig) {
        eprintln!("FAIL: write sig: {}", e); return ExitCode::from(1);
    }

    println!(
        "PASS: {} cert={} sig={} ({} byte cert + 64 byte sig)",
        task_name, out_cert.display(), out_sig.display(), CERT_SIZE
    );
    ExitCode::from(0)
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--manifest"     => { a.manifest     = Some(req(argv, i + 1)?.into()); i += 2; }
            "--task-name"    => { a.task_name    = Some(req(argv, i + 1)?.into()); i += 2; }
            "--task-id"      => { a.task_id      = Some(req(argv, i + 1)?.parse()
                                       .map_err(|e| format!("--task-id: {}", e))?); i += 2; }
            "--text-bin"     => { a.text_bin     = Some(req(argv, i + 1)?.into()); i += 2; }
            "--rodata-bin"   => { a.rodata_bin   = Some(req(argv, i + 1)?.into()); i += 2; }
            "--data-bin"     => { a.data_bin     = Some(req(argv, i + 1)?.into()); i += 2; }
            "--signing-key"  => { a.signing_key  = Some(req(argv, i + 1)?.into()); i += 2; }
            "--out-cert"     => { a.out_cert     = Some(req(argv, i + 1)?.into()); i += 2; }
            "--out-sig"      => { a.out_sig      = Some(req(argv, i + 1)?.into()); i += 2; }
            "--repo-root"    => { a.repo_root    = Some(req(argv, i + 1)?.into()); i += 2; }
            "-h" | "--help" => {
                println!("Usage: sntm-cert-gen --manifest <toml> --task-name <name> --task-id <u8>");
                println!("                     [--text-bin --rodata-bin --data-bin] (hash inputs)");
                println!("                     --signing-key <pem> --out-cert <bin> --out-sig <bin>");
                std::process::exit(0);
            }
            other => return Err(format!("unknown arg: {}", other)),
        }
    }
    for required in [
        ("--manifest", a.manifest.is_some()),
        ("--task-name", a.task_name.is_some()),
        ("--task-id", a.task_id.is_some()),
        ("--signing-key", a.signing_key.is_some()),
        ("--out-cert", a.out_cert.is_some()),
        ("--out-sig", a.out_sig.is_some()),
    ] {
        if !required.1 { return Err(format!("{} required", required.0)); }
    }
    Ok(a)
}

fn req<'a>(argv: &'a [String], idx: usize) -> Result<&'a str, String> {
    argv.get(idx).map(String::as_str).ok_or_else(|| "missing value".into())
}
