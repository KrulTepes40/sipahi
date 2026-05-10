// U-22 GÖREV 1 [M4]: Linker ↔ config drift detection.
// sipahi.ld ile src/common/config.rs arasındaki sabit uyumsuzluklarını
// build-time'da yakala. Şu an sadece TASK_STACK_SIZE alignment'ı için
// string-match yapılıyor; v1.5'te regex parser eklenebilir.

fn main() {
    let ld = std::fs::read_to_string("sipahi.ld")
        .expect("sipahi.ld not found in repo root");

    // TASK_STACK_SIZE = 8192 (config.rs:35) — linker .task_stacks ALIGN(8192)
    // ile eşleşmeli (PMP NAPOT power-of-two requirement, Entry 8).
    if !ld.contains("ALIGN(8192)") {
        panic!(
            "config drift: sipahi.ld must contain ALIGN(8192) for .task_stacks \
             (TASK_STACK_SIZE = 8192, config.rs:35). \
             PMP NAPOT requires power-of-two alignment."
        );
    }

    // .wasm_arena ALIGN(4096) page-aligned — WASM_HEAP_SIZE = 4MB
    if !ld.contains("ALIGN(4096)") {
        panic!(
            "config drift: sipahi.ld must contain ALIGN(4096) for .wasm_arena \
             (4KB page boundary)."
        );
    }

    println!("cargo:rerun-if-changed=sipahi.ld");
    println!("cargo:rerun-if-changed=src/common/config.rs");
    println!("cargo:rerun-if-changed=build.rs");
}
