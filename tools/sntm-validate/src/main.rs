//! Sipahi SNTM Manifest Validator — host tool.
//!
//! Usage:
//!   sntm-validate --manifest sipahi.toml
//!   sntm-validate --manifest sipahi.toml --output-rs src/kernel/pmp/generated.rs
//!
//! Validates 6 invariants (SNTM-R3, R4, R5 + uniqueness + kernel-overlap + budget):
//!   1. Task ID uniqueness
//!   2. NAPOT alignment (SNTM-R5)
//!   3. Region overlap (intra-task) (SNTM-R3)
//!   4. Region overlap (cross-task) (SNTM-R3)
//!   5. Region overlap (kernel-task) (SNTM-R3 critical)
//!   6. PMP entry budget (reserved + per-task ≤ platform.pmp_entries)
//!
//! With --output-rs: emits src/kernel/pmp/generated.rs with PMP_PROFILES const
//! (manifest-driven build-time table). U-25 SNTM Phase 3 codegen.

mod codegen;
mod manifest;
mod napot;
mod validate;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut manifest_path: Option<PathBuf> = None;
    let mut output_rs_path: Option<PathBuf> = None;
    let mut output_cap_path: Option<PathBuf> = None;
    let mut output_channels_path: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --manifest requires a path argument");
                    return ExitCode::from(2);
                }
                manifest_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--output-rs" => {
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --output-rs requires a path argument");
                    return ExitCode::from(2);
                }
                output_rs_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--output-cap-table" => {
                // SAFE-2 (sprint-u31): emit cap_generated.rs (LOCAL_CAP_TABLE + BOOT_CHANNELS).
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --output-cap-table requires a path argument");
                    return ExitCode::from(2);
                }
                output_cap_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--output-channels" => {
                // SAFE-2 (sprint-u31): emit sipahi_api/src/channels.rs (typed IPC API).
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --output-channels requires a path argument");
                    return ExitCode::from(2);
                }
                output_channels_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: sntm-validate --manifest sipahi.toml \\");
                println!("         [--output-rs <pmp_generated.rs>] \\");
                println!("         [--output-cap-table <cap_generated.rs>] \\");
                println!("         [--output-channels <sipahi_api/channels.rs>]");
                return ExitCode::from(0);
            }
            other => {
                eprintln!("Unknown arg: {}", other);
                return ExitCode::from(2);
            }
        }
    }

    let path = match manifest_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: sntm-validate --manifest sipahi.toml [--output-rs PATH]");
            return ExitCode::from(2);
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAIL: cannot read {}: {}", path.display(), e);
            return ExitCode::from(1);
        }
    };

    let m: manifest::Manifest = match toml::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL: TOML parse error: {}", e);
            return ExitCode::from(1);
        }
    };

    match validate::validate_all(&m, Some(&path)) {
        Ok(()) => {
            let total_regions: usize =
                m.tasks.iter().map(|t| t.regions.len()).sum();
            println!(
                "PASS: manifest valid ({} tasks, {} regions)",
                m.tasks.len(),
                total_regions,
            );
            if let Some(ref out) = output_rs_path {
                if let Err(e) = codegen::generate_pmp_profiles_rs(&m, out) {
                    eprintln!("FAIL: pmp codegen error: {}", e);
                    return ExitCode::from(1);
                }
                println!("PASS: generated {}", out.display());
            }
            if let Some(ref out) = output_cap_path {
                if let Err(e) = codegen::generate_cap_table_rs(&m, out) {
                    eprintln!("FAIL: cap-table codegen error: {}", e);
                    return ExitCode::from(1);
                }
                println!("PASS: generated {}", out.display());
            }
            if let Some(ref out) = output_channels_path {
                if let Err(e) = codegen::generate_channels_rs(&m, out) {
                    eprintln!("FAIL: channels codegen error: {}", e);
                    return ExitCode::from(1);
                }
                println!("PASS: generated {}", out.display());
            }
            ExitCode::from(0)
        }
        Err(errs) => {
            for e in &errs {
                eprintln!("FAIL: {}", e);
            }
            ExitCode::from(1)
        }
    }
}
