//! sntm-stack CLI — Plan B static stack analyzer.
//!
//! Usage:
//!   sntm-stack --bin target/.../task_hello --output target/native/task_hello.stack.txt
//!   sntm-stack --bin <path>         (stdout)
//!
//! Exit code 0: tool çalıştı (rapor üretildi). PASS/FAIL içerik raporda.
//! Exit code 1: I/O hatası.
//! Exit code 2: argv hatası — host tool doctrine (SAFE-3 CR-15 lesson).

use std::path::PathBuf;
use std::process::ExitCode;

use sntm_stack::{analysis, elf, report};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let mut bin_path: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--bin" => {
                if i + 1 >= argv.len() { return arg_err("--bin requires value"); }
                bin_path = Some(argv[i + 1].clone().into());
                i += 2;
            }
            "--output" => {
                if i + 1 >= argv.len() { return arg_err("--output requires value"); }
                output = Some(argv[i + 1].clone().into());
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: sntm-stack --bin <elf> [--output <path>]");
                return ExitCode::from(0);
            }
            other => return arg_err(&format!("unknown arg: {}", other)),
        }
    }
    let bin_path = match bin_path {
        Some(v) => v,
        None    => return arg_err("--bin required"),
    };

    let data = match std::fs::read(&bin_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("FAIL: read {}: {}", bin_path.display(), e);
            return ExitCode::from(1);
        }
    };

    let analysis_report = match elf::parse(&data) {
        Ok(info) => analysis::analyze(info),
        Err(e) => {
            eprintln!("WARN: ELF parse: {}", e);
            analysis::AnalysisReport::from(e)
        }
    };

    let text = report::render(&analysis_report, &bin_path.display().to_string());
    if let Some(p) = output {
        if let Err(e) = std::fs::write(&p, text.as_bytes()) {
            eprintln!("FAIL: write {}: {}", p.display(), e);
            return ExitCode::from(1);
        }
        println!("PASS: stack report written to {}", p.display());
    } else {
        print!("{}", text);
    }
    ExitCode::from(0)
}

fn arg_err(msg: &str) -> ExitCode {
    eprintln!("error: {}\n  --help for usage", msg);
    ExitCode::from(2)
}
