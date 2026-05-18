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

/// SAFE-3 (sprint-u32, Section 8 CR-2): task region @ 0x80100000 silent
/// geçiyordu eski 1MB hardcoded KERNEL_SIZE altında — kernel `.task_stacks`
/// (NOLOAD MAX_TASKS×8KB) + `.wasm_arena` + `.bss` ile çakışıyor.
/// Manifest reserved_size=6MB ile bu region artık REJECT olmalı.
///
/// VERIFIES: SAFE-3 CR-2 kernel reserved range invariant.
/// FAILS-IF: validator 0x80100000..0x80104000 region'ı kabul ederse.
#[test]
fn safe3_kernel_overlap_at_1MB_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "below_native_base"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "shadow"
base = 0x80100000
size = 0x4000
perm = "RX"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "0x80100000 region should fail (CR-2 6MB kernel), got code=0\n{}", out);
    assert!(out.to_lowercase().contains("kernel"),
        "expected 'kernel' in output:\n{}", out);
}

/// SAFE-3 CR-2 (positive): explicit reserved_size override edilirse 1MB
/// olur, eski davranışa geri döner — manifest field doğru parse ediliyor.
///
/// VERIFIES: SAFE-3 CR-2 reserved_size manifest field round-trip.
/// FAILS-IF: validator manifest field'ı görmezse, default 6MB her zaman
///           uygulanır → bu test silent fail eder (CR-2 fix yarım).
#[test]
fn safe3_kernel_reserved_size_manifest_override() {
    let mut hdr = HEADER.to_string();
    // Override default 6MB with 1MB — task at 0x80100000 then accepted.
    hdr = hdr.replace(
        "stack_size = 16384",
        "stack_size = 16384\nreserved_size = 0x100000",
    );
    let toml = format!(r#"{hdr}
[[task]]
name = "above_1MB"
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
"#);
    let (code, out) = run(&toml);
    assert_eq!(code, 0, "1MB override should accept 0x80100000, got code={}\n{}", code, out);
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

    // Manifest: 1 task, 2 NAPOT-aligned region. Addresses ≥ NATIVE_TASK_BASE
    // (0x80600000) per SAFE-3 CR-2 6MB kernel reserved range default.
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
base = 0x80600000
size = 0x4000
perm = "RX"

[[task.region]]
name = "stack"
base = 0x80610000
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
    assert!(generated.contains("0x80600000"));
    assert!(generated.contains("0x80610000"));
    assert!(generated.contains("Permission::RX"));
    assert!(generated.contains("Permission::RW"));
    assert!(generated.contains("PmpEncoding::Napot"));

    // Task 1..7 = EMPTY (count emit)
    let empty_count = generated.matches("PmpProfile::EMPTY,").count();
    assert_eq!(empty_count, 7, "expected 7 EMPTY profiles for task 1..7");
}

/// U-27 SNTM-R12 statik kanıt: cross-task region overlap manifest level reject.
///
// VERIFIES: SNTM-R12 (cross-task PMP isolation statik — sntm-validate
//           compile-time overlap reject; runtime trap → isolate U-27.5)
// CALLS:    sntm-validate --manifest <toml> (iki task, overlap region)
// FAILS-IF: Validator iki farklı task'in çakışan region'larını kabul ederse
//           (statik isolation kırılır), ya da hata mesajı 'overlap' içermez.
#[test]
fn cross_task_overlap_rejected() {
    // task_a region: [0x80100000..0x80104000)
    // task_b region: [0x80102000..0x80106000) — overlap at 0x80102000..0x80104000
    let toml = format!(r#"{HEADER}
[[task]]
name = "task_a"
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

[[task]]
name = "task_b"
binary = ""
task_id = 1
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "text"
base = 0x80102000
size = 0x4000
perm = "RX"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "cross-task overlap should fail, got code=0\n{}", out);
    assert!(out.to_lowercase().contains("overlap"),
        "expected 'overlap' in output:\n{}", out);
}

/// U-27 SNTM-R14 prereq: iki task disjoint region accepted (positive case).
///
// VERIFIES: PMP_PROFILES[2]+[3] disjoint manifest accepted.
// CALLS:    sntm-validate --manifest <toml> (task_hello + task_world layout)
// FAILS-IF: Validator disjoint iki task'i overlap diye reject ederse (false
//           positive), kernel-overlap diye reject ederse (FIX-A regression).
#[test]
fn two_tasks_disjoint_accepted() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "task_hello_like"
binary = ""
task_id = 2
priority = 6
period_ticks = 50
budget_cycles = 500000
dal_level = "D"

[[task.region]]
name = "text"
base = 0x80600000
size = 0x4000
perm = "RX"

[[task.region]]
name = "stack"
base = 0x80610000
size = 0x2000
perm = "RW"

[[task]]
name = "task_world_like"
binary = ""
task_id = 3
priority = 7
period_ticks = 50
budget_cycles = 500000
dal_level = "D"

[[task.region]]
name = "text"
base = 0x80700000
size = 0x4000
perm = "RX"

[[task.region]]
name = "stack"
base = 0x80710000
size = 0x2000
perm = "RW"
"#);
    let (code, out) = run(&toml);
    assert_eq!(code, 0, "disjoint two-task should pass, got code={}\n{}", code, out);
}

// ─── U-30.1: demo_feature_waivers Cargo.toml cross-check ──────────

/// Helper: full workspace fixture (manifest + tasks/<name>/Cargo.toml).
/// Returns (exit_code, combined_output).
fn run_with_tasks(toml: &str, task_cargos: &[(&str, &str)]) -> (i32, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("sipahi.toml"), toml).unwrap();
    for (name, cargo) in task_cargos {
        let td = dir.path().join("tasks").join(name);
        std::fs::create_dir_all(&td).unwrap();
        std::fs::write(td.join("Cargo.toml"), cargo).unwrap();
    }
    let out = std::process::Command::new(BIN)
        .arg("--manifest")
        .arg(dir.path().join("sipahi.toml"))
        .output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    (out.status.code().unwrap_or(-1), combined)
}

const FRESH_TASK_CARGO_NO_FEATURES: &str = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"
"#;

const FRESH_TASK_CARGO_WITH_DEMO: &str = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"
[features]
demo = []
"#;

const FRESH_TASK_CARGO_DEFAULT_ON: &str = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"
[features]
default = ["demo"]
demo = []
"#;

const FRESH_TASK_CARGO_ORPHAN: &str = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"
[features]
other = []
"#;

/// VERIFIES: demo_feature_waivers Cargo.toml cross-check positive case.
/// FAILS-IF: Validator waiver=["demo"] + [features.demo=[]] kabul etmezse.
#[test]
fn demo_waiver_present_accepted() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "demo"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"
demo_feature_waivers = ["demo"]
"#);
    let (code, out) = run_with_tasks(&toml, &[("demo", FRESH_TASK_CARGO_WITH_DEMO)]);
    assert_eq!(code, 0, "expected PASS, got code={}\n{}", code, out);
}

/// VERIFIES: demo_feature_waivers default-ON drift FAIL.
/// FAILS-IF: Validator [features.default=["demo"]] + waiver=["demo"] kabul ederse.
#[test]
fn demo_waiver_default_on_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "demo"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"
demo_feature_waivers = ["demo"]
"#);
    let (code, out) = run_with_tasks(&toml, &[("demo", FRESH_TASK_CARGO_DEFAULT_ON)]);
    assert_ne!(code, 0, "default-ON drift should fail, got code=0\n{}", out);
    assert!(out.contains("default-OFF") || out.contains("drift") || out.contains("default"),
        "missing 'drift/default' in output:\n{}", out);
}

/// VERIFIES: orphan waiver (not in [features]) FAIL.
/// FAILS-IF: Validator waiver=["demo"] + [features.other=[]] (no demo) kabul ederse.
#[test]
fn demo_waiver_orphan_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "demo"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"
demo_feature_waivers = ["demo"]
"#);
    let (code, out) = run_with_tasks(&toml, &[("demo", FRESH_TASK_CARGO_ORPHAN)]);
    assert_ne!(code, 0, "orphan waiver should fail, got code=0\n{}", out);
    assert!(out.contains("orphan") || out.contains("not in"),
        "missing 'orphan/not in' in output:\n{}", out);
}

/// VERIFIES: demo_feature_waivers + [features] table missing → FAIL.
#[test]
fn demo_waiver_missing_features_table_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "demo"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"
demo_feature_waivers = ["demo"]
"#);
    let (code, out) = run_with_tasks(&toml, &[("demo", FRESH_TASK_CARGO_NO_FEATURES)]);
    assert_ne!(code, 0, "missing [features] should fail, got code=0\n{}", out);
}

// ─── SAFE-2 (sprint-u31): [[resource]] + [[channel]] invariants ────

const TWO_TASK_HEADER: &str = r#"
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

[[task]]
name = "alpha"
binary = ""
task_id = 2
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"

[[task]]
name = "beta"
binary = ""
task_id = 3
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"
trust_tier = "safe"
"#;

/// VERIFIES: SAFE-2 positive — minimal valid channel + resource accepted.
#[test]
fn safe2_positive_channel_resource_accepted() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[resource]]
id = 0
name = "uart_console"
kind = "device"

[[channel]]
id = 0
producer = "alpha"
consumer = "beta"
message = "Ping"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_eq!(code, 0, "expected PASS, got code={}\n{}", code, out);
}

/// VERIFIES: SAFE-2 — channel producer not in [[task]] rejected (orphan).
#[test]
fn safe2_channel_orphan_producer_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 0
producer = "ghost_task"
consumer = "beta"
message = "Ping"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "ghost producer should fail, got code=0\n{}", out);
    assert!(out.contains("orphan") && out.contains("ghost_task"),
        "missing 'orphan'/'ghost_task' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — channel producer == consumer rejected (self-loop).
#[test]
fn safe2_channel_self_loop_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 0
producer = "alpha"
consumer = "alpha"
message = "Ping"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "self-loop should fail, got code=0\n{}", out);
    assert!(out.contains("self-loop") || out.contains("producer == consumer"),
        "missing 'self-loop' marker in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — duplicate channel id rejected.
#[test]
fn safe2_channel_duplicate_id_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 0
producer = "alpha"
consumer = "beta"
message = "Ping"
size = 8

[[channel]]
id = 0
producer = "beta"
consumer = "alpha"
message = "Pong"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "duplicate id should fail, got code=0\n{}", out);
    assert!(out.contains("duplicate"),
        "missing 'duplicate' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — channel size > IPC_MSG_SIZE(64) rejected.
#[test]
fn safe2_channel_size_overflow_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 0
producer = "alpha"
consumer = "beta"
message = "Ping"
size = 65
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "size > IPC_MSG_SIZE should fail, got code=0\n{}", out);
    assert!(out.contains("IPC_MSG_SIZE") || out.contains("size=65"),
        "missing 'IPC_MSG_SIZE'/'size=65' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — channel message snake_case rejected (PascalCase required).
#[test]
fn safe2_channel_message_snake_case_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 0
producer = "alpha"
consumer = "beta"
message = "greeting_ping"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "snake_case message should fail, got code=0\n{}", out);
    assert!(out.contains("PascalCase") || out.contains("non-alphanumeric")
            || out.contains("uppercase"),
        "missing case-policy marker in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — channel id >= MAX_IPC_CHANNELS(8) rejected.
#[test]
fn safe2_channel_id_too_large_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[channel]]
id = 8
producer = "alpha"
consumer = "beta"
message = "Ping"
size = 8
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "id=8 should fail (>= MAX_IPC_CHANNELS), got code=0\n{}", out);
    assert!(out.contains("MAX_IPC_CHANNELS"),
        "missing 'MAX_IPC_CHANNELS' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — duplicate resource id rejected.
#[test]
fn safe2_resource_duplicate_id_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[resource]]
id = 0
name = "a"
kind = "device"

[[resource]]
id = 0
name = "b"
kind = "device"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "duplicate resource id should fail, got code=0\n{}", out);
    assert!(out.contains("duplicate"),
        "missing 'duplicate' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — resource id >= MAX_RESOURCES(4) rejected.
#[test]
fn safe2_resource_id_too_large_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[resource]]
id = 4
name = "fifth"
kind = "device"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "id=4 should fail (>= MAX_RESOURCES), got code=0\n{}", out);
    assert!(out.contains("MAX_RESOURCES"),
        "missing 'MAX_RESOURCES' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — local_cap referencing undeclared resource rejected (orphan).
#[test]
fn safe2_local_cap_orphan_resource_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[task.local_cap]]
resource_id = 3
action = "Read"
"#);
    // Inserts local_cap on first task (alpha); resource 3 not declared.
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "orphan local_cap should fail, got code=0\n{}", out);
    assert!(out.contains("orphan") && out.contains("resource_id=3"),
        "missing 'orphan'/'resource_id=3' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 — local_cap invalid action rejected (must be enum value).
#[test]
fn safe2_local_cap_invalid_action_rejected() {
    let toml = format!(r#"{TWO_TASK_HEADER}
[[resource]]
id = 0
name = "uart"
kind = "device"

[[task.local_cap]]
resource_id = 0
action = "Admin"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "invalid action should fail, got code=0\n{}", out);
    assert!(out.contains("invalid local_cap action"),
        "missing 'invalid local_cap action' in output:\n{}", out);
}

/// VERIFIES: SAFE-2 DAL-A + trusted_unsafe HARD-FAIL.
#[test]
fn dal_a_trusted_unsafe_rejected() {
    let toml = format!(r#"{HEADER}
[[task]]
name = "dal_a"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "A"
trust_tier = "trusted_unsafe"
waiver_reason = "test"
"#);
    let (code, out) = run(&toml);
    assert_ne!(code, 0, "DAL-A trusted_unsafe should fail, got code=0\n{}", out);
    assert!(out.contains("DAL-A") && out.contains("trusted_unsafe"),
        "missing DAL-A/trusted_unsafe in output:\n{}", out);
}

// ─── SAFE-4 (sprint-u33) Section 8 CR-3/CR-4/CR-5 stack bound tests ─

/// SAFE-4 manifest sample with one valid task + 8KB stack region.
fn manifest_with_one_task(stack_size: usize) -> String {
    format!(r#"{HEADER}
[[task]]
name = "task_x"
binary = ""
task_id = 0
priority = 1
period_ticks = 1
budget_cycles = 1
dal_level = "D"

[[task.region]]
name = "stack"
base = 0x80700000
size = {stack_size}
perm = "RW"
"#)
}

fn run_with_stack_report(toml: &str, report: &str, task_name: &str) -> (i32, String) {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("sipahi.toml");
    let report_path = dir.path().join("task.stack.txt");
    std::fs::write(&manifest_path, toml).unwrap();
    std::fs::write(&report_path, report).unwrap();
    let out = Command::new(BIN)
        .arg("--manifest").arg(&manifest_path)
        .arg("--call-stack-report").arg(&report_path)
        .arg("--task-name").arg(task_name)
        .output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    (out.status.code().unwrap_or(-1), combined)
}

const REAL_GOLDEN_FIXTURE: &str = include_str!(
    "../../sntm-stack/tests/fixtures/task_hello.stack.golden.txt"
);

/// SAFE-4 CR-3: parser sntm-stack golden fixture'a karşı PASS. Format
/// kontrat — banner + status + max_stack_bytes mandatory.
// VERIFIES: SNTM-SAFE-R6 (stackreport.rs parser sntm-stack golden fixture
//           kontratıyla uyumlu — Section 8 CR-3 hardcoded varsayım yok).
// CALLS:    sntm-validate --call-stack-report --task-name; stackreport::parse_max_stack_bytes.
/// FAILS-IF: parser hardcoded varsayım golden fixture'la uyuşmuyor.
#[test]
fn stack_report_parse_real_golden_fixture_accepted() {
    let toml = manifest_with_one_task(8192);
    let (code, out) = run_with_stack_report(&toml, REAL_GOLDEN_FIXTURE, "task_x");
    assert_eq!(code, 0, "real golden fixture should PASS\n{}", out);
    assert!(out.contains("stack bound"), "missing stack bound PASS marker:\n{}", out);
    assert!(out.contains("128"), "expected observed 128 byte in output:\n{}", out);
}

/// SAFE-4 CR-5 negative: exact equality (stack_size == observed) FAIL —
/// margin enforce edilmeli. 4KB stack + 4KB observed + 256 margin = 4352 > 4096.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-5 exact equality PASS doctrine
//           ihlali yakalanır — STACK_ANALYSIS_MARGIN_BYTES enforce).
// CALLS:    validate::check_stack_bounds, STACK_ANALYSIS_MARGIN_BYTES.
/// FAILS-IF: margin enforce edilmiyor (CR-5 ihlali).
#[test]
fn stack_bound_exact_equality_rejected_margin_enforced() {
    let toml = manifest_with_one_task(4096);
    let report = "\
SNTM-STACK v1.0
status: PASS
max_stack_bytes: 4096
";
    let (code, out) = run_with_stack_report(&toml, report, "task_x");
    assert_ne!(code, 0, "exact equality should FAIL (CR-5 margin), got code=0\n{}", out);
    assert!(out.contains("CR-5"), "missing CR-5 reference in error:\n{}", out);
    assert!(out.contains("margin"), "missing margin reference:\n{}", out);
}

/// SAFE-4 CR-4: --call-stack-report ile UNKNOWN sentinel asla PASS değil.
/// Manifest stack_size ne kadar büyük olursa olsun, observed sentinel ise reject.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-4 cert fallback yasak — validator
//           simetrisi: UNKNOWN sentinel 0xFFFF_FFFF her zaman reject).
// CALLS:    validate::check_stack_bounds, STACK_UNKNOWN_SENTINEL.
/// FAILS-IF: validator UNKNOWN sentinel'i sessizce kabul eder.
#[test]
fn stack_bound_unknown_sentinel_rejected_always() {
    let toml = manifest_with_one_task(65536); // huge stack
    let report = "\
SNTM-STACK v1.0
status: FAIL
reason: indirect call detected
max_stack_bytes: 0xFFFFFFFF
";
    let (code, out) = run_with_stack_report(&toml, report, "task_x");
    assert_ne!(code, 0, "UNKNOWN sentinel should FAIL, got code=0\n{}", out);
    assert!(out.contains("indirect call") || out.contains("FAIL"),
        "expected FAIL/reason in output:\n{}", out);
}

/// SAFE-4 CR-3 negative: malformed report (missing banner) → parser Err.
// VERIFIES: SNTM-SAFE-R6 (Section 8 CR-3 format drift erken yakalanır;
//           sessiz default kabul YOK; banner kontrat zorunlu).
// CALLS:    stackreport::parse_max_stack_bytes banner kontrol.
/// FAILS-IF: parser hardcoded varsayım, malformed input "ok" kabul ederse.
#[test]
fn stack_bound_malformed_report_rejected() {
    let toml = manifest_with_one_task(8192);
    let report = "garbage content\nstatus: PASS\nmax_stack_bytes: 32\n";
    let (code, out) = run_with_stack_report(&toml, report, "task_x");
    assert_ne!(code, 0, "malformed report should FAIL, got code=0\n{}", out);
    assert!(out.contains("banner") || out.contains("parse"),
        "expected parse/banner error:\n{}", out);
}

/// SAFE-4: --call-stack-report verildi ama --task-name eksik → ExitCode 2.
/// Host tool doctrine (SAFE-3 CR-15 lesson) — argv yarısı yasak.
/// VERIFIES: argv kontrolü zorunlu, sessiz default yok.
/// FAILS-IF: yalnız report verilince validator pass eder.
#[test]
fn stack_report_without_task_name_fails_exitcode_2() {
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("sipahi.toml");
    let report_path = dir.path().join("task.stack.txt");
    std::fs::write(&manifest_path, manifest_with_one_task(8192)).unwrap();
    std::fs::write(&report_path, REAL_GOLDEN_FIXTURE).unwrap();
    let out = Command::new(BIN)
        .arg("--manifest").arg(&manifest_path)
        .arg("--call-stack-report").arg(&report_path)
        .output().unwrap();
    assert_eq!(out.status.code(), Some(2),
        "expected ExitCode 2, got {:?}", out.status.code());
}
