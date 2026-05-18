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
    //
    // SAFE-2 (sprint-u31): typed IPC consumer demo. Each pass:
    //   1. local_cap_invoke(channel_greeting, Read) — proves manifest grant
    //      task_world[2]=Read (manifest [[task.local_cap]] in sipahi.toml).
    //   2. recv_greeting_ping() — typed wrapper. Ok(None) on empty channel.
    use sipahi_api::channels;
    const RESOURCE_CHANNEL_GREETING: u8 = 2;
    const ACTION_READ: u8 = 0x01;
    let mut counter: u32 = 0;
    let mut last_seen: u32 = 0;
    loop {
        let _ = syscall::local_cap_invoke(RESOURCE_CHANNEL_GREETING, ACTION_READ);
        if let Ok(Some(msg)) = channels::recv_greeting_ping() {
            let seen = u32::from_le_bytes([
                msg.bytes[0], msg.bytes[1], msg.bytes[2], msg.bytes[3],
            ]);
            last_seen = seen;
        }
        let _ = last_seen; // observed value — placeholder demo sink
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
