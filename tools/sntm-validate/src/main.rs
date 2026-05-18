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
mod stackreport;
mod validate;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut manifest_path: Option<PathBuf> = None;
    let mut output_rs_path: Option<PathBuf> = None;
    let mut output_cap_path: Option<PathBuf> = None;
    let mut output_channels_path: Option<PathBuf> = None;
    let mut call_stack_report: Option<PathBuf> = None;
    let mut task_name: Option<String> = None;
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
            "--call-stack-report" => {
                // SAFE-4 (sprint-u33, Section 8 CR-3+CR-5): sntm-stack rapor parse +
                // check_stack_bounds invariant. SAFE gate'te ZORUNLU.
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --call-stack-report requires a path argument");
                    return ExitCode::from(2);
                }
                call_stack_report = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--task-name" => {
                // SAFE-4 CR-4: --call-stack-report ile birlikte zorunlu — hangi
                // task'a uygulanacağı net olsun.
                if i + 1 >= args.len() {
                    eprintln!("FAIL: --task-name requires a value");
                    return ExitCode::from(2);
                }
                task_name = Some(args[i + 1].clone());
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: sntm-validate --manifest sipahi.toml \\");
                println!("         [--output-rs <pmp_generated.rs>] \\");
                println!("         [--output-cap-table <cap_generated.rs>] \\");
                println!("         [--output-channels <sipahi_api/channels.rs>] \\");
                println!("         [--call-stack-report <task.stack.txt> --task-name <name>]");
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
            // SAFE-4 (sprint-u33) Section 8 CR-3/CR-4/CR-5: stack bound check.
            // Iki flag birlikte verilmeli — yarısı yetmez.
            match (call_stack_report.as_ref(), task_name.as_ref()) {
                (Some(rep_path), Some(tn)) => {
                    let report = match std::fs::read_to_string(rep_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("FAIL: cannot read stack report {}: {}",
                                rep_path.display(), e);
                            return ExitCode::from(1);
                        }
                    };
                    let observed_max = match stackreport::parse_max_stack_bytes(&report) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("FAIL: stack report parse: {}", e);
                            return ExitCode::from(1);
                        }
                    };
                    let task = m.tasks.iter().find(|t| t.name == *tn);
                    let task = match task {
                        Some(t) => t,
                        None => {
                            eprintln!("FAIL: task '{}' not found in manifest", tn);
                            return ExitCode::from(1);
                        }
                    };
                    if let Err(errs) = validate::check_stack_bounds(task, observed_max) {
                        for e in errs { eprintln!("FAIL: {}", e); }
                        return ExitCode::from(1);
                    }
                    println!(
                        "PASS: stack bound — task '{}' observed_max {} byte + margin {} byte ≤ stack region",
                        tn, observed_max,
                        task.stack_margin_override
                            .unwrap_or(validate::STACK_ANALYSIS_MARGIN_BYTES),
                    );
                }
                (Some(_), None) | (None, Some(_)) => {
                    eprintln!("FAIL: --call-stack-report and --task-name must be given together");
                    return ExitCode::from(2);
                }
                (None, None) => {}
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
