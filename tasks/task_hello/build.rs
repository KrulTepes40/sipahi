// U-23 SNTM Phase 1: task_hello build script.
// Linker script path'i absolute olarak set eder — Cargo rustflags
// content'inde variable expansion yapmadığı için CARGO_MANIFEST_DIR'ı
// runtime'da resolve etmek gerek.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rerun-if-changed={}/task_hello.ld", manifest_dir);
    println!("cargo:rustc-link-search=native={}", manifest_dir);
    println!("cargo:rustc-link-arg=-T{}/task_hello.ld", manifest_dir);
}
