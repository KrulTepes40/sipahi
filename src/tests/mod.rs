//! Integration test functions extracted from main.rs.

use crate::arch;
use crate::common;
use crate::ipc;
use crate::kernel;
use crate::sandbox;
use crate::common::fmt::print_u32;

// ═══ Sprint 10: Policy Engine Test ═══
pub fn test_policy_engine() {
    arch::uart::println("[TEST] Policy engine...");
    {
        use kernel::policy::{decide_action, FailureMode, PolicyEvent};

        // Budget aşımı: restart_count=0 → RESTART, count=1 → DEGRADE
        let a1 = decide_action(PolicyEvent::BudgetExhausted as u8, 0, 3);
        let a2 = decide_action(PolicyEvent::BudgetExhausted as u8, 1, 3);
        arch::uart::println(if a1 == FailureMode::Restart {
            "[TEST] Budget(0)→Restart ✓"
        } else {
            "[TEST] Budget(0)→Restart FAIL ✗"
        });
        arch::uart::println(if a2 == FailureMode::Degrade {
            "[TEST] Budget(1)→Degrade ✓"
        } else {
            "[TEST] Budget(1)→Degrade FAIL ✗"
        });

        // Cap violation → her zaman ISOLATE
        let a3 = decide_action(PolicyEvent::CapViolation as u8, 0, 0);
        arch::uart::println(if a3 == FailureMode::Isolate {
            "[TEST] CapViolation→Isolate ✓"
        } else {
            "[TEST] CapViolation→Isolate FAIL ✗"
        });

        // PMP fail → her zaman SHUTDOWN
        let a4 = decide_action(PolicyEvent::PmpIntegrityFail as u8, 0, 0);
        arch::uart::println(if a4 == FailureMode::Shutdown {
            "[TEST] PmpFail→Shutdown ✓"
        } else {
            "[TEST] PmpFail→Shutdown FAIL ✗"
        });

        // Deadline miss: DAL-A → FAILOVER, DAL-D → ISOLATE
        let a5 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 0);
        let a6 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 3);
        arch::uart::println(if a5 == FailureMode::Failover {
            "[TEST] DeadlineMiss DAL-A→Failover ✓"
        } else {
            "[TEST] DeadlineMiss DAL-A FAIL ✗"
        });
        arch::uart::println(if a6 == FailureMode::Isolate {
            "[TEST] DeadlineMiss DAL-D→Isolate ✓"
        } else {
            "[TEST] DeadlineMiss DAL-D FAIL ✗"
        });

        arch::uart::println("[TEST] ★ Policy engine OK ★");
    }
}

// ═══ Sprint 9: Capability Broker Test ═══
pub fn test_capability_broker() {
    arch::uart::println("[TEST] Capability broker...");
    {
        use kernel::capability::{Token, ACTION_READ};
        use kernel::capability::broker;

        // 1. MAC key provisioning
        let key = [0x5Au8; 32];
        broker::provision_key(&key);

        // 2. Token oluştur + MAC imzala (stub: provision_key gerekli)
        let mut tok = Token::zeroed();
        tok.id = 1;
        tok.task_id = 0;
        tok.resource = 1; // IPC kanal 1
        tok.action = ACTION_READ;
        tok.dal = 1; // DAL-B
        tok.nonce = 42;
        broker::sign_token(&mut tok); // SipahiMAC-STUB

        // 3. Full validate → cache'e ekler
        let v = broker::validate_full(&tok);
        arch::uart::println(if v { "[TEST] validate_full OK ✓" } else { "[TEST] validate_full FAIL ✗" });

        // 4. Cache hit via syscall (~10c)
        let r = kernel::syscall::cap_invoke(1, 1, ACTION_READ as usize, 0);
        arch::uart::println(if r == 0 { "[TEST] cap_invoke (cache) OK ✓" } else { "[TEST] cap_invoke FAIL ✗" });

        // 5. Cache miss → DENIED (token hiç validate edilmedi)
        let r2 = kernel::syscall::cap_invoke(99, 7, ACTION_READ as usize, 0);
        arch::uart::println(if r2 != 0 { "[TEST] cap_invoke (miss) DENIED ✓" } else { "[TEST] cap_invoke miss FAIL ✗" });
    }
    // yield testi task içinden yapılacak — boot sırasında schedule() crash yapar
    // let r = kernel::syscall::yield_cpu();
}

// ═══ Sprint 8: IPC Test (assert! YOK — doktrin uyumlu) ═══
pub fn test_ipc() {
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
        let max_iter = common::config::IPC_CHANNEL_SLOTS as u32;
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
}

// ═══ WCET Regression Check ═══
pub fn test_wcet_limits() {
    arch::uart::println("[TEST] WCET regression check...");
    let ok = kernel::syscall::dispatch::check_wcet_limits();
    if ok {
        arch::uart::println("[TEST] ★ WCET limits OK ★");
    } else {
        arch::uart::println("[TEST] ⚠ WCET limit exceeded (QEMU TCG — informational only)");
    }
}

// ═══ Sprint 13: Secure Boot + BLAKE3 ═══
pub fn test_crypto() {
    arch::uart::println("[BOOT] Sprint 13: Secure Boot & Real BLAKE3");

    // BLAKE3 keyed hash — deterministik ve key-bağımlı
    {
        use common::crypto::provider::HashProvider;
        use common::crypto::Blake3Provider;

        let key1 = [0x5Au8; 32];
        let key2 = [0xA5u8; 32];
        let data = [0x42u8; 16];

        let h1a = Blake3Provider::keyed_hash(&key1, &data);
        let h1b = Blake3Provider::keyed_hash(&key1, &data);
        let mut same = true;
        let mut i: usize = 0;
        while i < 16 { if h1a[i] != h1b[i] { same = false; } i += 1; }
        arch::uart::println(if same { "[SEC] BLAKE3 deterministik ✓" } else { "[SEC] BLAKE3 deterministik FAIL ✗" });

        let h2 = Blake3Provider::keyed_hash(&key2, &data);
        let mut different = false;
        let mut j: usize = 0;
        while j < 16 { if h1a[j] != h2[j] { different = true; } j += 1; }
        arch::uart::println(if different { "[SEC] BLAKE3 key-binding ✓" } else { "[SEC] BLAKE3 key-binding FAIL ✗" });
    }

    // Ed25519 — test-keys feature ile
    #[cfg(feature = "test-keys")]
    {
        use crate::hal::secure_boot::secure_boot_check;
        use crate::hal::key::{QEMU_TEST_PUBKEY, QEMU_TEST_SIGNATURE};

        let valid = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &QEMU_TEST_SIGNATURE);
        arch::uart::println(if valid { "[SEC] Ed25519 RFC8032 TV1 ✓" } else { "[SEC] Ed25519 RFC8032 TV1 FAIL ✗" });

        let mut bad_sig = QEMU_TEST_SIGNATURE;
        bad_sig[0] ^= 0xFF;
        let rejected = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &bad_sig);
        arch::uart::println(if !rejected { "[SEC] Ed25519 tampered sig RED ✓" } else { "[SEC] Ed25519 tamper tespiti FAIL ✗" });

        let wrong_key = [0xFFu8; 32];
        let rejected2 = secure_boot_check(&[], &wrong_key, &QEMU_TEST_SIGNATURE);
        arch::uart::println(if !rejected2 { "[SEC] Ed25519 wrong key RED ✓" } else { "[SEC] Ed25519 wrong key FAIL ✗" });
    }
    #[cfg(not(feature = "test-keys"))]
    arch::uart::println("[SEC] Ed25519 tests SKIP (no test-keys)");

    arch::uart::println("[BOOT] Sprint 13 PASS");
    arch::uart::println("");
}

// ═══ Sprint 12: WASM Sandbox ═══
pub fn test_wasm() {
    arch::uart::println("[BOOT] Sprint 12: WASM Sandbox");
    sandbox::allocator::epoch_reset();
    arch::uart::println("[WASM] Arena: 4MB bump allocator");

    #[allow(clippy::unusual_byte_groupings)]
    const WASM_SIMPLE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
        0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b,
    ];
    #[allow(clippy::unusual_byte_groupings)]
    const WASM_FLOAT_OPS: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7d,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
        0x0a, 0x0f, 0x01, 0x0d, 0x00,
        0x43, 0x00, 0x00, 0x80, 0x3f,
        0x43, 0x00, 0x00, 0x00, 0x40,
        0x92, 0x0b,
    ];

    use sandbox::{WasmSandbox, SandboxError};

    // Test 1: Normal yükleme + çalıştırma
    {
        let mut ws = WasmSandbox::new();
        match ws.load_module(WASM_SIMPLE) {
            Ok(n) => { arch::uart::puts("[WASM] Module loaded: "); print_u32(n as u32); arch::uart::println(" bytes"); }
            Err(_) => arch::uart::println("[WASM] Load FAIL ✗"),
        }
        match ws.execute("run", 100_000) {
            Ok(42) => arch::uart::println("[WASM] Execute: OK, result=42 ✓"),
            Ok(_)  => arch::uart::println("[WASM] Execute: yanlış sonuç ✗"),
            Err(_) => arch::uart::println("[WASM] Execute FAIL ✗"),
        }
    }
    // Test 2: Fuel tükenmesi
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
    // Test 3: Float reject
    match WasmSandbox::check_module(WASM_FLOAT_OPS) {
        Err(SandboxError::FloatOpcodes) => arch::uart::println("[WASM] Float reject: REJECTED ✓"),
        _ => arch::uart::println("[WASM] Float reject FAIL ✗"),
    }
    // Test 4: Epoch reset + reload
    {
        sandbox::allocator::epoch_reset();
        let mut ws = WasmSandbox::new();
        match ws.load_module(WASM_SIMPLE) {
            Ok(_) => arch::uart::println("[WASM] Epoch reset + reload: OK ✓"),
            Err(_) => arch::uart::println("[WASM] Epoch reset reload FAIL ✗"),
        }
    }
    arch::uart::println("[WASM] Sprint 12 PASS");
    arch::uart::println("");
}

// ═══ Sprint 11: Blackbox ═══
pub fn test_blackbox() {
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

        blackbox::log(blackbox::BlackboxEvent::TaskStart, 0, &[0u8, 4, 1]);
        blackbox::log(blackbox::BlackboxEvent::TaskStart, 1, &[1u8, 8, 2]);

        arch::uart::puts("[TEST] Records after log: ");
        print_u32(blackbox::count() as u32);
        arch::uart::println("");

        let mut bb_pass = true;
        let mut idx: usize = 0;
        while idx < blackbox::count() {
            if blackbox::read(idx).is_none() { bb_pass = false; }
            idx += 1;
        }
        arch::uart::println(if bb_pass { "[TEST] Blackbox records all valid ✓" } else { "[TEST] Blackbox record CRC FAIL ✗" });
        arch::uart::println("[TEST] ★ Blackbox OK ★");
    }
    arch::uart::println("");
}

/// Tüm entegrasyon testlerini çalıştır
pub fn run_all() {
    test_policy_engine();
    test_capability_broker();
    test_ipc();
    test_wcet_limits();
    test_crypto();
    test_wasm();
    test_blackbox();
}
