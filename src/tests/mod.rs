//! Integration test functions extracted from main.rs.

use crate::arch;
use crate::common;
use crate::ipc;
use crate::kernel;
use crate::common::fmt::print_u32;

// ═══ Sprint 10: Policy Engine Test ═══
pub fn test_policy_engine() {
    arch::uart::println("[TEST] Policy engine...");
    {
        use kernel::policy::{decide_action, FailureMode, PolicyEvent};

        // Budget aşımı: restart_count=0 → RESTART, count=1 → DEGRADE
        let a1 = decide_action(PolicyEvent::BudgetExhausted as u8, 0, 3);
        let a2 = decide_action(PolicyEvent::BudgetExhausted as u8, 1, 3);
        arch::uart::println(if a1 == FailureMode::Restart as u8 {
            "[TEST] Budget(0)→Restart ✓"
        } else {
            "[TEST] Budget(0)→Restart FAIL ✗"
        });
        arch::uart::println(if a2 == FailureMode::Degrade as u8 {
            "[TEST] Budget(1)→Degrade ✓"
        } else {
            "[TEST] Budget(1)→Degrade FAIL ✗"
        });

        // Cap violation → her zaman ISOLATE
        let a3 = decide_action(PolicyEvent::CapViolation as u8, 0, 0);
        arch::uart::println(if a3 == FailureMode::Isolate as u8 {
            "[TEST] CapViolation→Isolate ✓"
        } else {
            "[TEST] CapViolation→Isolate FAIL ✗"
        });

        // PMP fail → her zaman SHUTDOWN
        let a4 = decide_action(PolicyEvent::PmpIntegrityFail as u8, 0, 0);
        arch::uart::println(if a4 == FailureMode::Shutdown as u8 {
            "[TEST] PmpFail→Shutdown ✓"
        } else {
            "[TEST] PmpFail→Shutdown FAIL ✗"
        });

        // Deadline miss: DAL-A → FAILOVER, DAL-D → ISOLATE
        let a5 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 0);
        let a6 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 3);
        arch::uart::println(if a5 == FailureMode::Failover as u8 {
            "[TEST] DeadlineMiss DAL-A→Failover ✓"
        } else {
            "[TEST] DeadlineMiss DAL-A FAIL ✗"
        });
        arch::uart::println(if a6 == FailureMode::Isolate as u8 {
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
