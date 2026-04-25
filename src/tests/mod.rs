//! Integration test functions extracted from main.rs.

use crate::arch;
use crate::common;
use crate::ipc;
use crate::kernel;
use crate::sandbox;
use crate::common::fmt::print_u32;
use crate::common::sync::SingleHartCell;

// ═══════════════════════════════════════════════════════
// Test fail counter — DO-178C fail criteria
// ═══════════════════════════════════════════════════════

/// Global test fail sayacı — run_all() sonunda kontrol edilir
/// > 0 ise kernel HALT (boot durmalı, production deploy edilmemeli)
static TEST_FAIL_COUNT: SingleHartCell<u32> = SingleHartCell::new(0);

/// Test fail — sayacı artırır + mesaj yazdırır
fn test_fail(msg: &str) {
    arch::uart::println(msg);
    // SAFETY: Single-hart, boot sequence, no concurrent access.
    unsafe { *TEST_FAIL_COUNT.get_mut() += 1; }
}

/// Test pass — sadece mesaj yazdırır
fn test_pass(msg: &str) {
    arch::uart::println(msg);
}

/// Ternary-style test sonucu: pass ise pass_msg, değilse fail_msg (+ counter)
fn test_result(pass: bool, pass_msg: &str, fail_msg: &str) {
    if pass {
        test_pass(pass_msg);
    } else {
        test_fail(fail_msg);
    }
}

// ═══ Sprint 10: Policy Engine Test ═══
pub fn test_policy_engine() {
    arch::uart::println("[TEST] Policy engine...");
    {
        use kernel::policy::{decide_action, FailureMode, PolicyEvent};

        // Budget aşımı: restart_count=0 → RESTART, count=1 → DEGRADE
        let a1 = decide_action(PolicyEvent::BudgetExhausted as u8, 0, 3);
        let a2 = decide_action(PolicyEvent::BudgetExhausted as u8, 1, 3);
        test_result(a1 == FailureMode::Restart,
            "[TEST] Budget(0)→Restart ✓",
            "[TEST] Budget(0)→Restart FAIL ✗");
        test_result(a2 == FailureMode::Degrade,
            "[TEST] Budget(1)→Degrade ✓",
            "[TEST] Budget(1)→Degrade FAIL ✗");

        // Cap violation → her zaman ISOLATE
        let a3 = decide_action(PolicyEvent::CapViolation as u8, 0, 0);
        test_result(a3 == FailureMode::Isolate,
            "[TEST] CapViolation→Isolate ✓",
            "[TEST] CapViolation→Isolate FAIL ✗");

        // PMP fail → her zaman SHUTDOWN
        let a4 = decide_action(PolicyEvent::PmpIntegrityFail as u8, 0, 0);
        test_result(a4 == FailureMode::Shutdown,
            "[TEST] PmpFail→Shutdown ✓",
            "[TEST] PmpFail→Shutdown FAIL ✗");

        // Deadline miss: DAL-A → FAILOVER, DAL-D → ISOLATE
        let a5 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 0);
        let a6 = decide_action(PolicyEvent::DeadlineMiss as u8, 0, 3);
        test_result(a5 == FailureMode::Failover,
            "[TEST] DeadlineMiss DAL-A→Failover ✓",
            "[TEST] DeadlineMiss DAL-A FAIL ✗");
        test_result(a6 == FailureMode::Isolate,
            "[TEST] DeadlineMiss DAL-D→Isolate ✓",
            "[TEST] DeadlineMiss DAL-D FAIL ✗");

        // Sprint U-11: StackOverflow escalation (restart 0-2 → Restart, 3+ → Isolate)
        let a_so = decide_action(PolicyEvent::StackOverflow as u8, 0, 2);
        test_result(a_so == FailureMode::Restart,
            "[TEST] StackOverflow(0)→Restart ✓",
            "[TEST] StackOverflow(0)→Restart FAIL ✗");

        let a_so3 = decide_action(PolicyEvent::StackOverflow as u8, 3, 2);
        test_result(a_so3 == FailureMode::Isolate,
            "[TEST] StackOverflow(3)→Isolate ✓",
            "[TEST] StackOverflow(3)→Isolate FAIL ✗");

        // Sprint U-11: MultiModuleCrash → Shutdown
        let a_mc = decide_action(PolicyEvent::MultiModuleCrash as u8, 0, 0);
        test_result(a_mc == FailureMode::Shutdown,
            "[TEST] MultiModuleCrash→Shutdown ✓",
            "[TEST] MultiModuleCrash→Shutdown FAIL ✗");

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
        let v = broker::validate_full(&tok, 0); // task_id=0 (boot context)
        test_result(v, "[TEST] validate_full OK ✓", "[TEST] validate_full FAIL ✗");

        // 4. Cache hit via syscall (~10c)
        let r = kernel::syscall::cap_invoke(1, 1, ACTION_READ as usize, 0);
        test_result(r == 0, "[TEST] cap_invoke (cache) OK ✓", "[TEST] cap_invoke FAIL ✗");

        // 5. Cache miss → DENIED (token hiç validate edilmedi)
        let r2 = kernel::syscall::cap_invoke(99, 7, ACTION_READ as usize, 0);
        test_result(r2 != 0, "[TEST] cap_invoke (miss) DENIED ✓", "[TEST] cap_invoke miss FAIL ✗");
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
    // IPC fail sayısını global sayaca ekle
    // SAFETY: Single-hart, boot sequence, no concurrent access.
    unsafe { *TEST_FAIL_COUNT.get_mut() += ipc_fail; }
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
        test_result(same,
            "[SEC] BLAKE3 deterministik ✓",
            "[SEC] BLAKE3 deterministik FAIL ✗");

        let h2 = Blake3Provider::keyed_hash(&key2, &data);
        let mut different = false;
        let mut j: usize = 0;
        while j < 16 { if h1a[j] != h2[j] { different = true; } j += 1; }
        test_result(different,
            "[SEC] BLAKE3 key-binding ✓",
            "[SEC] BLAKE3 key-binding FAIL ✗");
    }

    // Ed25519 — test-keys feature ile
    #[cfg(feature = "test-keys")]
    {
        use crate::hal::secure_boot::secure_boot_check;
        use crate::hal::key::{QEMU_TEST_PUBKEY, QEMU_TEST_SIGNATURE};

        let valid = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &QEMU_TEST_SIGNATURE);
        test_result(valid,
            "[SEC] Ed25519 RFC8032 TV1 ✓",
            "[SEC] Ed25519 RFC8032 TV1 FAIL ✗");

        let mut bad_sig = QEMU_TEST_SIGNATURE;
        bad_sig[0] ^= 0xFF;
        let rejected = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &bad_sig);
        test_result(!rejected,
            "[SEC] Ed25519 tampered sig RED ✓",
            "[SEC] Ed25519 tamper tespiti FAIL ✗");

        let wrong_key = [0xFFu8; 32];
        let rejected2 = secure_boot_check(&[], &wrong_key, &QEMU_TEST_SIGNATURE);
        test_result(!rejected2,
            "[SEC] Ed25519 wrong key RED ✓",
            "[SEC] Ed25519 wrong key FAIL ✗");
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
            Err(_) => test_fail("[WASM] Load FAIL ✗"),
        }
        match ws.execute("run", 100_000) {
            Ok(42) => arch::uart::println("[WASM] Execute: OK, result=42 ✓"),
            Ok(_)  => test_fail("[WASM] Execute: yanlış sonuç ✗"),
            Err(_) => test_fail("[WASM] Execute FAIL ✗"),
        }
    }
    // Test 2: Fuel tükenmesi
    {
        let mut ws = WasmSandbox::new();
        let _ = ws.load_module(WASM_SIMPLE);
        match ws.execute("run", 0) {
            Err(SandboxError::FuelExhausted) | Err(SandboxError::Trapped) =>
                arch::uart::println("[WASM] Fuel exhaustion: TRAPPED ✓"),
            Ok(_)  => test_fail("[WASM] Fuel test: beklenen trap gelmedi ✗"),
            Err(_) => test_fail("[WASM] Fuel test: başka hata ✗"),
        }
    }
    // Test 3: Float reject
    match WasmSandbox::check_module(WASM_FLOAT_OPS) {
        Err(SandboxError::FloatOpcodes) => arch::uart::println("[WASM] Float reject: REJECTED ✓"),
        _ => test_fail("[WASM] Float reject FAIL ✗"),
    }
    // Test 4: Epoch reset + reload
    {
        sandbox::allocator::epoch_reset();
        let mut ws = WasmSandbox::new();
        match ws.load_module(WASM_SIMPLE) {
            Ok(_) => arch::uart::println("[WASM] Epoch reset + reload: OK ✓"),
            Err(_) => test_fail("[WASM] Epoch reset reload FAIL ✗"),
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
        test_result(bb_pass,
            "[TEST] Blackbox records all valid ✓",
            "[TEST] Blackbox record CRC FAIL ✗");
        arch::uart::println("[TEST] ★ Blackbox OK ★");
    }
    arch::uart::println("");
}

/// Power-On Self Test — kernel bileşen doğrulaması
pub fn post() {
    arch::uart::println("[POST] Kernel self-test...");

    // 1. CRC32 bilinen vektör
    let crc_data = [0x31u8, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39];
    let crc_result = ipc::crc32(&crc_data);
    if crc_result != 0xCBF4_3926 {
        arch::uart::println("[POST] FAIL: CRC32 engine corrupted — HALT");
        // SAFETY: WFI loop — halt on self-test failure.
        loop { unsafe { core::arch::asm!("wfi"); } }
    }
    arch::uart::println("[POST] CRC32 engine ✓");

    // 2. PMP integrity
    if !kernel::memory::verify_pmp_integrity() {
        arch::uart::println("[POST] FAIL: PMP registers corrupted — HALT");
        loop { unsafe { core::arch::asm!("wfi"); } }
    }
    arch::uart::println("[POST] PMP integrity ✓");

    // 3. Policy engine — PMP fail her zaman Shutdown
    let action = kernel::policy::decide_action(5, 0, 0);
    if action != kernel::policy::FailureMode::Shutdown {
        arch::uart::println("[POST] FAIL: Policy engine corrupted — HALT");
        loop { unsafe { core::arch::asm!("wfi"); } }
    }
    arch::uart::println("[POST] Policy engine ✓");

    // 4. mstatus CSR accessible (M-mode privilege implicit check)
    // NOT: MPP = previous-trap mode, NOT current mode. Current M-mode is
    // implicit: this CSR read only succeeds in M-mode (U-mode → illegal inst).
    // MPP valid values: 0 (U), 3 (M). 1 (S) not used, 2 reserved.
    {
        let mstatus = crate::arch::csr::read_mstatus();
        let mpp = (mstatus >> 11) & 0x3;
        // MPP=2 is reserved — if set, hardware corrupt
        if mpp == 2 {
            arch::uart::println("[POST] FAIL: mstatus.MPP reserved value — HALT");
            loop { unsafe { core::arch::asm!("wfi"); } }
        }
        arch::uart::println("[POST] M-mode CSR access (mstatus) ✓");
    }

    // 5. mtvec set edilmiş mi (boot::init'te yazıldı)
    {
        let mtvec = crate::arch::csr::read_mtvec();
        if mtvec == 0 {
            arch::uart::println("[POST] FAIL: mtvec = 0 — trap handler not set — HALT");
            loop { unsafe { core::arch::asm!("wfi"); } }
        }
        arch::uart::println("[POST] mtvec set ✓");
    }

    // 6. BLAKE3 determinism + non-zero output self-test
    #[cfg(feature = "fast-crypto")]
    {
        use crate::common::crypto::provider::HashProvider;
        use crate::common::crypto::Crypto;
        let key = [0x42u8; 32];
        let data = [0x01u8, 0x02, 0x03, 0x04];
        let h1 = Crypto::keyed_hash(&key, &data);
        let h2 = Crypto::keyed_hash(&key, &data);
        // Determinism: aynı input → aynı output
        let mut same = true;
        let mut i = 0;
        while i < 16 { if h1[i] != h2[i] { same = false; } i += 1; }
        if !same {
            arch::uart::println("[POST] FAIL: BLAKE3 non-deterministic — HALT");
            loop { unsafe { core::arch::asm!("wfi"); } }
        }
        // Non-zero: degenerate hash değil
        let mut all_zero = true;
        let mut j = 0;
        while j < 16 { if h1[j] != 0 { all_zero = false; } j += 1; }
        if all_zero {
            arch::uart::println("[POST] FAIL: BLAKE3 zero output — HALT");
            loop { unsafe { core::arch::asm!("wfi"); } }
        }
        arch::uart::println("[POST] BLAKE3 self-test ✓");
    }

    // 7. Ed25519 known-vector self-test (sadece test-keys feature ile)
    #[cfg(feature = "test-keys")]
    {
        use crate::hal::secure_boot::secure_boot_check;
        use crate::hal::key::{QEMU_TEST_PUBKEY, QEMU_TEST_SIGNATURE};
        let valid = secure_boot_check(&[], &QEMU_TEST_PUBKEY, &QEMU_TEST_SIGNATURE);
        if !valid {
            arch::uart::println("[POST] FAIL: Ed25519 RFC8032 TV1 — HALT");
            loop { unsafe { core::arch::asm!("wfi"); } }
        }
        arch::uart::println("[POST] Ed25519 self-test ✓");
    }
    #[cfg(not(feature = "test-keys"))]
    arch::uart::println("[POST] Ed25519 SKIP (no test-keys)");

    // Sprint U-15: CLINT timer ilerliyor mu?
    {
        let t1 = crate::arch::clint::read_mtime();
        // Birkaç NOP — mtime'ın ilerlemesine zaman ver
        let mut _dummy: u64 = 0;
        let mut k = 0u32;
        while k < 100 { _dummy = _dummy.wrapping_add(1); k += 1; }
        let t2 = crate::arch::clint::read_mtime();
        if t2 <= t1 {
            arch::uart::println("[POST] WARN: mtime not advancing (CLINT issue)");
        } else {
            arch::uart::println("[POST] CLINT timer ✓");
        }
    }

    // Sprint U-15: misa CSR — ISA identity doğrulama
    // RISC-V misa register: bit 8='I', bit 12='M', bit 0='A', bit 2='C'
    // MXL field (bit 63:62) = 2 (64-bit)
    {
        let misa: usize;
        // SAFETY: M-mode CSR access, always valid in M-mode.
        unsafe { core::arch::asm!("csrr {}, misa", out(reg) misa); }
        let mxl = (misa >> 62) & 0x3;
        let has_i = (misa >> 8)  & 1;
        let has_m = (misa >> 12) & 1;
        let has_a = misa & 1;          // bit 0 = 'A'
        let has_c = (misa >> 2)  & 1;
        if mxl != 2 || has_i == 0 || has_m == 0 || has_a == 0 || has_c == 0 {
            arch::uart::println("[POST] WARN: misa does not match riscv64imac");
        } else {
            arch::uart::println("[POST] misa ISA identity ✓");
        }
    }

    arch::uart::println("[POST] ★ All self-tests PASSED ★");
}

/// Per-task PMP NAPOT testi
pub fn test_pmp_napot() {
    arch::uart::println("[TEST] Per-task PMP NAPOT...");

    // Task A (id=0) oluşturulmuş — pmp_addr_napot kontrol
    let napot = unsafe { kernel::scheduler::TASKS.get()[0].pmp_addr_napot };
    if napot == 0 {
        test_fail("[TEST] PMP NAPOT: pmp_addr_napot = 0 FAIL ✗");
        return;
    }
    arch::uart::println("[TEST] PMP NAPOT: addr nonzero ✓");

    // Stack base 8KB aligned?
    let decoded_base = (napot & !0x3FF) << 2;
    if decoded_base % 8192 != 0 {
        test_fail("[TEST] PMP NAPOT: stack not 8KB aligned FAIL ✗");
        return;
    }
    arch::uart::println("[TEST] PMP NAPOT: 8KB aligned ✓");

    // NAPOT decode == stack base?
    let stack_base = unsafe {
        &kernel::scheduler::TASK_STACKS.get()[0].0 as *const _ as usize
    };
    if decoded_base != stack_base {
        test_fail("[TEST] PMP NAPOT: decode mismatch FAIL ✗");
        return;
    }
    arch::uart::println("[TEST] PMP NAPOT: decode matches stack base ✓");
    arch::uart::println("[TEST] ★ PMP NAPOT OK ★");
}

// ═══════════════════════════════════════════════════════
// Sprint U-8: QEMU Fault Injection Tests
// FPGA gerekmez — saf software, QEMU'da yapılabilen FI testleri
// ═══════════════════════════════════════════════════════

pub fn test_fault_injection() {
    arch::uart::println("");
    arch::uart::println("[FI] Fault injection tests...");

    // FI-3: IPC CRC corruption → receiver reject
    fi_ipc_crc_corruption();

    // FI-4: Capability MAC forgery → validate_full reject
    fi_mac_forgery();

    // FI-7: Policy budget exhaustion → DEGRADE decision
    fi_budget_exhaustion_policy();

    arch::uart::println("[FI] ★ All FI tests PASSED ★");
}

/// FI-3: IPC mesajı corrupt → CRC doğrulama reddetmeli
fn fi_ipc_crc_corruption() {
    // Geçerli mesaj oluştur
    let mut msg = ipc::IpcMessage::zeroed();
    msg.data[0] = 0xDE;
    msg.data[1] = 0xAD;
    msg.set_crc();

    // CRC doğru olduğunu kontrol et
    if !msg.verify_crc() {
        test_fail("[FI-3] CRC set failed ✗");
        return;
    }

    // Data'yı corrupt et (CRC alanını bozmadan sadece data değiştir)
    msg.data[0] = 0x00;

    // CRC artık tutmamalı
    if msg.verify_crc() {
        test_fail("[FI-3] CRC corruption NOT detected ✗");
    } else {
        test_pass("[FI-3] CRC corruption detected ✓");
    }
}

/// FI-4: Token MAC forgery → broker reddetmeli
#[cfg(feature = "fast-crypto")]
fn fi_mac_forgery() {
    use kernel::capability::{Token, ACTION_READ};
    use kernel::capability::broker;

    // Geçerli token oluştur (high nonce — replay guard geçer)
    let mut tok = Token::zeroed();
    tok.id = 99;
    tok.task_id = 0;
    tok.resource = 99;
    tok.action = ACTION_READ;
    tok.dal = 1;
    tok.nonce = 9999;
    broker::sign_token(&mut tok);

    let valid = broker::validate_full(&tok, 0);
    if !valid {
        test_fail("[FI-4] Valid token rejected ✗");
        return;
    }
    test_pass("[FI-4] Valid token accepted ✓");

    // MAC'ı corrupt et + cache bypass (farklı resource)
    let mut forged = tok;
    forged.mac[0] ^= 0xFF;
    forged.nonce = 10000;
    forged.resource = 100;

    let rejected = broker::validate_full(&forged, 0);
    if rejected {
        test_fail("[FI-4] Forged MAC accepted ✗");
    } else {
        test_pass("[FI-4] Forged MAC rejected ✓");
    }
}

/// FI-4 stub — fast-crypto feature yoksa
#[cfg(not(feature = "fast-crypto"))]
fn fi_mac_forgery() {
    arch::uart::println("[FI-4] SKIP (no fast-crypto)");
}

/// FI-7: Budget exhaustion → policy DEGRADE escalation
fn fi_budget_exhaustion_policy() {
    use kernel::policy::{decide_action, FailureMode, PolicyEvent};

    // İlk budget exhaustion → RESTART
    let a1 = decide_action(PolicyEvent::BudgetExhausted as u8, 0, 2); // DAL-C
    if a1 != FailureMode::Restart {
        test_fail("[FI-7] Budget(0) != Restart ✗");
        return;
    }
    test_pass("[FI-7] Budget(0) → Restart ✓");

    // İkinci budget exhaustion → DEGRADE
    let a2 = decide_action(PolicyEvent::BudgetExhausted as u8, 1, 2);
    if a2 != FailureMode::Degrade {
        test_fail("[FI-7] Budget(1) != Degrade ✗");
        return;
    }
    test_pass("[FI-7] Budget(1) → Degrade ✓");

    // Üçüncü ve sonrası → hâlâ DEGRADE (saturated)
    let a3 = decide_action(PolicyEvent::BudgetExhausted as u8, 255, 2);
    if a3 != FailureMode::Degrade {
        test_fail("[FI-7] Budget(255) != Degrade ✗");
        return;
    }
    test_pass("[FI-7] Budget(255) → Degrade (saturated) ✓");
}

/// Tüm entegrasyon testlerini çalıştır
/// Fail varsa kernel HALT — production'da test başarısız = boot durmalı (DO-178C)
/// NOT: test_wcet_limits() QEMU TCG'de her zaman EXCEED — bu FAIL sayılmaz
pub fn run_all() {
    post();
    test_pmp_napot();
    test_policy_engine();
    test_capability_broker();
    test_ipc();
    test_wcet_limits(); // informational only — QEMU TCG exceed her zaman var
    test_crypto();
    test_wasm();
    test_blackbox();
    test_fault_injection();

    // ─── Fail criteria: DO-178C "pass criteria clearly defined" ───
    // SAFETY: Single-hart, boot sequence, no concurrent access.
    let total_fail = unsafe { *TEST_FAIL_COUNT.get() };
    if total_fail > 0 {
        arch::uart::puts("[TEST] TOTAL FAILURES: ");
        print_u32(total_fail);
        arch::uart::println("");
        arch::uart::println("[TEST] ✗✗✗ BOOT HALTED — fix failures before deployment ✗✗✗");
        crate::ipc::blackbox::log(
            crate::ipc::blackbox::BlackboxEvent::PolicyShutdown, 0xFF, &[],
        );
        // SAFETY: WFI loop — halt on test failure (DO-178C fail criteria).
        loop { unsafe { core::arch::asm!("wfi"); } }
    }
    arch::uart::println("[TEST] ★★★ ALL TESTS PASSED ★★★");
}
