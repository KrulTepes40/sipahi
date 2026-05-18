//! Sipahi SNTM Phase 5 second native task — yield loop + heartbeat.
//!
//! task_world (task_id=3) ikinci native SNTM task. task_hello (task_id=2)
//! ile paralel runnable, PMP profile DISJOINT (0x80700000+). FIX-E:
//! production'da silent yield, sadece N yield'de bir heartbeat (budget burn
//! ve UART spam guard).

#![no_std]
#![no_main]
// SAFE-1 (U-30): Pure safe tier — task-lint enforce (manifest trust_tier="safe").
// Source-level forbid(unsafe_code) EKLENMEDİ çünkü rustc 1.82+ unsafe_code
// lint'i no_mangle attribute'unu işaretler (false positive). task-lint
// manifest trust_tier="safe" + AST unsafe count == 0 gate'i ile enforce eder.

use sipahi_api::syscall;

/// Task entry point — kernel `mret` hedefi.
///
/// SAFETY: Kernel ensures sp, mepc, mstatus.MPP=U correct.
/// All registers cleared (zero scrub). gp/tp = 0.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    main_loop()
}

fn main_loop() -> ! {
    // U-27: Production live boot — forever yield, NO auto-exit.
    // task_hello (tasks/task_hello/src/main.rs) ile uyumlu: iki native task
    // aynı anda Isolated → MultiModuleCrash → SHUTDOWN engelini önler.
    let mut counter: u32 = 0;
    loop {
        syscall::yield_cpu();
        counter = counter.wrapping_add(1);
    }
}

/// Panic handler — Sipahi doctrine: panic = abort.
/// task_hello (tasks/task_hello/src/main.rs:43-46) ile uyumlu fail-closed:
/// panic → exit(255) → kernel isolate. NO `loop {}` (silent hang riski).
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    syscall::exit(255);
}
