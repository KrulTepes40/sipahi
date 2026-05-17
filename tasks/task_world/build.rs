// U-27 SNTM Phase 5: task_world build script (task_hello pattern mirror).
// Linker script path'i absolute olarak set eder.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rerun-if-changed={}/task_world.ld", manifest_dir);
    println!("cargo:rustc-link-search=native={}", manifest_dir);
    println!("cargo:rustc-link-arg=-T{}/task_world.ld", manifest_dir);
}
