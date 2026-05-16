//! Sipahi SNTM task packer — ELF → per-section .bin (host tool).
//!
//! Usage:
//!   sntm-pack --elf task.elf \
//!             --out-text  task.text.bin \
//!             --out-rodata task.rodata.bin \
//!             --out-data   task.data.bin
//!
//! SNTM design v0.8 §4.8.5: per-section .bin (text/rodata/data ayrı),
//! .bss NOLOAD (kernel boot'ta zero-fill), .stack NOLOAD (RAM reserve).
//!
//! Output exit codes:
//!   0 — PASS (all 3 .bin yazıldı)
//!   1 — ELF parse/section missing/IO error
//!   2 — Argument error

mod pack;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut elf_path:   Option<PathBuf> = None;
    let mut out_text:   Option<PathBuf> = None;
    let mut out_rodata: Option<PathBuf> = None;
    let mut out_data:   Option<PathBuf> = None;
    let mut i = 1;

    // U-26 FIX-E: arg parsing bounds-check (panic yerine ExitCode 2,
    // sntm-validate pattern korunur). Her "--xxx" arg sonrasında değer
    // beklenmesi explicit check.
    macro_rules! consume_arg {
        ($target:ident, $name:expr) => {{
            if i + 1 >= args.len() {
                eprintln!("FAIL: {} requires a path argument", $name);
                return ExitCode::from(2);
            }
            $target = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        }};
    }

    while i < args.len() {
        match args[i].as_str() {
            "--elf"        => consume_arg!(elf_path,   "--elf"),
            "--out-text"   => consume_arg!(out_text,   "--out-text"),
            "--out-rodata" => consume_arg!(out_rodata, "--out-rodata"),
            "--out-data"   => consume_arg!(out_data,   "--out-data"),
            "-h" | "--help" => {
                println!("Usage: sntm-pack --elf X.elf --out-text X.text.bin --out-rodata X.rodata.bin --out-data X.data.bin");
                return ExitCode::from(0);
            }
            other => {
                eprintln!("Unknown arg: {}", other);
                return ExitCode::from(2);
            }
        }
    }

    let (elf, ot, or_, od) = match (elf_path, out_text, out_rodata, out_data) {
        (Some(e), Some(t), Some(r), Some(d)) => (e, t, r, d),
        _ => {
            eprintln!("FAIL: --elf + --out-text + --out-rodata + --out-data zorunlu");
            return ExitCode::from(2);
        }
    };

    match pack::pack_elf(&elf, &ot, &or_, &od) {
        Ok(stats) => {
            println!("PASS: text={}B rodata={}B data={}B",
                stats.text, stats.rodata, stats.data);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("FAIL: {}", e);
            ExitCode::from(1)
        }
    }
}
