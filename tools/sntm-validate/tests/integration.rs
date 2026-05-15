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
    // platform.pmp_entries=8 with kernel(6) + 6 task regions = 12 > 8.
    // Override platform.pmp_entries to 8 (default 16 not enough headroom).
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
