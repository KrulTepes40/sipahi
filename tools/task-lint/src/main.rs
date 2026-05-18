//! Sipahi SNTM-SAFE Safe Native Profile lint tool.
//!
//! SAFE-1 (U-30, Sprint v1.6): 11 Rust yasağı build-time uygular.
//! Pipeline: syn AST cfg-aware traversal → manifest demo_feature_waivers skip
//! → DAL × trust_tier policy matrix.
//!
//! Kullanım:
//!   task-lint --manifest sipahi.toml --tasks-dir tasks/
//!
//! Exit code: 0 = PASS, 1 = FAIL (any task violation).

use std::path::PathBuf;

use task_lint::{lint, Manifest};

fn parse_args() -> (PathBuf, PathBuf) {
    let args: Vec<String> = std::env::args().collect();
    let mut manifest = None;
    let mut tasks_dir = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --manifest requires a value");
                    std::process::exit(2);
                }
                manifest = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--tasks-dir" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --tasks-dir requires a value");
                    std::process::exit(2);
                }
                tasks_dir = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--help" | "-h" => {
                println!("Sipahi SNTM-SAFE task-lint v0.1.0");
                println!("Usage: task-lint --manifest <sipahi.toml> --tasks-dir <tasks/>");
                std::process::exit(0);
            }
            other => {
                eprintln!("error: unknown arg {}", other);
                std::process::exit(2);
            }
        }
    }
    let m = manifest.unwrap_or_else(|| {
        eprintln!("error: --manifest required");
        std::process::exit(2);
    });
    let td = tasks_dir.unwrap_or_else(|| {
        eprintln!("error: --tasks-dir required");
        std::process::exit(2);
    });
    (m, td)
}

fn main() {
    let (manifest_path, tasks_dir) = parse_args();

    let manifest_content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FAIL: cannot read manifest {}: {}", manifest_path.display(), e);
            std::process::exit(2);
        }
    };

    let manifest: Manifest = match toml::from_str(&manifest_content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL: manifest parse error: {}", e);
            std::process::exit(2);
        }
    };

    let mut fail_count = 0;
    for task in &manifest.tasks {
        let task_dir = tasks_dir.join(&task.name);
        if !task_dir.exists() {
            // Not all manifest tasks correspond to a directory (legacy task_a/task_b).
            // Skip silently — task-lint only audits SNTM native tasks with source dirs.
            continue;
        }
        match lint::lint_task(task, &task_dir) {
            Ok(report) => {
                println!("{}", report);
            }
            Err(e) => {
                eprintln!("FAIL: {}: {}", task.name, e);
                fail_count += 1;
            }
        }
    }

    if fail_count > 0 {
        eprintln!("\n{} task(s) failed Safe Native Profile lint", fail_count);
        std::process::exit(1);
    }
    println!("\nPASS: all safe-tier tasks lint clean");
}
