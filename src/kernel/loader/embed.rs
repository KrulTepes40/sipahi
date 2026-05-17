//! Native task image embedding — include_bytes!() macro entry.
//!
//! SNTM design v0.8 §4.6: kernel image task ELF içermez; sntm-pack
//! per-section .bin üretir, kernel build pre-step bunları
//! `target/native/` altına yazar, kernel binary'si compile-time
//! include_bytes!() ile embed eder.
//!
//! U-26 ilk native task: task_hello (task_id=2, FIX-A NATIVE_TASK_BASE 0x80600000).
//! U-27+ yeni task eklenirse: bu dosyaya yeni TASK_X_{TEXT,RODATA,DATA}
//! const'lar + loader/mod.rs içinde generic `load_native_task(task_id)`
//! + dispatch table (U-26 sadece spesifik `load_task_hello()`).

/// task_hello .text segment — RX, base 0x80600000 (FIX-A NATIVE_TASK_BASE).
pub static TASK_HELLO_TEXT: &[u8] =
    include_bytes!("../../../target/native/task_hello.text.bin");

/// task_hello .rodata segment — R, base 0x80604000.
pub static TASK_HELLO_RODATA: &[u8] =
    include_bytes!("../../../target/native/task_hello.rodata.bin");

/// task_hello .data segment — RW, base 0x80605000.
/// data + bss aynı PMP region (.data önce, kalan bss zero-fill loader'da
/// FIX-D: load_region zero_fill ÖNCE, sonra src copy).
pub static TASK_HELLO_DATA: &[u8] =
    include_bytes!("../../../target/native/task_hello.data.bin");

// ─── U-27 SNTM Phase 5: task_world (task_id=3) ────────────────────────

/// task_world .text segment — RX, base 0x80700000 (task_hello + 1MB margin).
pub static TASK_WORLD_TEXT: &[u8] =
    include_bytes!("../../../target/native/task_world.text.bin");

/// task_world .rodata segment — R, base 0x80704000.
pub static TASK_WORLD_RODATA: &[u8] =
    include_bytes!("../../../target/native/task_world.rodata.bin");

/// task_world .data segment — RW, base 0x80705000.
pub static TASK_WORLD_DATA: &[u8] =
    include_bytes!("../../../target/native/task_world.data.bin");
