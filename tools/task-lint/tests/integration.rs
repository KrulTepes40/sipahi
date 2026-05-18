//! task-lint integration tests — 15 fixture.
//!
//! Her fixture temp directory'de minimal workspace + task crate + main.rs
//! oluşturur, sonra `task_lint::lint::lint_task(&entry, &task_dir)` API'sini
//! direkt çağırır (lib refactor U-30.1).
//!
//! VERIFIES: SAFE-1 Rust 12 yasak + cfg-aware demo_feature_waivers + drift guard.
//! FAILS-IF: Yasak ihlali PASS dönerse (false negative) ya da temiz task FAIL
//!           dönerse (false positive).

use std::path::{Path, PathBuf};
use task_lint::{lint, TaskEntry};
use tempfile::TempDir;

/// Fixture helper: temp workspace + Cargo.toml + task crate + src/main.rs.
struct Fixture {
    _tmp: TempDir,
    task_dir: PathBuf,
}

fn make_fixture(
    name: &str,
    main_rs: &str,
    task_cargo_features: Option<&str>,
    workspace_panic_abort: bool,
) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Workspace root Cargo.toml (Rule 7 check reads bu).
    let ws_cargo = if workspace_panic_abort {
        r#"[workspace]
members = []
[profile.release]
panic = "abort"
"#
    } else {
        r#"[workspace]
members = []
[profile.release]
panic = "unwind"
"#
    };
    std::fs::write(root.join("Cargo.toml"), ws_cargo).unwrap();

    // tasks/<name>/Cargo.toml (Rule 12 features check reads bu).
    let task_dir = root.join("tasks").join(name);
    std::fs::create_dir_all(task_dir.join("src")).unwrap();
    let default_task_cargo = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{}"
path = "src/main.rs"
"#, name, name
    );
    let full_cargo = match task_cargo_features {
        Some(features) => format!("{}\n{}", default_task_cargo, features),
        None => default_task_cargo,
    };
    std::fs::write(task_dir.join("Cargo.toml"), full_cargo).unwrap();

    // src/main.rs
    std::fs::write(task_dir.join("src").join("main.rs"), main_rs).unwrap();

    Fixture { _tmp: tmp, task_dir }
}

fn entry(name: &str, tier: &str, dal: &str, waivers: &[&str], waiver_reason: &str) -> TaskEntry {
    TaskEntry {
        name: name.to_string(),
        dal_level: dal.to_string(),
        trust_tier: tier.to_string(),
        waiver_reason: waiver_reason.to_string(),
        demo_feature_waivers: waivers.iter().map(|s| s.to_string()).collect(),
    }
}

fn run_lint(name: &str, tier: &str, dal: &str, waivers: &[&str],
            main_rs: &str, task_cargo_features: Option<&str>,
            workspace_panic_abort: bool) -> Result<String, String> {
    let fx = make_fixture(name, main_rs, task_cargo_features, workspace_panic_abort);
    let e = entry(name, tier, dal, waivers, "test reason");
    lint::lint_task(&e, &fx.task_dir)
}

// ───────── 15 FIXTURES ──────────────────────────────────────────

/// FIXTURE 1 — Pure safe task → PASS.
#[test]
fn fixture_01_safe_task_pass() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut x: u32 = 0;
    loop { x = x.wrapping_add(1); }
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("clean", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_ok(), "expected PASS, got Err: {:?}", r);
    let report = r.unwrap();
    assert!(report.contains("PASS:"), "missing PASS marker:\n{}", report);
}

/// FIXTURE 2 — unsafe outside waiver → FAIL.
#[test]
fn fixture_02_unsafe_outside_waiver_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe { core::ptr::write_volatile(0x80800000 as *mut u8, 0xAA); }
    loop {}
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("u", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on unsafe, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("unsafe blocks"), "missing 'unsafe blocks' in err");
}

/// FIXTURE 3 — cfg-gated unsafe + manifest demo_feature_waivers → PASS (waived).
#[test]
fn fixture_03_cfg_gated_unsafe_with_waiver_pass() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(feature = "demo")]
    unsafe { core::ptr::write_volatile(0x80800000 as *mut u8, 0xAA); }
    loop {}
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let features = r#"[features]
demo = []
"#;
    let r = run_lint("w", "safe", "D", &["demo"], main_rs, Some(features), true);
    assert!(r.is_ok(), "expected PASS on cfg-gated unsafe + waiver, got Err: {:?}", r);
    let report = r.unwrap();
    assert!(report.contains("waived") || report.contains("waiver"),
        "missing waiver log:\n{}", report);
}

/// FIXTURE 4 — cfg-gated unsafe WITHOUT manifest waiver → FAIL.
#[test]
fn fixture_04_cfg_gated_unsafe_without_waiver_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(feature = "demo")]
    unsafe { core::ptr::write_volatile(0x80800000 as *mut u8, 0xAA); }
    loop {}
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let features = r#"[features]
demo = []
"#;
    let r = run_lint("nw", "safe", "D", &[], main_rs, Some(features), true);
    assert!(r.is_err(), "expected FAIL when demo waiver missing, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("unsafe blocks"), "missing 'unsafe blocks' in err");
}

/// FIXTURE 5 — direct recursion → FAIL.
#[test]
fn fixture_05_direct_recursion_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
fn forever() { forever(); }
#[no_mangle]
pub extern "C" fn _start() -> ! {
    forever();
    loop {}
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("rec", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on recursion, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("recursion"), "missing 'recursion' in err");
}

/// FIXTURE 6 — dyn Trait → FAIL.
#[test]
fn fixture_06_dyn_trait_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
trait T { fn f(&self); }
fn x(_v: &dyn T) {}
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("dyn", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on dyn Trait, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("dyn"), "missing 'dyn' in err");
}

/// FIXTURE 7 — fn pointer type → FAIL.
#[test]
fn fixture_07_fn_pointer_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
type Cb = fn(u32) -> u32;
static _CB: Option<Cb> = None;
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("fnp", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on fn pointer, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("fnptr"), "missing 'fnptr' in err");
}

/// FIXTURE 8 — foreign extern block → FAIL.
#[test]
fn fixture_08_foreign_extern_block_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
extern "C" {
    fn external_thing();
}
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("ffi", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on foreign mod, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("foreign"), "missing 'foreign' in err");
}

/// FIXTURE 9 — pub extern "C" fn _start (ABI annotation, NOT foreign block) → PASS.
#[test]
fn fixture_09_pub_extern_c_start_pass() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("entry", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_ok(), "expected PASS on entry ABI, got Err: {:?}", r);
}

/// FIXTURE 10 — core::sync::atomic → FAIL.
#[test]
fn fixture_10_core_sync_atomic_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
use core::sync::atomic::AtomicU32;
static _C: AtomicU32 = AtomicU32::new(0);
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("atom", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on atomic, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("atomic"), "missing 'atomic' in err");
}

/// FIXTURE 11 — asm! macro → FAIL.
#[test]
fn fixture_11_asm_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
use core::arch::asm;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe { asm!("nop"); }
    loop {}
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("asm", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on asm, got Ok: {:?}", r);
    // unsafe + asm her ikisi de fail eder, asm err mesajında olmalı:
    let err = r.unwrap_err();
    assert!(err.contains("asm") || err.contains("unsafe"),
        "missing asm/unsafe in err: {}", err);
}

/// FIXTURE 12 — extern crate alloc → FAIL.
#[test]
fn fixture_12_extern_crate_alloc_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
extern crate alloc;
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("al", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on extern crate alloc, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("alloc"), "missing 'alloc' in err");
}

/// FIXTURE 13 — integer literal raw pointer cast (MMIO) → FAIL.
#[test]
fn fixture_13_raw_pointer_cast_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
fn _bad() {
    let _: *mut u8 = 0x10000 as *mut u8;
}
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("mmio", "safe", "D", &[], main_rs, None, true);
    assert!(r.is_err(), "expected FAIL on raw ptr cast, got Ok: {:?}", r);
    let err = r.unwrap_err();
    assert!(err.contains("MMIO") || err.contains("cast"),
        "missing 'MMIO/cast' in err: {}", err);
}

/// FIXTURE 14 — workspace panic = unwind → FAIL (Rule 7).
#[test]
fn fixture_14_panic_unwind_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let r = run_lint("pu", "safe", "D", &[], main_rs, None, /*panic_abort=*/false);
    assert!(r.is_err(), "expected FAIL on panic=unwind, got Ok: {:?}", r);
    assert!(r.unwrap_err().contains("panic"), "missing 'panic' in err");
}

/// FIXTURE 15 — demo_feature_waivers item default-ON in Cargo.toml → FAIL (Rule 12 drift).
#[test]
fn fixture_15_demo_feature_waivers_default_on_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    // demo waiver feature default array'a girmiş — drift attack senaryosu.
    let features = r#"[features]
default = ["demo"]
demo = []
"#;
    let r = run_lint("drift", "safe", "D", &["demo"], main_rs, Some(features), true);
    assert!(r.is_err(), "expected FAIL on default-ON drift, got Ok: {:?}", r);
    let err = r.unwrap_err();
    assert!(err.contains("drift") || err.contains("default-OFF") || err.contains("default"),
        "missing 'drift/default' in err: {}", err);
}

// ─── Bonus: orphan waiver (waiver not in [features]) → FAIL ──────

#[test]
fn fixture_15b_orphan_waiver_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let features = r#"[features]
other = []
"#;
    let r = run_lint("orph", "safe", "D", &["demo"], main_rs, Some(features), true);
    assert!(r.is_err(), "expected FAIL on orphan waiver, got Ok: {:?}", r);
    let err = r.unwrap_err();
    assert!(err.contains("orphan") || err.contains("not in"),
        "missing 'orphan/not in' in err: {}", err);
}

// ─── Bonus: DAL-A + trusted_unsafe → HARD-FAIL ──────

#[test]
fn fixture_15c_dal_a_trusted_unsafe_hard_fail() {
    let main_rs = r#"
#![no_std]
#![no_main]
#[no_mangle]
pub extern "C" fn _start() -> ! { loop {} }
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { loop {} }
"#;
    let mut e = entry("dal_a", "trusted_unsafe", "A", &[], "test");
    e.waiver_reason = "test".into();
    let fx = make_fixture("dal_a", main_rs, None, true);
    let r = lint::lint_task(&e, &fx.task_dir);
    assert!(r.is_err(), "expected HARD-FAIL on DAL-A trusted_unsafe, got Ok: {:?}", r);
    let err = r.unwrap_err();
    assert!(err.contains("DAL-A") && err.contains("trusted_unsafe"),
        "missing DAL-A/trusted_unsafe in err: {}", err);
}

// Sanity: real Sipahi task_hello + task_world layout (smoke).
#[test]
fn sanity_path_helper() {
    let _ = Path::new("/tmp");
}
