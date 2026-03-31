// Sipahi — Safety-Critical Hard Real-Time Microkernel
// RISC-V 64-bit · Rust · no_std · no_alloc
//
// Doktrin: %100 deterministic · sıfır heap · sıfır panic
//          sıfır float · sıfır recursion · bounded loops

#![no_std]
#![no_main]
#![allow(dead_code)]

mod arch;
mod common;
mod hal;
mod kernel;
#[cfg(kani)]
mod verify;

use core::panic::PanicInfo;

#[cfg(not(kani))]
extern "C" {
    fn trap_entry();
}

// ═══════════════════════════════════════════════════════
// Demo Tasks
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
fn task_a() -> ! {
    arch::csr::enable_machine_interrupts();
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-A] tick ");
            print_u32(counter);
            arch::uart::println("");
        }
        unsafe { core::arch::asm!("wfi") };
    }
}

#[cfg(not(kani))]
fn task_b() -> ! {
    arch::csr::enable_machine_interrupts();
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-B] tick ");
            print_u32(counter);
            arch::uart::println("");
        }
        unsafe { core::arch::asm!("wfi") };
    }
}

// ═══════════════════════════════════════════════════════
// Kernel Entry Point
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    arch::uart::println("=============================");
    arch::uart::println("  Sipahi Microkernel v0.1.0");
    arch::uart::println("  RISC-V 64 · RV64IMAC");
    arch::uart::println("  Safety-Critical RTOS");
    arch::uart::println("=============================");
    arch::uart::println("");
    arch::uart::println("[BOOT] Hart 0 active");
    arch::uart::println("[BOOT] BSS cleared");
    arch::uart::println("[BOOT] Stack initialized");

    // Trap Handler
    arch::csr::write_mtvec(trap_entry as *const () as usize);
    arch::uart::puts("[BOOT] mtvec = 0x");
    print_hex(arch::csr::read_mtvec());
    arch::uart::println("");

    // PMP
    kernel::memory::init_pmp();

    // HAL — Device trait + IOPMP stub
    arch::uart::println("[HAL]  Device trait registered");
    arch::uart::println("[HAL]  IOPMP stub ready (feature=iopmp)");

    // Tasks
    let id_a = kernel::scheduler::create_task(task_a);
    let id_b = kernel::scheduler::create_task(task_b);
    arch::uart::puts("[BOOT] Task A created: id=");
    print_u32(id_a.unwrap_or(255) as u32);
    arch::uart::println("");
    arch::uart::puts("[BOOT] Task B created: id=");
    print_u32(id_b.unwrap_or(255) as u32);
    arch::uart::println("");

    // Timer EN SON — scheduler hazır
    arch::csr::enable_timer_interrupt();
    arch::clint::init_timer();
    arch::uart::println("[BOOT] Timer armed");
    arch::uart::println("[BOOT] Starting scheduler...");
    arch::uart::println("");

    kernel::scheduler::start_first_task();
}

// ═══════════════════════════════════════════════════════
// Yardımcı yazdırma
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = 0;
    if val == 0 {
        arch::uart::putc(b'0');
        return;
    }
    while val > 0 {
        buf[i] = hex[val & 0xF];
        val >>= 4;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        arch::uart::putc(buf[i]);
    }
}

#[cfg(not(kani))]
fn print_u32(mut val: u32) {
    if val == 0 {
        arch::uart::putc(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        arch::uart::putc(buf[i]);
    }
}

#[cfg(not(kani))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::uart::println("[PANIC] Kernel panic!");
    if let Some(location) = info.location() {
        arch::uart::puts("[PANIC] at ");
        arch::uart::println(location.file());
    }
    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}
