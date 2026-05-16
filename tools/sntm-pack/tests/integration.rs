//! sntm-pack integration test — gerçek task_hello ELF ile end-to-end pipeline.
//!
//! VERIFIES: SNTM-R10 (ELF → per-section .bin pipeline)
//! CALLS:    sntm-pack binary --elf <path> --out-text/rodata/data
//! FAILS-IF: task_hello ELF yok ve cargo build başarısız, sntm-pack exit
//!           non-zero, text.bin boş veya ELF magic içeriyor, text > 16K
//!           NAPOT region budget.
//!
//! U-26 FIX-C: SKIP YASAK. Eski versiyon ELF yoksa SKIP'liyordu → false
//! GREEN. Yeni: ya self-build, ya hard FAIL.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_sntm-pack");

#[test]
fn pack_task_hello_elf() {
    // U-26 FIX-C: SKIP YASAK — image assembly sprint'in hedefi, tool test'i
    // GERÇEKTEN çalışmalı. ELF yoksa cargo build self-invoke ile üret;
    // başaramazsa hard FAIL.
    let elf = "../../target/riscv64imac-unknown-none-elf/release/task_hello";
    if !std::path::Path::new(elf).exists() {
        eprintln!("[setup] task_hello ELF yok, cargo build çağrılıyor...");
        let build = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir("../../tasks/task_hello")
            .status()
            .expect("cargo build invocation failed");
        assert!(build.success(),
            "FIX-C: task_hello cargo build FAIL — sntm-pack integration test \
             prerequisite not met. CI must run `bash scripts/build_native_tasks.sh` \
             before `cargo test -p sntm-pack`.");
        assert!(std::path::Path::new(elf).exists(),
            "FIX-C: task_hello build başarılı ama ELF yok (path mismatch)");
    }

    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("text.bin");
    let rodata = dir.path().join("rodata.bin");
    let data = dir.path().join("data.bin");

    let out = Command::new(BIN)
        .arg("--elf").arg(elf)
        .arg("--out-text").arg(&text)
        .arg("--out-rodata").arg(&rodata)
        .arg("--out-data").arg(&data)
        .output().unwrap();
    assert!(out.status.success(),
        "exit={:?}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr));

    // text.bin non-empty + RISC-V machine code OLMALI, ELF header DEĞİL.
    let text_bytes = std::fs::read(&text).unwrap();
    assert!(!text_bytes.is_empty(), "text.bin boş");
    assert!(text_bytes.len() <= 0x4000,
        "text > 16K NAPOT region budget (len={})", text_bytes.len());
    // ELF header magic [0x7F, 'E', 'L', 'F'] OLMAMALI (sadece machine code).
    let has_elf_magic = text_bytes.len() >= 4
        && text_bytes[0] == 0x7F
        && text_bytes[1] == b'E'
        && text_bytes[2] == b'L'
        && text_bytes[3] == b'F';
    assert!(!has_elf_magic,
        "text.bin hâlâ ELF header içeriyor — objcopy raw değil");
}

#[test]
fn arg_parsing_missing_value_returns_exit_2() {
    // U-26 FIX-E: bounds-check, panic değil ExitCode 2.
    let out = Command::new(BIN)
        .arg("--elf")  // değer eksik
        .output().unwrap();
    assert_eq!(out.status.code(), Some(2),
        "Expected exit 2 on missing --elf value, got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr));
}

#[test]
fn arg_parsing_no_args_returns_exit_2() {
    let out = Command::new(BIN).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
