//! sntm-validate integration tests — fault injection scenarios.
//!
//! Her invariant için 1+ negative case + 1 positive case.
//! VERIFIES: SNTM-R3 (kernel-overlap + intra/cross-task overlap),
//!           SNTM-R5 (NAPOT alignment), + uniqueness + PMP budget.
//! CALLS:    sntm-validate binary --manifest <toml>
//! FAILS-IF: validator invalid manifest'i kabul ederse (exit 0) ya da
//!           valid manifest'i reddetmesi (exit 1) — tool-side false neg/pos.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_sntm-validate");

fn run(toml_content: &str) -> (i32, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sipahi.toml");
    std::fs::write(&path, toml_content).unwrap();
    let out = Command::new(BIN)
        .arg("--manifest")
        .arg(&path)
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    (out.status.code().unwrap_or(-1), combined)
}

const HEADER: &str = r#"
[kernel]
name = "sipahi"
version = "1.5.0"
binary = "target/sipahi"
stack_size = 16384

[platform]
target = "riscv64imac-unknown-none-elf"
machine = "qemu-virt"
pmp_entries = 16
ram_base = 0x80000000
ram_size = 0x20000000
"#;

#[test]
fn valid_manifest_passes() {
    // Header-only (no tasks) — should pass all invariants trivially.
    let (code, out) = run(HEADER);
    assert_eq!(code, 0, "expected PASS, got code={}\noutput:\n{}", code, out);
    assert!(out.contains("PASS"), "missing PASS marker:\n{}", out);
}

#[test]
fn duplicate_task_id_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "a"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task]]
name = "b"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "duplicate task_id should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("duplicate"),
        "expected 'duplicate' in output:\n{}", out);
}

#[test]
fn napot_alignment_violation_rejected() {
    // size = 6K (0x1800) is not power-of-2 — SNTM-R5 violation.
    let toml = format!(r#"{HEADER}
[[task]]
name = "bad_napot"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "data"
base = 0x80100000
size = 0x1800
perm = "RW"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "non-pow2 size should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("napot"),
        "expected 'NAPOT' in output:\n{}", out);
}

#[test]
fn intra_task_overlap_rejected() {
    // r1 = [0x80100000..0x80102000), r2 = [0x80101000..0x80103000)
    // Half-open intersection at 0x80101000..0x80102000.
    let toml = format!(r#"{HEADER}
[[task]]
name = "overlap_task"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "r1"
base = 0x80100000
size = 0x2000
perm = "RX"

[[task.region]]
name = "r2"
base = 0x80101000
size = 0x2000
perm = "RW"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "intra-task overlap should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("overlap"),
        "expected 'overlap' in output:\n{}", out);
}

#[test]
fn kernel_task_overlap_rejected() {
    // task region in kernel range [0x80000000..0x80100000) — SNTM-R3 critical.
    let toml = format!(r#"{HEADER}
[[task]]
name = "kernel_shadow"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "evil"
base = 0x80080000
size = 0x4000
perm = "RX"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "kernel-task overlap should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("kernel"),
        "expected 'kernel' in output:\n{}", out);
}

#[test]
fn pmp_budget_exceeded_rejected() {
    // U-25 FIX-6: RESERVED_LOW_PMP_ENTRIES=8 (kernel 0..5 + UART 6..7).
    // platform.pmp_entries=8 + 6 task regions → 8+6=14 > 8 fail.
    let mut header_small = HEADER.replace("pmp_entries = 16", "pmp_entries = 8");
    // Add 6 regions to one task. Each region 4K, base aligned.
    header_small.push_str(r#"
[[task]]
name = "big_task"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "r1"
base = 0x80100000
size = 0x1000
perm = "RW"

[[task.region]]
name = "r2"
base = 0x80101000
size = 0x1000
perm = "RW"

[[task.region]]
name = "r3"
base = 0x80102000
size = 0x1000
perm = "RW"

[[task.region]]
name = "r4"
base = 0x80103000
size = 0x1000
perm = "RW"

[[task.region]]
name = "r5"
base = 0x80104000
size = 0x1000
perm = "RW"

[[task.region]]
name = "r6"
base = 0x80105000
size = 0x1000
perm = "RW"
"#);
    let (code, out) = run(&header_small);
    assert_ne!(code, 0, "PMP budget exceed should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("budget"),
        "expected 'budget' in output:\n{}", out);
}

/// U-25 G9 SNTM-R8: --output-rs codegen round-trip.
///
// VERIFIES: SNTM-R8 (manifest → generated.rs content match)
// CALLS:    sntm-validate --manifest <toml> --output-rs <path>
// FAILS-IF: Codegen exit non-zero, output file yok, region_count yanlış,
//           PmpEncoding::Napot/Permission string'leri eksik, ya da
//           PmpProfile::EMPTY task 1..7 için emit edilmemiş.
#[test]
fn output_rs_codegen_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("sipahi.toml");
    let out_path = dir.path().join("generated.rs");

    // Manifest: 1 task, 2 NAPOT-aligned region.
    let toml = format!(r#"{HEADER}
[[task]]
name = "demo"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "text"
base = 0x80100000
size = 0x4000
perm = "RX"

[[task.region]]
name = "stack"
base = 0x80110000
size = 0x2000
perm = "RW"
"#);
    std::fs::write(&manifest_path, &toml).unwrap();

    let out = Command::new(BIN)
        .arg("--manifest").arg(&manifest_path)
        .arg("--output-rs").arg(&out_path)
        .output().unwrap();
    assert!(out.status.success(),
        "exit code {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr));

    let generated = std::fs::read_to_string(&out_path).unwrap();

    // Header + use statements
    assert!(generated.contains("GENERATED FILE — DO NOT EDIT"),
        "missing header in:\n{}", generated);
    assert!(generated.contains("use crate::arch::pmp::PmpEncoding"));
    assert!(generated.contains("pub static PMP_PROFILES: [PmpProfile; 8]"));

    // Task 0 content
    assert!(generated.contains("region_count: 2,"),
        "expected region_count: 2 for task 0\n{}", generated);
    assert!(generated.contains("0x80100000"));
    assert!(generated.contains("0x80110000"));
    assert!(generated.contains("Permission::RX"));
    assert!(generated.contains("Permission::RW"));
    assert!(generated.contains("PmpEncoding::Napot"));

    // Task 1..7 = EMPTY (count emit)
    let empty_count = generated.matches("PmpProfile::EMPTY,").count();
    assert_eq!(empty_count, 7, "expected 7 EMPTY profiles for task 1..7");
}

