//! riscv-bin-verify CLI — `--elf <path> --manifest sipahi.toml --task-name <name>`.
//!
//! SAFE-3 (sprint-u32) §17.3. Exit codes:
//!   0 PASS
//!   1 violation(s)
//!   2 IO/parse error / invalid argv

use std::path::PathBuf;
use std::process::ExitCode;

use riscv_bin_verify::{verify_elf, Category};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut elf:       Option<PathBuf> = None;
    let mut manifest:  Option<PathBuf> = None;
    let mut task_name: Option<String>  = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--elf" => {
                if i + 1 >= args.len() { return arg_err("--elf requires path"); }
                elf = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--manifest" => {
                if i + 1 >= args.len() { return arg_err("--manifest requires path"); }
                manifest = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--task-name" => {
                if i + 1 >= args.len() { return arg_err("--task-name requires value"); }
                task_name = Some(args[i + 1].clone());
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: riscv-bin-verify --elf <task.elf> \\");
                println!("                        --manifest sipahi.toml \\");
                println!("                        --task-name <name>");
                return ExitCode::from(0);
            }
            other => return arg_err(&format!("unknown arg: {}", other)),
        }
    }
    let elf       = match elf       { Some(v) => v, None => return arg_err("--elf required") };
    let manifest  = match manifest  { Some(v) => v, None => return arg_err("--manifest required") };
    let task_name = match task_name { Some(v) => v, None => return arg_err("--task-name required") };

    let report = match verify_elf(&elf, &manifest, &task_name) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL: {}", e);
            return ExitCode::from(2);
        }
    };

    if report.passed() {
        println!(
            "PASS: {} ({} bytes, 0 violations)",
            report.task_name, report.elf_bytes
        );
        return ExitCode::from(0);
    }

    eprintln!(
        "FAIL: {} — {} violation(s):",
        report.task_name, report.violations.len()
    );
    for v in &report.violations {
        let cat = match v.category {
            Category::PrivilegedOp     => "opcode",
            Category::FloatOp          => "float",
            Category::ForbiddenSection => "section",
            Category::WxViolation      => "wx",
            Category::Relocation       => "reloc",
            Category::RegionBoundary   => "region",
            Category::KernelRangeJal   => "cfi",
            Category::ParseError       => "parse",
        };
        eprintln!("  [{}] {}", cat, v.message);
    }
    ExitCode::from(1)
}

fn arg_err(msg: &str) -> ExitCode {
    eprintln!("error: {}\n  --help for usage", msg);
    ExitCode::from(2)
}
