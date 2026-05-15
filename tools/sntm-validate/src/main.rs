//! Sipahi SNTM Manifest Validator — host tool.
//!
//! Usage: sntm-validate --manifest sipahi.toml
//!
//! Validates 6 invariants (SNTM-R3, R4, R5 + uniqueness + kernel-overlap + budget):
//!   1. Task ID uniqueness
//!   2. NAPOT alignment (SNTM-R5)
//!   3. Region overlap (intra-task) (SNTM-R3)
//!   4. Region overlap (cross-task) (SNTM-R3)
//!   5. Region overlap (kernel-task) (SNTM-R3 critical)
//!   6. PMP entry budget (kernel 6 + per-task ≤ platform.pmp_entries)

mod manifest;
mod validate;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut manifest_path: Option<PathBuf> = None;
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
            "-h" | "--help" => {
                println!("Usage: sntm-validate --manifest sipahi.toml");
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
            eprintln!("Usage: sntm-validate --manifest sipahi.toml");
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

    match validate::validate_all(&m) {
        Ok(()) => {
            let total_regions: usize =
                m.tasks.iter().map(|t| t.regions.len()).sum();
            println!(
                "PASS: manifest valid ({} tasks, {} regions)",
                m.tasks.len(),
                total_regions,
            );
            // U-24 PLACEHOLDER: generated const tables (PMP_PROFILES)
            // Sprint U-25 hedefi (--output-rs flag eklenecek).
            println!(
                "PLACEHOLDER: generated const tables (PMP_PROFILES) — Sprint U-25 hedefi"
            );
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
