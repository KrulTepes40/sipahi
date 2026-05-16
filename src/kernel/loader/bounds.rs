//! Loader dst safety check — kernel + wasm_arena range overlap guard.
//!
//! VERIFIES: SNTM-R9 loader_no_kernel_overwrite (Kani-proven).
//! Reserved range = [KERNEL_BASE, KERNEL_BASE + KERNEL_SIZE).
//! KERNEL_SIZE = 0x600000 (6MB) — kernel + stack + task_stacks + wasm_arena
//! (self-test build 4MB) + margin. NATIVE_TASK_BASE = 0x80600000 sonrası.
//! dst..dst+size reserved range ile DİSJOİNT olmalı (manifest validator
//! zaten check_kernel_task_overlap ile boot-time reject, runtime
//! defense-in-depth: cosmic ray / bug-late-injection).

#[must_use]
pub const fn is_safe_load_dst(dst: usize, size: usize) -> bool {
    if size == 0 { return false; }
    let dst_end = match dst.checked_add(size) {
        Some(e) => e,
        None => return false,
    };
    let kbase = crate::common::config::KERNEL_BASE;
    let kend  = match kbase.checked_add(crate::common::config::KERNEL_SIZE) {
        Some(e) => e,
        None => return false,
    };
    // Disjoint check: dst..dst_end kernel range'i ile çakışmamalı.
    dst_end <= kbase || dst >= kend
}
