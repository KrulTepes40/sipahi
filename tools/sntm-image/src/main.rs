//! sntm-image CLI — image assembly + verify.
//!
//! Usage:
//!   sntm-image --manifest sipahi.toml \
//!              --kernel target/.../sipahi \
//!              --task task_hello target/native/task_hello \
//!              --task task_world target/native/task_world \
//!              --signing-key keys/dev-image.priv \
//!              --output target/sipahi-image.bin
//!
//!   sntm-image --verify target/sipahi-image.bin --pubkey keys/dev-image.pub

use std::path::PathBuf;
use std::process::ExitCode;

use sntm_image::format::{assemble, write_image, ImageInputs, TaskPayload, TAIL_SIG_SIZE};
use sntm_image::sign::{sign_image, verify_image};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|a| a == "--verify") {
        return cmd_verify(&argv);
    }
    cmd_assemble(&argv)
}

fn cmd_assemble(argv: &[String]) -> ExitCode {
    let mut manifest: Option<PathBuf> = None;
    let mut kernel: Option<PathBuf> = None;
    let mut tasks: Vec<(String, PathBuf)> = Vec::new();
    let mut signing_key: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 1;
    macro_rules! consume {
        ($i:ident) => {{
            if $i + 1 >= argv.len() { return arg_err("missing value"); }
            let v = argv[$i + 1].clone();
            $i += 2;
            v
        }};
    }
    while i < argv.len() {
        match argv[i].as_str() {
            "--manifest"    => { manifest    = Some(consume!(i).into()); }
            "--kernel"      => { kernel      = Some(consume!(i).into()); }
            "--task"        => {
                if i + 2 >= argv.len() {
                    return arg_err("--task requires <name> <prefix>");
                }
                tasks.push((argv[i + 1].clone(), argv[i + 2].clone().into()));
                i += 3;
            }
            "--signing-key" => { signing_key = Some(consume!(i).into()); }
            "--output"      => { output      = Some(consume!(i).into()); }
            "-h" | "--help" => {
                println!("Usage: sntm-image --manifest <toml> --kernel <bin> \\");
                println!("                  [--task <name> <prefix>]... \\");
                println!("                  --signing-key <pem> --output <image.bin>");
                println!("       sntm-image --verify <image.bin> --pubkey <pem>");
                return ExitCode::from(0);
            }
            other => return arg_err(&format!("unknown arg: {}", other)),
        }
    }
    let manifest    = match manifest    { Some(v) => v, None => return arg_err("--manifest required") };
    let kernel_path = match kernel      { Some(v) => v, None => return arg_err("--kernel required") };
    let signing_key = match signing_key { Some(v) => v, None => return arg_err("--signing-key required") };
    let output      = match output      { Some(v) => v, None => return arg_err("--output required") };

    let manifest_bytes = match std::fs::read(&manifest) {
        Ok(b) => b,
        Err(e) => { eprintln!("FAIL: manifest: {}", e); return ExitCode::from(1); }
    };
    let mut manifest_hash = [0u8; 32];
    manifest_hash.copy_from_slice(blake3::hash(&manifest_bytes).as_bytes());

    let kernel_bytes = match std::fs::read(&kernel_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("FAIL: kernel: {}", e); return ExitCode::from(1); }
    };

    // SAFE-3 (Section 8 CR-14): supply chain doctrine — eksik task section
    // dosyası SESSİZ KABUL EDİLEMEZ. Önceki `.unwrap_or_default()` boş
    // section ile image üretiyordu; cert eski/non-empty hash taşıyabilir →
    // image vs cert content drift, drift guard pipeline (sign+verify) bunu
    // yakalamaz (sig kapsamı image body; cert ayrı). Hard fail zorunlu.
    //
    // .text.bin MUST exist (entry point şart). .rodata/.data zaten sntm-pack
    // 0B dosya üretir (boş section için), missing = explicit pipeline bug.
    let mut payloads = Vec::new();
    for (name, prefix) in &tasks {
        let read_section = |suffix: &str| -> Result<Vec<u8>, String> {
            let path = format!("{}{}", prefix.display(), suffix);
            std::fs::read(&path)
                .map_err(|e| format!("section '{}' missing: {}", path, e))
        };
        let text = match read_section(".text.bin") {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => {
                eprintln!("FAIL: {}.text.bin is empty (entry point required)",
                          prefix.display());
                return ExitCode::from(1);
            }
            Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
        };
        let rodata = match read_section(".rodata.bin") {
            Ok(b) => b,
            Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
        };
        let data = match read_section(".data.bin") {
            Ok(b) => b,
            Err(e) => { eprintln!("FAIL: {}", e); return ExitCode::from(1); }
        };
        let cert   = match std::fs::read(format!("{}.cert.bin", prefix.display())) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("FAIL: cert {}.cert.bin missing: {}", prefix.display(), e);
                return ExitCode::from(1);
            }
        };
        let sig    = match std::fs::read(format!("{}.cert.sig", prefix.display())) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("FAIL: sig {}.cert.sig missing: {}", prefix.display(), e);
                return ExitCode::from(1);
            }
        };
        payloads.push(TaskPayload {
            name: name.clone(),
            task_id: 0,  // image format hash-based identity; cert holds task_id
            text_bin: text,
            rodata_bin: rodata,
            data_bin: data,
            cert_bin: cert,
            cert_sig: sig,
        });
    }

    let inputs = ImageInputs {
        manifest_hash,
        kernel: &kernel_bytes,
        tasks: &payloads,
    };
    let body = match assemble(&inputs) {
        Ok(b) => b,
        Err(e) => { eprintln!("FAIL: assemble: {}", e); return ExitCode::from(1); }
    };

    let priv_pem = match std::fs::read_to_string(&signing_key) {
        Ok(s) => s,
        Err(e) => { eprintln!("FAIL: signing key: {}", e); return ExitCode::from(2); }
    };
    let sig = match sign_image(&body, &priv_pem) {
        Ok(s) => s,
        Err(e) => { eprintln!("FAIL: sign: {}", e); return ExitCode::from(2); }
    };

    if let Err(e) = write_image(&output, &body, &sig) {
        eprintln!("FAIL: write: {}", e);
        return ExitCode::from(1);
    }

    println!(
        "PASS: image assembled {} ({} byte body + 64 byte sig = {} total)",
        output.display(), body.len(), body.len() + TAIL_SIG_SIZE
    );
    let _ = manifest;
    ExitCode::from(0)
}

fn cmd_verify(argv: &[String]) -> ExitCode {
    let mut image: Option<PathBuf> = None;
    let mut pubkey: Option<PathBuf> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--verify" => {
                if i + 1 >= argv.len() { return arg_err("--verify missing value"); }
                image = Some(argv[i + 1].clone().into());
                i += 2;
            }
            "--pubkey" => {
                if i + 1 >= argv.len() { return arg_err("--pubkey missing value"); }
                pubkey = Some(argv[i + 1].clone().into());
                i += 2;
            }
            _ => i += 1,
        }
    }
    // SAFE-3 Section 8 CR-15 (Codex audit): host tool disiplin — verify
    // modunda da panic değil controlled ExitCode 2.
    let image_path  = match image  { Some(v) => v, None => return arg_err("--verify <image.bin> required") };
    let pubkey_path = match pubkey { Some(v) => v, None => return arg_err("--pubkey <pem> required") };

    let image_bytes = match std::fs::read(&image_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("FAIL: read image: {}", e); return ExitCode::from(1); }
    };
    let pub_pem = match std::fs::read_to_string(&pubkey_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("FAIL: read pubkey: {}", e); return ExitCode::from(1); }
    };

    match verify_image(&image_bytes, &pub_pem) {
        Ok(()) => {
            println!("PASS: image {} verifies", image_path.display());
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("FAIL: {}", e);
            ExitCode::from(1)
        }
    }
}

fn arg_err(msg: &str) -> ExitCode {
    eprintln!("error: {}\n  --help for usage", msg);
    ExitCode::from(2)
}
