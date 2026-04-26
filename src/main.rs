//! Sipahi microkernel entry point — boot, init, sprint integration tests.
// Sipahi — Safety-Critical Hard Real-Time Microkernel
// RISC-V 64-bit · Rust · no_std · alloc (WASM sandbox için)

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

// alloc crate — SADECE wasmi sandbox kullanır, kernel kodu KULLANMAZ
extern crate alloc;

mod arch;
mod common;
mod hal;
pub mod ipc;
mod kernel;
mod sandbox;
#[cfg(not(kani))]
mod boot;
// Sprint U-16: Test suite sadece self-test build'de derlenir.
// Production binary minimum yüzey alanı + minimum binary boyutu.
#[cfg(all(not(kani), feature = "self-test"))]
mod tests;
#[cfg(kani)]
mod verify;

// ═══ WASM Bump Allocator — GlobalAlloc ═══
#[cfg(not(kani))]
#[global_allocator]
static ALLOCATOR: sandbox::allocator::BumpAllocator = sandbox::allocator::BumpAllocator;

// OOM handler — panic DEĞİL, wfi loop
#[cfg(not(kani))]
#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! {
    arch::uart::puts("[OOM] WASM arena dolu — offset=");
    print_u32(sandbox::allocator::current_offset() as u32);
    arch::uart::println(" wfi");
    // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
    loop { unsafe { core::arch::asm!("wfi") }; }
}

// ═══ Task fonksiyonları ═══

#[cfg(not(kani))]
pub fn task_a() -> ! {
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-A] tick ");
            print_u32(counter);
            arch::uart::println("");
        }
        // SAFETY: NOP — U-mode'da WFI illegal instruction trap verir (QEMU TW=1).
        unsafe { core::arch::asm!("nop") };
    }
}

#[cfg(not(kani))]
pub fn task_b() -> ! {
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-B] tick ");
            print_u32(counter);
            arch::uart::println("");
        }
        // SAFETY: NOP — U-mode'da WFI illegal instruction trap verir (QEMU TW=1).
        unsafe { core::arch::asm!("nop") };
    }
}

// ═══ Kernel entry point ═══

#[cfg(not(kani))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    arch::uart::println("=============================");
    arch::uart::println("  Sipahi Microkernel v1.5");
    arch::uart::println("  RISC-V 64 · RV64IMAC");
    arch::uart::println("  Safety-Critical RTOS");
    arch::uart::println("=============================");
    arch::uart::println("");
    arch::uart::println("[BOOT] Hart 0 active");
    arch::uart::println("[BOOT] BSS cleared");
    arch::uart::println("[BOOT] Stack initialized");

    boot::init();
    // Sprint U-16: tests::run_all() sadece self-test feature build'de derlenir
    // ve POST + integration + FI suite çalıştırır. Production build (no
    // self-test) doğrudan scheduler'a geçer — minimal attack surface.
    #[cfg(feature = "self-test")]
    tests::run_all();
    boot::start();
}

#[cfg(not(kani))]
use common::fmt::{print_u32, print_hex};

// Suppress unused import warning — print_hex used by boot.rs via crate::common::fmt
#[cfg(not(kani))]
const _: () = { let _ = print_hex as fn(usize); };

#[cfg(not(kani))]
use core::panic::PanicInfo;

#[cfg(not(kani))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::uart::println("[PANIC] Kernel panic!");
    if let Some(location) = info.location() {
        arch::uart::puts("[PANIC] at ");
        arch::uart::println(location.file());
    }
    // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
    loop { unsafe { core::arch::asm!("wfi") }; }
}
