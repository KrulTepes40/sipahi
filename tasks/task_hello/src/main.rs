//! Sipahi SNTM Phase 1 minimal task — yield loop + exit.
//!
//! Bu task'ın amacı: sipahi_api ABI'ını gerçek native task'tan kullanmak,
//! .elf üretmek, build pipeline'ı doğrulamak. Kernel loader Phase 4
//! (Sprint U-26) hedefi — bu task şu an STANDALONE compile ediyor,
//! kernel image'a embed edilmiyor.

#![no_std]
#![no_main]

use sipahi_api::syscall;

/// Task entry point — kernel `mret` hedefi (v1.5+).
///
/// SAFETY: Kernel ensures sp, mepc, mstatus.MPP=U correct.
/// All caller-saved + callee-saved registers cleared (zero scrub —
/// U-21 G7 start_first_task fix). gp/tp = 0 (small-data + TLS yok).
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // No global init — Sipahi forbids static initializers.
    // No gp setup — small-data disabled (-G0).
    // No tp setup — TLS not used in SNTM.
    main_loop()
}

fn main_loop() -> ! {
    // U-27: Production live boot — forever yield, NO auto-exit.
    // U-26'da auto-exit (counter>=1000 → syscall::exit(0)) vardı, U-27'de
    // task_world ikinci native task ile beraber çalıştığında her ikisi de
    // Isolated state'e girdiği için MultiModuleCrash (isolated_count>=2) →
    // SHUTDOWN tetikledi. Forever yield: production'da heartbeat pattern.
    let mut counter: u32 = 0;
    loop {
        syscall::yield_cpu();
        counter = counter.wrapping_add(1);
    }
}

/// Panic handler — Sipahi doctrine: panic = abort.
///
/// .eh_frame discarded (linker /DISCARD/), stack unwinding YOK.
/// Task panic ederse: exit(255) syscall → kernel isolate eder
/// (TaskState::Isolated, scheduler atlar).
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    syscall::exit(255);
}
