//! sntm-stack integration tests — SAFE-4 Section 8 CR-7 doctrine.
//!
//! Section 9.2 T1-T8: ±çift (synthetic ELF PASS vs FAIL), error message,
//! temp dir izolasyon, drift fail simulation.
//!
//! Plan B özel: rapor format consumer (sntm-validate stackreport.rs) ile
//! anlaşılan key satırlar burada şart koşulur.

mod elf_builder;

use elf_builder::{build_elf, encode_addi_nop, encode_auipc, encode_jalr, encode_ret, Func};
use sntm_stack::{analysis, elf, report};

fn run_analysis(text_base: u64, funcs: Vec<Func>) -> (analysis::AnalysisReport, String) {
    let bytes = build_elf(text_base, &funcs);
    let info = elf::parse(&bytes).expect("synthetic ELF parses");
    let report = analysis::analyze(info);
    let text = report::render(&report, "synthetic.elf");
    (report, text)
}

#[test]
fn synthetic_passes_with_direct_call_only() {
    // main (0x80600000, frame=64) calls helper (0x80600100, frame=16)
    // via auipc/jalr pair.
    let mut main_bytes = Vec::new();
    main_bytes.extend_from_slice(&encode_auipc(1, 0).to_le_bytes());
    main_bytes.extend_from_slice(&encode_jalr(1, 1, 0x100 - 0).to_le_bytes()); // jalr ra, ra, 0x100
    main_bytes.extend_from_slice(&encode_ret().to_le_bytes());
    let helper_bytes = encode_ret().to_le_bytes().to_vec();

    let funcs = vec![
        Func { name: "main".into(),   addr: 0x80600000, bytes: main_bytes,   frame: 64 },
        Func { name: "helper".into(), addr: 0x80600100, bytes: helper_bytes, frame: 16 },
    ];
    let (rep, text) = run_analysis(0x80600000, funcs);
    assert!(matches!(rep.status, analysis::Status::Pass));
    assert_eq!(rep.max_stack_bytes, 80);
    assert!(text.contains("status: PASS"));
    assert!(text.contains("max_stack_bytes: 80"));
    assert!(text.contains("main"));
    assert!(text.contains("helper"));
}

#[test]
fn synthetic_indirect_call_fails_unknown() {
    // main has bare jalr ra, t0, 0 (rs1=5, no AUIPC).
    let mut main_bytes = Vec::new();
    main_bytes.extend_from_slice(&encode_addi_nop().to_le_bytes());
    main_bytes.extend_from_slice(&encode_jalr(1, 5, 0).to_le_bytes()); // indirect call
    main_bytes.extend_from_slice(&encode_ret().to_le_bytes());
    let funcs = vec![Func {
        name: "main".into(), addr: 0x80600000, bytes: main_bytes, frame: 32,
    }];
    let (rep, text) = run_analysis(0x80600000, funcs);
    assert!(matches!(
        rep.status,
        analysis::Status::Fail(analysis::FailReason::IndirectCallDetected)
    ));
    assert_eq!(rep.max_stack_bytes, sntm_stack::UNKNOWN_SENTINEL);
    assert!(text.contains("status: FAIL"));
    assert!(text.contains("reason: indirect call"));
    assert!(text.contains("max_stack_bytes: 0xFFFFFFFF"));
}

#[test]
fn synthetic_recursion_self_call_fails() {
    // main (0x80600000) calls itself via auipc/jalr to 0x80600000.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&encode_auipc(1, 0).to_le_bytes());
    // jalr ra, ra, 0  → target = pc_auipc + 0 = 0x80600000 (self).
    bytes.extend_from_slice(&encode_jalr(1, 1, 0).to_le_bytes());
    bytes.extend_from_slice(&encode_ret().to_le_bytes());
    let funcs = vec![Func {
        name: "main".into(), addr: 0x80600000, bytes, frame: 16,
    }];
    let (rep, text) = run_analysis(0x80600000, funcs);
    assert!(matches!(
        rep.status,
        analysis::Status::Fail(analysis::FailReason::RecursionDetected)
    ));
    assert_eq!(rep.max_stack_bytes, sntm_stack::UNKNOWN_SENTINEL);
    assert!(text.contains("status: FAIL"));
    assert!(text.contains("reason: recursion"));
    assert!(text.contains("main -> main"));
}

#[test]
fn synthetic_report_contains_required_lines_for_parser() {
    // Section 8 CR-3 doctrine: parser sntm-validate stackreport.rs golden
    // fixture'tan türetilir — `max_stack_bytes: <N>` ve `status:` satırları
    // kontrat. Bu test rapor format kontratını koruyor.
    let funcs = vec![Func {
        name: "leaf".into(), addr: 0x80600000,
        bytes: encode_ret().to_le_bytes().to_vec(),
        frame: 32,
    }];
    let (_rep, text) = run_analysis(0x80600000, funcs);
    let mut saw_version = false;
    let mut saw_status = false;
    let mut saw_max_stack = false;
    let mut saw_caveat = false;
    for line in text.lines() {
        if line.starts_with("SNTM-STACK v")    { saw_version = true; }
        if line.starts_with("status:")          { saw_status = true; }
        if line.starts_with("max_stack_bytes:") { saw_max_stack = true; }
        if line.starts_with("caveat:")          { saw_caveat = true; }
    }
    assert!(saw_version);
    assert!(saw_status);
    assert!(saw_max_stack);
    assert!(saw_caveat, "report MUST declare over-approximation caveat");
}

#[test]
fn cli_missing_bin_arg_exits_2() {
    // Host tool doctrine (SAFE-3 CR-15 lesson) — argv eksikse panic değil ExitCode 2.
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-stack");
    let out = Command::new(bin).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2),
        "expected ExitCode 2, got {:?}\nstderr: {}",
        out.status.code(), String::from_utf8_lossy(&out.stderr));
}

#[test]
fn cli_unknown_arg_exits_2() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-stack");
    let out = Command::new(bin).arg("--bogus").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn cli_help_exits_zero() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-stack");
    let out = Command::new(bin).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage:"));
}

#[test]
fn cli_writes_output_file_with_passing_report() {
    use std::process::Command;
    let tmp = tempfile::tempdir().unwrap();
    let elf_path = tmp.path().join("synth.elf");
    let out_path = tmp.path().join("stack.txt");

    let funcs = vec![Func {
        name: "leaf".into(), addr: 0x80600000,
        bytes: encode_ret().to_le_bytes().to_vec(),
        frame: 48,
    }];
    std::fs::write(&elf_path, build_elf(0x80600000, &funcs)).unwrap();

    let bin = env!("CARGO_BIN_EXE_sntm-stack");
    let out = Command::new(bin)
        .arg("--bin").arg(&elf_path)
        .arg("--output").arg(&out_path)
        .output().unwrap();
    assert_eq!(out.status.code(), Some(0));

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("status: PASS"));
    assert!(content.contains("max_stack_bytes: 48"));
}

#[test]
fn elf_missing_stack_sizes_section_fails_gracefully() {
    // ELF without .stack_sizes — sntm-stack returns FAIL with SymbolResolveFailed
    // fallback (via From<ElfError>). We test via direct parse instead.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("no_stacksize.elf");
    // Write a 100-byte garbage file — object crate parse fail.
    std::fs::write(&path, vec![0u8; 100]).unwrap();

    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_sntm-stack");
    let out = Command::new(bin).arg("--bin").arg(&path).output().unwrap();
    // ELF parse fail → tool emits FAIL report and still exits 0 (report generated).
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("status: FAIL"));
    assert!(stdout.contains("max_stack_bytes: 0xFFFFFFFF"));
}
