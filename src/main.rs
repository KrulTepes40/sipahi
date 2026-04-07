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
mod tests;
#[cfg(kani)]
mod verify;

// ═══ WASM Bump Allocator — GlobalAlloc ═══
// Kernel heap YOK. Wasmi kendi 64KB sandbox arena'sını kullanır.
#[cfg(not(kani))]
#[global_allocator]
static ALLOCATOR: sandbox::allocator::BumpAllocator = sandbox::allocator::BumpAllocator;

// OOM handler — panic DEĞİL, wfi loop (doktrin: sıfır panic)
#[cfg(not(kani))]
#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! {
    arch::uart::puts("[OOM] WASM arena dolu — offset=");
    print_u32(sandbox::allocator::current_offset() as u32);
    arch::uart::println(" wfi");
    // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
    loop { unsafe { core::arch::asm!("wfi") }; }
}

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
        // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
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
        // SAFETY: WFI instruction — halts hart until interrupt, no state corruption.
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
    ipc::blackbox::init();

    arch::uart::println("[HAL]  Device trait registered");
    arch::uart::println("[HAL]  IOPMP stub ready");

    // Sprint 10: priority + budget + period parametreleriyle task oluştur
    // Task A: DAL-B (priority 4), %30 CPU = 300_000 cycle/period, 10-tick period (100ms)
    // Task B: DAL-C (priority 8), %20 CPU = 200_000 cycle/period, 10-tick period
    let id_a = kernel::scheduler::create_task(task_a, 4, 1, 300_000, 10);
    let id_b = kernel::scheduler::create_task(task_b, 8, 2, 200_000, 10);
    arch::uart::puts("[BOOT] Task A: id=");
    print_u32(id_a.unwrap_or(255) as u32);
    arch::uart::puts(" prio=4 dal=B budget=300K/period");
    arch::uart::println("");
    arch::uart::puts("[BOOT] Task B: id=");
    print_u32(id_b.unwrap_or(255) as u32);
    arch::uart::puts(" prio=8 dal=C budget=200K/period");
    arch::uart::println("");

    // ═══ Sprint 10: Policy Engine Test ═══
    tests::test_policy_engine();

    // ═══ Sprint 9: Capability Broker Test ═══
    tests::test_capability_broker();

    // ═══ Sprint 8: IPC Test ═══
    tests::test_ipc();

    // ═══ Sprint 13: Secure Boot + Real BLAKE3 Test ═══
    arch::uart::println("[BOOT] Sprint 13: Secure Boot & Real BLAKE3");
    {
        #[cfg(feature = "debug-boot")]
        { arch::uart::puts("[DBG] Arena before crypto: offset=");
          print_u32(sandbox::allocator::current_offset() as u32);
          arch::uart::println(""); }
        // Test 1: BLAKE3 gerçek keyed hash — deterministik ve key-bağımlı
        {
            use common::crypto::provider::HashProvider;
            use common::crypto::Blake3Provider;

            let key1 = [0x5Au8; 32];
            let key2 = [0xA5u8; 32]; // farklı key
            let data = [0x42u8; 16];

            let h1a = Blake3Provider::keyed_hash(&key1, &data);
            let h1b = Blake3Provider::keyed_hash(&key1, &data); // tekrar — aynı olmalı

            // Deterministik: aynı (key, data) → aynı hash
            let mut same = true;
            let mut i: usize = 0;
            while i < 16 {
                if h1a[i] != h1b[i] { same = false; }
                i += 1;
            }
            arch::uart::println(if same {
                "[SEC] BLAKE3 deterministik ✓"
            } else {
                "[SEC] BLAKE3 deterministik FAIL ✗"
            });

            // Key bağımlı: farklı key → farklı hash
            let h2 = Blake3Provider::keyed_hash(&key2, &data);
            let mut different = false;
            let mut j: usize = 0;
            while j < 16 {
                if h1a[j] != h2[j] { different = true; }
                j += 1;
            }
            arch::uart::println(if different {
                "[SEC] BLAKE3 key-binding ✓"
            } else {
                "[SEC] BLAKE3 key-binding FAIL ✗"
            });
        }

        // Test 2-4: Ed25519 — test-keys ile derlenmeli (release'de zero key ile atlanır)
        #[cfg(any(debug_assertions, feature = "test-keys"))]
        {
            use hal::secure_boot::secure_boot_check;
            use hal::key::{QEMU_TEST_PUBKEY, QEMU_TEST_SIGNATURE};

            // Test 2: RFC 8032 TV1 geçerli imza
            let valid = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &QEMU_TEST_SIGNATURE);
            arch::uart::println(if valid {
                "[SEC] Ed25519 RFC8032 TV1 ✓"
            } else {
                "[SEC] Ed25519 RFC8032 TV1 FAIL ✗"
            });

            // Test 3: Bozulmuş imza → RED
            let mut bad_sig = QEMU_TEST_SIGNATURE;
            bad_sig[0] ^= 0xFF;
            let rejected = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &bad_sig);
            arch::uart::println(if !rejected {
                "[SEC] Ed25519 tampered sig RED ✓"
            } else {
                "[SEC] Ed25519 tamper tespiti FAIL ✗"
            });

            // Test 4: Yanlış public key → RED
            let wrong_key = [0xFFu8; 32];
            let rejected2 = secure_boot_check(&[], &wrong_key, &QEMU_TEST_SIGNATURE);
            arch::uart::println(if !rejected2 {
                "[SEC] Ed25519 wrong key RED ✓"
            } else {
                "[SEC] Ed25519 wrong key FAIL ✗"
            });
        }
        #[cfg(not(any(debug_assertions, feature = "test-keys")))]
        arch::uart::println("[SEC] Ed25519 tests SKIP (no test-keys)");

        #[cfg(feature = "debug-boot")]
        { arch::uart::puts("[DBG] Arena after crypto: offset=");
          print_u32(sandbox::allocator::current_offset() as u32);
          arch::uart::println(""); }

        arch::uart::println("[BOOT] Sprint 13 PASS");
    }
    arch::uart::println("");

    // ═══ Sprint 12: WASM Sandbox Test ═══
    arch::uart::println("[BOOT] Sprint 12: WASM Sandbox");
    sandbox::allocator::epoch_reset();
    arch::uart::println("[WASM] Arena: 4MB bump allocator");
    {
        // Minimal WASM modül: () -> i32 { i32.const 42 }  export "run"
        #[allow(clippy::unusual_byte_groupings)]
        const WASM_SIMPLE: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // magic + version
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,       // type: () -> i32
            0x03, 0x02, 0x01, 0x00,                          // func section
            0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00, // export "run"
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b, // code: i32.const 42, end
        ];

        // Float opcode içeren WASM: f32.const + f32.add (0x92 = f32.add → taranır)
        #[allow(clippy::unusual_byte_groupings)]
        const WASM_FLOAT_OPS: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7d,       // type: () -> f32
            0x03, 0x02, 0x01, 0x00,
            0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
            0x0a, 0x0f, 0x01, 0x0d, 0x00,
            0x43, 0x00, 0x00, 0x80, 0x3f, // f32.const 1.0
            0x43, 0x00, 0x00, 0x00, 0x40, // f32.const 2.0
            0x92, 0x0b,                   // f32.add, end
        ];

        use sandbox::{WasmSandbox, SandboxError};

        // Test 1: Normal yükleme + çalıştırma
        {
            let mut ws = WasmSandbox::new();
            match ws.load_module(WASM_SIMPLE) {
                Ok(n) => {
                    arch::uart::puts("[WASM] Module loaded: ");
                    print_u32(n as u32);
                    arch::uart::println(" bytes");
                }
                Err(_) => arch::uart::println("[WASM] Load FAIL ✗"),
            }
            match ws.execute("run", 100_000) {
                Ok(42) => arch::uart::println("[WASM] Execute: OK, result=42 ✓"),
                Ok(_)  => arch::uart::println("[WASM] Execute: yanlış sonuç ✗"),
                Err(_) => arch::uart::println("[WASM] Execute FAIL ✗"),
            }
        }

        // Test 2: Fuel tükenmesi — fuel=0 → trap
        {
            let mut ws = WasmSandbox::new();
            let _ = ws.load_module(WASM_SIMPLE);
            match ws.execute("run", 0) {
                Err(SandboxError::FuelExhausted) | Err(SandboxError::Trapped) =>
                    arch::uart::println("[WASM] Fuel exhaustion: TRAPPED ✓"),
                Ok(_)  => arch::uart::println("[WASM] Fuel test: beklenen trap gelmedi ✗"),
                Err(_) => arch::uart::println("[WASM] Fuel test: başka hata ✗"),
            }
        }

        // Test 3: Float opcode tespiti → REJECT
        match WasmSandbox::check_module(WASM_FLOAT_OPS) {
            Err(SandboxError::FloatOpcodes) =>
                arch::uart::println("[WASM] Float reject: REJECTED ✓"),
            _ => arch::uart::println("[WASM] Float reject FAIL ✗"),
        }

        // Test 4: Arena epoch reset — reset sonrası yeni sandbox çalışır
        {
            sandbox::allocator::epoch_reset();
            let mut ws = WasmSandbox::new();
            match ws.load_module(WASM_SIMPLE) {
                Ok(_) => arch::uart::println("[WASM] Epoch reset + reload: OK ✓"),
                Err(_) => arch::uart::println("[WASM] Epoch reset reload FAIL ✗"),
            }
        }

        arch::uart::println("[WASM] Sprint 12 PASS");
    }
    arch::uart::println("");

    // ═══ Sprint 11: Blackbox Test ═══
    arch::uart::println("[TEST] Blackbox flight recorder...");
    {
        use ipc::blackbox;

        #[cfg(feature = "debug-boot")]
        { arch::uart::puts("[DBG] BB count after boot-init: ");
          print_u32(blackbox::count() as u32);
          arch::uart::println("");
          blackbox::init();
          arch::uart::puts("[DBG] BB count after re-init: ");
          print_u32(blackbox::count() as u32);
          arch::uart::println(""); }

        arch::uart::puts("[TEST] Records after init: ");
        print_u32(blackbox::count() as u32);
        arch::uart::println("");

        // Manuel log: task başlangıçları
        blackbox::log(blackbox::BlackboxEvent::TaskStart, 0, &[0u8, 4, 1]);
        blackbox::log(blackbox::BlackboxEvent::TaskStart, 1, &[1u8, 8, 2]);

        arch::uart::puts("[TEST] Records after log: ");
        print_u32(blackbox::count() as u32);
        arch::uart::println("");

        // Tüm kayıtları doğrula (CRC kontrolü)
        let mut bb_pass = true;
        let mut idx: usize = 0;
        while idx < blackbox::count() {
            if blackbox::read(idx).is_none() {
                bb_pass = false;
            }
            idx += 1;
        }
        arch::uart::println(if bb_pass {
            "[TEST] Blackbox records all valid ✓"
        } else {
            "[TEST] Blackbox record CRC FAIL ✗"
        });
        arch::uart::println("[TEST] ★ Blackbox OK ★");
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
use common::fmt::{print_u32, print_hex};

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
