//! Sipahi microkernel entry point — boot, init, sprint integration tests.
// Sipahi — Safety-Critical Hard Real-Time Microkernel
// RISC-V 64-bit · Rust · no_std · no_alloc (U-29 v2.0: WASM removed)

#![no_std]
#![no_main]

// U-29 v2.0: alloc bağımlılığı tamamen kaldırıldı.
// - WASM sandbox silindi (sandbox/ klasörü yok)
// - ed25519-dalek → ed25519-compact (no_alloc verify path)
// - #[global_allocator], #[alloc_error_handler], extern crate alloc YOK
// Kernel artık pure no_std + no_alloc — Sipahi doctrine compliance.

// U-21 GÖREV 6 [H2]: Capability MAC key provisioning şart. Pre-fix default
// build secure_boot + provision_key'i sessizce compile-out ediyordu ->
// production binary'de capability sistemi devre dışı kalıyordu.
// `test-keys` (development/QEMU) veya `production-otp` (HSM/OTP, v2.0 stub)
// VEYA Kani build (verification — runtime path yok) zorunlu.
#[cfg(not(any(feature = "test-keys", feature = "production-otp", kani)))]
compile_error!(
    "Sipahi build requires either 'test-keys' (development/CI) or \
     'production-otp' (production HSM/OTP — v2.0). Default features include \
     test-keys; for production deployment use --no-default-features --features \
     production-otp,fast-crypto,fast-sign."
);

mod arch;
mod common;
mod hal;
mod ipc; // U-19 GÖREV 8: pub gereksiz (binary crate, external consumer yok)
mod kernel;
#[cfg(not(kani))]
mod boot;
// Sprint U-16: Test suite sadece self-test build'de derlenir.
// Production binary minimum yüzey alanı + minimum binary boyutu.
#[cfg(all(not(kani), feature = "self-test"))]
mod tests;
#[cfg(kani)]
mod verify;

// ═══ Task fonksiyonları ═══

#[cfg(not(kani))]
pub fn task_a() -> ! {
    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);
        #[cfg(feature = "trace")]
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-A] tick ");
            common::fmt::print_u32(counter);
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
        #[cfg(feature = "trace")]
        if counter.is_multiple_of(50) {
            arch::uart::puts("[TASK-B] tick ");
            common::fmt::print_u32(counter);
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
    #[cfg(feature = "debug-boot")]
    {
        arch::uart::println("=============================");
        arch::uart::println("  Sipahi Microkernel v1.1.1");
        arch::uart::println("  RISC-V 64 · RV64IMAC");
        arch::uart::println("  Safety-Critical RTOS");
        arch::uart::println("=============================");
        arch::uart::println("");
        arch::uart::println("[BOOT] Hart 0 active");
        arch::uart::println("[BOOT] BSS cleared");
        arch::uart::println("[BOOT] Stack initialized");
    }

    boot::init();
    // Sprint U-16: tests::run_all() sadece self-test feature build'de derlenir
    // ve POST + integration + FI suite çalıştırır. Production build (no
    // self-test) doğrudan scheduler'a geçer — minimal attack surface.
    #[cfg(feature = "self-test")]
    tests::run_all();
    boot::start();
}

// U-29 v2.0: print_u32 import kaldırıldı (alloc_error fn silindiği için artık
// kullanılmıyor). boot.rs/scheduler kendi fmt path'lerini tam path ile çağırır.

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
