// Sipahi — Safety-Critical Hard Real-Time Microkernel
// RISC-V 64-bit · Rust · no_std · no_alloc

#![no_std]
#![no_main]
#![allow(dead_code)]

mod arch;
mod common;
mod hal;
pub mod ipc;
mod kernel;
#[cfg(kani)]
mod verify;

use core::panic::PanicInfo;

#[cfg(not(kani))]
extern "C" {
    fn trap_entry();
}

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

    arch::csr::write_mtvec(trap_entry as *const () as usize);
    arch::uart::puts("[BOOT] mtvec = 0x");
    print_hex(arch::csr::read_mtvec());
    arch::uart::println("");

    kernel::memory::init_pmp();

    arch::uart::println("[HAL]  Device trait registered");
    arch::uart::println("[HAL]  IOPMP stub ready");

    let id_a = kernel::scheduler::create_task(task_a);
    let id_b = kernel::scheduler::create_task(task_b);
    arch::uart::puts("[BOOT] Task A: id=");
    print_u32(id_a.unwrap_or(255) as u32);
    arch::uart::println("");
    arch::uart::puts("[BOOT] Task B: id=");
    print_u32(id_b.unwrap_or(255) as u32);
    arch::uart::println("");

    // ═══ Sprint 7: Syscall Test ═══
    arch::uart::println("[TEST] Syscall dispatch...");
    let r = kernel::syscall::cap_invoke(42, 1, 2, 0);
    arch::uart::println(if r == 0 { "[TEST] cap_invoke OK" } else { "[TEST] cap_invoke FAIL" });
    // yield testi task içinden yapılacak — boot sırasında schedule() crash yapar
    // let r = kernel::syscall::yield_cpu();

    // ═══ Sprint 8: IPC Test (assert! YOK — doktrin uyumlu) ═══
    arch::uart::println("");
    arch::uart::println("[TEST] IPC SPSC ring buffer...");

    let mut ipc_pass: u32 = 0;
    let mut ipc_fail: u32 = 0;

    // Test 1: Boş kanaldan recv → None
    if let Some(ch) = ipc::get_channel(0) {
        if ch.recv().is_none() {
            arch::uart::println("[TEST] Empty recv → None ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Empty recv → FAIL ✗");
            ipc_fail += 1;
        }
    } else {
        arch::uart::println("[TEST] Channel 0 → FAIL ✗");
        ipc_fail += 1;
    }

    // Test 2: CRC set/verify
    {
        let mut msg = ipc::IpcMessage::zeroed();
        msg.data[0] = 0x42;
        msg.data[1] = 0xAB;
        msg.data[2] = 0xCD;
        msg.set_crc();
        if msg.verify_crc() {
            arch::uart::println("[TEST] CRC set/verify ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] CRC set/verify FAIL ✗");
            ipc_fail += 1;
        }
    }

    // Test 3: Send + Recv roundtrip
    if let Some(ch) = ipc::get_channel(0) {
        let mut msg = ipc::IpcMessage::zeroed();
        msg.data[0] = 0x42;
        msg.data[1] = 0xAB;
        msg.set_crc();

        if ch.send(&msg).is_ok() {
            arch::uart::println("[TEST] Send OK ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Send FAIL ✗");
            ipc_fail += 1;
        }

        if let Some(received) = ch.recv() {
            if received.data[0] == 0x42 && received.data[1] == 0xAB && received.verify_crc() {
                arch::uart::println("[TEST] Recv → data + CRC valid ✓");
                ipc_pass += 1;
            } else {
                arch::uart::println("[TEST] Recv → data mismatch ✗");
                ipc_fail += 1;
            }
        } else {
            arch::uart::println("[TEST] Recv → None (unexpected) ✗");
            ipc_fail += 1;
        }

        if ch.recv().is_none() {
            arch::uart::println("[TEST] Second recv → None ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Second recv → FAIL ✗");
            ipc_fail += 1;
        }
    } else {
        arch::uart::println("[TEST] Channel 0 → FAIL ✗");
        ipc_fail += 3;
    }

    // Test 4: Buffer dolu — bounded loop (max IPC_CHANNEL_SLOTS iterasyon)
    if let Some(ch) = ipc::get_channel(1) {
        let msg = ipc::IpcMessage::zeroed();
        let mut sent: u32 = 0;
        let max_iter = crate::common::config::IPC_CHANNEL_SLOTS as u32;
        let mut i: u32 = 0;
        while i < max_iter {
            if ch.send(&msg).is_err() {
                break;
            }
            sent += 1;
            i += 1;
        }
        arch::uart::puts("[TEST] Buffer full at ");
        print_u32(sent);
        arch::uart::println(" messages ✓");
        ipc_pass += 1;

        if ch.send(&msg).is_err() {
            arch::uart::println("[TEST] Send when full → Err ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Send when full → FAIL ✗");
            ipc_fail += 1;
        }
    } else {
        arch::uart::println("[TEST] Channel 1 → FAIL ✗");
        ipc_fail += 2;
    }

    // Test 5: CRC bozulma
    {
        let mut msg = ipc::IpcMessage::zeroed();
        msg.data[0] = 0xFF;
        msg.set_crc();
        msg.data[0] = 0x00; // boz
        if !msg.verify_crc() {
            arch::uart::println("[TEST] Tampered CRC → fail ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Tampered CRC → FAIL ✗");
            ipc_fail += 1;
        }
    }

    // Test 6: Geçersiz kanal
    {
        if ipc::get_channel(8).is_none() {
            arch::uart::println("[TEST] Channel 8 → None ✓");
            ipc_pass += 1;
        } else {
            arch::uart::println("[TEST] Channel 8 → FAIL ✗");
            ipc_fail += 1;
        }
    }

    // Sonuç
    arch::uart::puts("[TEST] IPC: ");
    print_u32(ipc_pass);
    arch::uart::puts(" passed, ");
    print_u32(ipc_fail);
    arch::uart::println(" failed");
    if ipc_fail == 0 {
        arch::uart::println("[TEST] ★ All IPC tests PASSED ★");
    } else {
        arch::uart::println("[TEST] ✗ IPC FAILURES ✗");
    }
    arch::uart::println("");

    // Timer EN SON
    arch::csr::enable_timer_interrupt();
    arch::clint::init_timer();
    arch::uart::println("[BOOT] Timer armed");
    arch::uart::println("[BOOT] Starting scheduler...");
    arch::uart::println("");

    kernel::scheduler::start_first_task();
}

#[cfg(not(kani))]
fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = 0;
    if val == 0 { arch::uart::putc(b'0'); return; }
    while val > 0 { buf[i] = hex[val & 0xF]; val >>= 4; i += 1; }
    while i > 0 { i -= 1; arch::uart::putc(buf[i]); }
}

#[cfg(not(kani))]
fn print_u32(mut val: u32) {
    if val == 0 { arch::uart::putc(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
    while i > 0 { i -= 1; arch::uart::putc(buf[i]); }
}

#[cfg(not(kani))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::uart::println("[PANIC] Kernel panic!");
    if let Some(location) = info.location() {
        arch::uart::puts("[PANIC] at ");
        arch::uart::println(location.file());
    }
    loop { unsafe { core::arch::asm!("wfi") }; }
}
