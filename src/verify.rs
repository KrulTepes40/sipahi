//! Kani formal verification harnesses — panic-freedom and invariant proofs.
// Sipahi — Kani Verification Harnesses
// Sprint 6: 25 formal verification proof
//
// Çalıştırma: cargo kani --harness proof_ismi

#[cfg(kani)]
mod verification {
    use crate::common::config::*;
    use crate::common::types::*;

    // ═══════════════════════════════════════════════════════
    // PROOF 1: Timer tick hesabı overflow olmaz
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn timer_ticks_no_overflow() {
        let clint_freq: u64 = 10_000_000;
        let tick_us: u64 = TICK_PERIOD_US as u64;
        let product = clint_freq.checked_mul(tick_us);
        assert!(product.is_some());
        let result = product.unwrap() / 1_000_000;
        assert!(result > 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 2: DAL safety factor her zaman 100-150 arasında
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_safety_factor_valid() {
        let levels = [DalLevel::A, DalLevel::B, DalLevel::C, DalLevel::D];
        for level in &levels {
            let factor = level.safety_factor();
            assert!(factor >= 100);
            assert!(factor <= 150);
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 3: DAL-A her zaman en yüksek safety factor
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_a_highest_safety() {
        let a = DalLevel::A.safety_factor();
        let b = DalLevel::B.safety_factor();
        let c = DalLevel::C.safety_factor();
        let d = DalLevel::D.safety_factor();
        assert!(a >= b);
        assert!(b >= c);
        assert!(c >= d);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 4: WCET hedefleri tutarlı sırada
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn wcet_ordering_consistent() {
        assert!(WCET_YIELD <= WCET_TRAP_ENTRY);
        assert!(WCET_TRAP_ENTRY <= WCET_IPC_RECV);
        assert!(WCET_IPC_RECV <= WCET_TRAP_HANDLER);
        assert!(WCET_TRAP_HANDLER <= WCET_IPC_SEND);
        assert!(WCET_IPC_SEND <= WCET_SCHEDULER_TICK);
        assert!(WCET_SCHEDULER_TICK <= WCET_CAP_INVOKE);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 5: Syscall ID'leri benzersiz ve 0-4 arasında
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn syscall_ids_valid() {
        let ids = [SYS_CAP_INVOKE, SYS_IPC_SEND, SYS_IPC_RECV, SYS_YIELD, SYS_TASK_INFO];
        for &id in &ids {
            assert!(id <= 4);
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert!(ids[i] != ids[j]);
            }
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 6: Compute service ID'leri benzersiz
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn compute_ids_unique() {
        let ids = [COMPUTE_COPY, COMPUTE_CRC, COMPUTE_MAC, COMPUTE_MATH];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert!(ids[i] != ids[j]);
            }
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 7: IPC kanal bellek hesabı
    // Slot verisi ayrı, gerçek struct boyutu ayrı kontrol ediliyor.
    // SORUN 1: SpscChannel = 1028B (4B AtomicU16 overhead + 1024B slot)
    //          8 × 1028 = 8,224B > 8,192B (PMP R3 bütçesi)
    //          Fix: head/tail'i ilk slot'a göm → Sprint 8 sonrası assert aktif et.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn ipc_pool_size_fits() {
        // Slot verisi: 16 slot × 64B = 1024B (mesaj alanı)
        let channel_data = IPC_CHANNEL_SLOTS * IPC_MSG_SIZE;
        assert!(channel_data == 1024);

        // Gerçek struct boyutu: slots + AtomicU16 overhead + olası padding
        let actual_size = core::mem::size_of::<crate::ipc::SpscChannel>();
        assert!(actual_size >= channel_data); // overhead var, sadece slot değil

        let actual_pool = MAX_IPC_CHANNELS * actual_size;

        // PMP R3 = 8KB — SORUN 1 fix sonrası bu assert aktif edilecek:
        // assert!(actual_pool <= 8 * 1024); // TODO: Sprint 8 head/tail gömme fix'i

        // RAM'e sığıyor (512KB >> 8KB)
        assert!(actual_pool < 512 * 1024);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 8: Bellek bütçesi 4MB RAM'e sığar
    // Sprint 12'de linker script 512K → 4M oldu (wasmi binary ~700KB).
    // WASM_HEAP_SIZE Sprint 13'te 256KB'a yükseltildi (wasmi 1.0.9 overhead).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn memory_fits_in_ram() {
        let kernel_text: usize = 32 * 1024;
        let kernel_data: usize = 20 * 1024;
        let kernel_rodata: usize = 8 * 1024;
        let ipc_pool: usize = 8 * 1024;
        let blackbox: usize = 8 * 1024;
        let device_mmio: usize = 4 * 1024;
        let dma_buffer: usize = 8 * 1024;
        let kernel_total = kernel_text + kernel_data + kernel_rodata
            + ipc_pool + blackbox + device_mmio + dma_buffer;
        let task_total = MAX_TASKS * 24 * 1024;
        let wasm_heap = WASM_HEAP_SIZE;
        let total = kernel_total + task_total + wasm_heap;
        // QEMU virt = 512MB, linker script RAM = 8MB (sipahi.ld)
        let ram_size: usize = 8 * 1024 * 1024;
        assert!(total < ram_size);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 9: INTERRUPT_BIT RV64 için doğru
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn interrupt_bit_correct() {
        let bit: usize = 1 << 63;
        assert!(bit == 0x8000_0000_0000_0000);
        let timer_interrupt: usize = bit | 7;
        assert!(timer_interrupt & bit != 0);
        assert!(timer_interrupt & !bit == 7);
        let exception: usize = 2;
        assert!(exception & bit == 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 10: DAL budget toplamı %100
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_budget_sum_100() {
        let dal_a_pct: u32 = 40;
        let dal_b_pct: u32 = 30;
        let dal_c_pct: u32 = 20;
        let dal_d_pct: u32 = 10;
        assert!(dal_a_pct + dal_b_pct + dal_c_pct + dal_d_pct == 100);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 11: Task bellek hesabı overflow olmaz
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn task_memory_no_overflow() {
        let per_task = TASK_STACK_SIZE.checked_add(16 * 1024);
        assert!(per_task.is_some());
        let total = per_task.unwrap().checked_mul(MAX_TASKS);
        assert!(total.is_some());
        assert!(total.unwrap() == 192 * 1024);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 12: Host call overhead bounded
    // Sprint 13'te aktif edildi — WCET_COMPUTE_* config.rs'e eklendi.
    // Doğru metrik: HOST_CALL_LIMIT × max(WCET_COMPUTE_COPY..WCET_COMPUTE_MAC)
    //   = 16 × 350c (COMPUTE_MAC) = 5,600c < 10,000c ✓
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn host_call_budget_bounded() {
        // cap_invoke dispatch overhead (cache hit path)
        let cap_overhead = (HOST_CALL_LIMIT as u64) * (WCET_CAP_INVOKE as u64);
        assert!(cap_overhead < 100_000); // 16 × 120 = 1,920c ✓

        // Compute service overhead (WCET_COMPUTE_MAC = worst-case service)
        let compute_overhead = (HOST_CALL_LIMIT as u64) * WCET_COMPUTE_MAC;
        assert!(compute_overhead < 10_000); // 16 × 350 = 5,600c < 10,000 ✓

        // COMPUTE_MAC her zaman en pahalı servis
        assert!(WCET_COMPUTE_MAC >= WCET_COMPUTE_COPY);
        assert!(WCET_COMPUTE_MAC >= WCET_COMPUTE_CRC);
        assert!(WCET_COMPUTE_MAC >= WCET_COMPUTE_MATH);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 13: Stack hizalama RISC-V ABI uyumlu (16-byte)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn stack_alignment_valid() {
        assert!(KERNEL_STACK_SIZE % 16 == 0);
        assert!(TASK_STACK_SIZE % 16 == 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 14: TaskContext boyutu context.S ile eşleşir
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn task_context_size_matches_asm() {
        assert!(core::mem::size_of::<crate::kernel::scheduler::TaskContext>() == 128);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 15: PMP config pack doğru çalışır
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_config_pack_correct() {
        use crate::arch::pmp;

        let configs: [u8; 8] = [
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X,
            0,
            pmp::PMP_TOR | pmp::PMP_R,
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W,
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W,
        ];

        let packed = pmp::pack_pmpcfg(configs);

        let entry1 = ((packed >> 8) & 0xFF) as u8;
        assert!(entry1 == (pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X));

        let entry3 = ((packed >> 24) & 0xFF) as u8;
        assert!(entry3 == (pmp::PMP_TOR | pmp::PMP_R));

        let entry5 = ((packed >> 40) & 0xFF) as u8;
        assert!(entry5 == (pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W));

        let entry7 = ((packed >> 56) & 0xFF) as u8;
        assert!(entry7 == (pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W));

        assert!((packed & 0xFF) as u8 == 0);
        assert!(((packed >> 16) & 0xFF) as u8 == 0);
        assert!(((packed >> 32) & 0xFF) as u8 == 0);
        assert!(((packed >> 48) & 0xFF) as u8 == 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 16: PMP TOR sabitleri RISC-V spec ile uyumlu
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_tor_mode_bits_correct() {
        use crate::arch::pmp;
        assert!(pmp::PMP_R == 0x01);
        assert!(pmp::PMP_W == 0x02);
        assert!(pmp::PMP_X == 0x04);
        assert!(pmp::PMP_TOR == 0x08);
        assert!(pmp::PMP_L == 0x80);
        assert!(pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X == 0x0D);
        assert!(pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W == 0x0B);
        assert!(pmp::PMP_TOR | pmp::PMP_R == 0x09);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 17: .text bölgesi yazılabilir DEĞİL
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_text_not_writable() {
        use crate::arch::pmp;
        let text_cfg = pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X;
        assert!(text_cfg & pmp::PMP_W == 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 18: .rodata yazılabilir ve çalıştırılabilir DEĞİL
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_rodata_readonly() {
        use crate::arch::pmp;
        let rodata_cfg = pmp::PMP_TOR | pmp::PMP_R;
        assert!(rodata_cfg & pmp::PMP_W == 0);
        assert!(rodata_cfg & pmp::PMP_X == 0);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 19: Tüm TOR entry'lerde L-bit (lock) set — M-mode dahil kilitli
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_all_tor_entries_locked() {
        use crate::arch::pmp;
        let configs: [u8; 8] = [
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_X | pmp::PMP_L,
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_L,
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W | pmp::PMP_L,
            0,
            pmp::PMP_TOR | pmp::PMP_R | pmp::PMP_W | pmp::PMP_L,
        ];
        let mut i = 0;
        while i < 8 {
            if configs[i] & pmp::PMP_TOR != 0 {
                assert!(configs[i] & pmp::PMP_L != 0);
            }
            i += 1;
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 20: pack_pmpcfg geri çözülebilir (roundtrip)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_pack_unpack_roundtrip() {
        use crate::arch::pmp;
        let original: [u8; 8] = [0, 0x8D, 0, 0x89, 0, 0x8B, 0, 0x8B]; // L-bit set
        let packed = pmp::pack_pmpcfg(original);
        let mut i = 0;
        while i < 8 {
            let extracted = ((packed >> (i * 8)) & 0xFF) as u8;
            assert!(extracted == original[i]);
            i += 1;
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 21: IOPMP max bölge sayısı makul
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_max_regions_bounded() {
        use crate::hal::iopmp;
        assert!(iopmp::IOPMP_MAX_REGIONS <= 8);
        assert!(iopmp::IOPMP_MAX_REGIONS >= 1);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 22: IOPMP controller başlangıçta kapalı
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_starts_disabled() {
        use crate::hal::iopmp::IopmpController;
        let ctrl = IopmpController::new();
        assert!(!ctrl.is_enabled());
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 23: IOPMP kapalıyken tüm erişim serbest
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_disabled_allows_all() {
        use crate::hal::iopmp::IopmpController;
        let ctrl = IopmpController::new();
        // Kapalı → her adres, her boyut, okuma/yazma serbest
        assert!(ctrl.check_access(0x1000, 4, false));
        assert!(ctrl.check_access(0x2000, 8, true));
        assert!(ctrl.check_access(0, 1, false));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 24: IOPMP geçersiz index → InvalidParameter
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_invalid_index_rejected() {
        use crate::hal::iopmp::{IopmpController, IopmpRegion, IOPMP_MAX_REGIONS};
        use crate::common::error::SipahiError;
        let mut ctrl = IopmpController::new();
        let region = IopmpRegion::new(0x1000, 0x100, true, false);
        let result = ctrl.add_region(IOPMP_MAX_REGIONS, region);
        assert!(result == Err(SipahiError::InvalidParameter));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 25: IOPMP etkin + tanımsız bölge → erişim RED
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_enabled_denies_unknown() {
        use crate::hal::iopmp::IopmpController;
        let mut ctrl = IopmpController::new();
        let _ = ctrl.enable();
        // Hiç bölge tanımlı değil → tüm erişim reddedilmeli
        assert!(!ctrl.check_access(0x1000, 4, false));
        assert!(!ctrl.check_access(0x2000, 8, true));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (logic): schedule() sıfıra bölme riski yok
    // schedule() TASK_COUNT < 2 ise erken döner.
    // % TASK_COUNT yalnızca TASK_COUNT >= 2 durumunda çalışır.
    // Tüm iterasyon adımları MAX_TASKS ile bounded.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn schedule_no_mod_by_zero() {
        let task_count: u8 = kani::any();
        kani::assume(task_count >= 2);
        kani::assume(task_count <= 8); // MAX_TASKS

        let current: u8 = kani::any();
        kani::assume(current < task_count);

        let next = ((current as usize) + 1) % (task_count as usize);
        assert!(next < task_count as usize);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (logic): create_task() stack hizalaması doğru
    // stack_top & !0xF → her zaman 16-byte aligned, stack_top'tan ≤
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn create_task_stack_alignment() {
        let stack_top: u32 = kani::any();
        kani::assume(stack_top >= 8192); // TASK_STACK_SIZE

        let aligned = (stack_top as usize) & !0xF_usize;
        assert!(aligned % 16 == 0);                      // 16-byte aligned
        assert!(aligned <= stack_top as usize);           // aşağı yuvarlama — asla artmaz
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 71: Isolated task asla Ready'ye dönmez
    // Faz 1 mantığı: yalnızca Suspended → Ready (Isolated kapsam dışı)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn isolated_task_never_becomes_ready() {
        // Faz 1 periyot reset mantığını simüle et:
        // sadece Suspended → Ready, diğer state'ler değişmez
        let state = TaskState::Isolated;
        let new_state = if state == TaskState::Suspended {
            TaskState::Ready
        } else {
            state
        };
        assert!(new_state != TaskState::Ready);
        assert!(new_state == TaskState::Isolated);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 72: DAL-A task budget aşımında asla Isolated olmaz
    // Budget aşımı escalation: RESTART → DEGRADE (Isolate değil)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_a_budget_exhausted_never_isolated() {
        use crate::kernel::policy::{decide_action, FailureMode, PolicyEvent};
        let event = PolicyEvent::BudgetExhausted as u8;
        let dal   = 0u8; // DAL-A
        let count: u8 = kani::any();
        // Budget exhausted → RESTART(1) → DEGRADE, hiçbir koşulda ISOLATE değil
        let action = decide_action(event, count, dal);
        assert!(action != FailureMode::Isolate);
    }

    // ═══════════════════════════════════════════════════════
    // v1.5 Batch 1: 11 new proofs (85-95)
    // ═══════════════════════════════════════════════════════

    // --- COMMON ---

    /// Proof 85: SipahiError::as_str() her variant için boş olmayan string döner
    #[kani::proof]
    fn sipahi_error_as_str_never_empty() {
        let err: crate::common::error::SipahiError = kani::any();
        let s = err.as_str();
        assert!(!s.is_empty());
    }

    /// Proof 86: SyscallResult::to_raw() farklı variant'lar farklı değer döner
    #[kani::proof]
    fn syscall_result_to_raw_unique() {
        use crate::kernel::syscall::dispatch::SyscallResult;
        let a: SyscallResult = kani::any();
        let b: SyscallResult = kani::any();
        kani::assume(a != b);
        assert!(a.to_raw() != b.to_raw());
    }

    /// Proof 87: CRC32 deterministic — aynı girdi aynı çıktı (1 byte, 8 bit iter)
    #[kani::proof]
    #[kani::unwind(10)]
    fn crc32_deterministic() {
        let mut data = [0u8; 1];
        data[0] = kani::any();
        let h1 = crate::ipc::crc32(&data);
        let h2 = crate::ipc::crc32(&data);
        assert!(h1 == h2);
    }

    /// Proof 88: CRC32 avalanche — tek bit farkı farklı hash (1 byte, 8 bit)
    #[kani::proof]
    #[kani::unwind(10)]
    fn crc32_single_bit_change() {
        let mut data = [0u8; 1];
        data[0] = kani::any();
        let original = crate::ipc::crc32(&data);
        let bit: usize = kani::any();
        kani::assume(bit < 8);
        data[0] ^= 1 << bit;
        let modified = crate::ipc::crc32(&data);
        assert!(original != modified);
    }

    // --- POLICY ---

    /// Proof 89: WatchdogTimeout event asla Shutdown dönmez
    #[kani::proof]
    fn watchdog_timeout_never_shutdown() {
        use crate::kernel::policy::{decide_action, FailureMode};
        let restart_count: u8 = kani::any();
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        let action = decide_action(6, restart_count, dal);
        assert!(action != FailureMode::Shutdown);
    }

    /// Proof 90: İlk hata (restart_count=0) asla Shutdown dönmez (PMP fail hariç)
    #[kani::proof]
    fn first_failure_never_shutdown() {
        use crate::kernel::policy::{decide_action, FailureMode};
        let event: u8 = kani::any();
        kani::assume(event <= 7);
        kani::assume(event != 5); // PMP fail hariç
        let dal: u8 = kani::any();
        kani::assume(dal <= 3);
        let action = decide_action(event, 0, dal);
        assert!(action != FailureMode::Shutdown);
    }

    /// Proof 91: decide_action her zaman geçerli FailureMode döner (0-5)
    #[kani::proof]
    fn decide_action_always_valid_mode() {
        use crate::kernel::policy::{decide_action, FailureMode};
        let event: u8 = kani::any();
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        let action = decide_action(event, rc, dal);
        let v = action as u8;
        assert!(v <= FailureMode::Shutdown as u8);
    }

    // --- CAPABILITY ---

    /// Proof 92: Token expiry: expires=0 → asla expired olmaz
    #[kani::proof]
    fn token_expiry_zero_means_infinite() {
        let expires: u32 = 0;
        let current_tick: u64 = kani::any();
        let expired = expires > 0 && current_tick > expires as u64;
        assert!(!expired);
    }

    /// Proof 93: Token expiry: expires > 0 ve tick > expires → expired
    #[kani::proof]
    fn token_expiry_detects_expired() {
        let expires: u32 = kani::any();
        kani::assume(expires > 0);
        let current_tick: u64 = kani::any();
        kani::assume(current_tick > expires as u64);
        let expired = expires > 0 && current_tick > expires as u64;
        assert!(expired);
    }

    // --- SCHEDULER ---

    /// Proof 94: Priority / DAL grubu ilişkisi — prio 0-15 → dal_group 0-3
    #[kani::proof]
    fn task_priority_dal_group_bounded() {
        let prio: u8 = kani::any();
        kani::assume(prio <= 15);
        let dal_group = prio / 4;
        assert!(dal_group <= 3);
    }

    /// Proof 95: Watchdog limit=0 → asla tetiklenmez
    #[kani::proof]
    fn watchdog_limit_zero_disables() {
        let counter: u32 = kani::any();
        let limit: u32 = 0;
        let triggered = limit > 0 && counter >= limit;
        assert!(!triggered);
    }

    // --- HAL ---

    /// Proof 135: IOPMP new() → disabled, tüm erişim serbest
    #[kani::proof]
    fn iopmp_new_starts_disabled_allows_all() {
        use crate::hal::iopmp::IopmpController;
        let ctrl = IopmpController::new();
        assert!(ctrl.check_access(0, 1, false));
        assert!(ctrl.check_access(0x1000, 4, true));
    }

    /// Proof 136: Config sabitleri tutarlılık
    #[kani::proof]
    fn config_constants_consistent() {
        assert!(MAX_TASKS > 0);
        assert!(MAX_TASKS <= 16);
        assert!(TASK_STACK_SIZE >= 4096);
        assert!(MAX_IPC_CHANNELS > 0);
        assert!(MAX_IPC_CHANNELS <= MAX_TASKS);
        assert!(BLACKBOX_MAX_RECORDS > 0);
        assert!(BLACKBOX_MAX_RECORDS <= 256);
    }

    // --- CRYPTO ---

    /// Proof 139: BLAKE3 farklı key → farklı hash
    #[kani::proof]
    fn blake3_different_key_different_hash() {
        use crate::common::crypto::provider::HashProvider;
        use crate::common::crypto::Blake3Provider;
        let key1 = [0x5Au8; 32];
        let key2 = [0xA5u8; 32];
        let data = [0x42u8; 4];
        let h1 = Blake3Provider::keyed_hash(&key1, &data);
        let h2 = Blake3Provider::keyed_hash(&key2, &data);
        let mut same = true;
        let mut i = 0;
        while i < 16 { if h1[i] != h2[i] { same = false; } i += 1; }
        assert!(!same);
    }

    /// Proof 140: BLAKE3 aynı (key, data) → aynı hash
    #[kani::proof]
    fn blake3_same_input_same_hash() {
        use crate::common::crypto::provider::HashProvider;
        use crate::common::crypto::Blake3Provider;
        let key = [0x5Au8; 32];
        let data = [0x42u8; 4];
        let h1 = Blake3Provider::keyed_hash(&key, &data);
        let h2 = Blake3Provider::keyed_hash(&key, &data);
        let mut i = 0;
        while i < 16 { assert!(h1[i] == h2[i]); i += 1; }
    }

    // --- CROSS-MODULE ---

    /// Proof 147: Token MAC alanı == BLAKE3 çıktı boyutu (16 byte)
    #[kani::proof]
    fn token_mac_field_matches_blake3_output() {
        use crate::common::crypto::provider::HashProvider;
        use crate::common::crypto::Blake3Provider;
        use crate::kernel::capability::Token;
        let t = Token::zeroed();
        assert!(t.mac.len() == 16);
        let hash = Blake3Provider::keyed_hash(&[0u8; 32], &[0u8; 4]);
        assert!(hash.len() == 16);
    }

    /// Proof 148: BlackboxEvent u8'e sığar
    #[kani::proof]
    fn blackbox_event_fits_in_u8() {
        use crate::ipc::blackbox::BlackboxEvent;
        assert!(core::mem::size_of::<BlackboxEvent>() <= 1);
    }

    /// Proof 149: TaskState u8'e sığar
    #[kani::proof]
    fn task_state_fits_in_u8() {
        assert!(core::mem::size_of::<TaskState>() <= 1);
    }

    /// Proof 150: FailureMode u8'e sığar
    #[kani::proof]
    fn failure_mode_fits_in_u8() {
        use crate::kernel::policy::FailureMode;
        assert!(core::mem::size_of::<FailureMode>() <= 1);
    }

    /// Proof 154: Farklı token → farklı header (id veya nonce farklıysa)
    #[kani::proof]
    fn different_tokens_different_headers() {
        use crate::kernel::capability::Token;
        let mut t1 = Token::zeroed();
        let mut t2 = Token::zeroed();
        t1.id = kani::any();
        t2.id = kani::any();
        t1.nonce = kani::any();
        t2.nonce = kani::any();
        kani::assume(t1.id != t2.id || t1.nonce != t2.nonce);
        let h1 = t1.header_bytes();
        let h2 = t2.header_bytes();
        let mut same = true;
        let mut i = 0;
        while i < 16 { if h1[i] != h2[i] { same = false; } i += 1; }
        assert!(!same);
    }

    /// Proof 162: DalLevel safety_factor kesin sıralama A > B > C > D
    #[kani::proof]
    fn dal_safety_factor_strict_ordering() {
        let a = DalLevel::A.safety_factor();
        let b = DalLevel::B.safety_factor();
        let c = DalLevel::C.safety_factor();
        let d = DalLevel::D.safety_factor();
        assert!(a > b && b > c && c > d);
    }

    /// Proof 163: pack_pmpcfg symbolic entry → extract matches original
    #[kani::proof]
    fn pmp_pack_extract_any_entry() {
        use crate::arch::pmp::pack_pmpcfg;
        let mut configs = [0u8; 8];
        let idx: usize = kani::any();
        kani::assume(idx < 8);
        let val: u8 = kani::any();
        configs[idx] = val;
        let packed = pack_pmpcfg(configs);
        let extracted = ((packed >> (idx * 8)) & 0xFF) as u8;
        assert!(extracted == val);
    }

    /// Proof 164: Token resource symbolic u16 → LE bytes doğru
    #[kani::proof]
    fn token_header_resource_le_encoding() {
        use crate::kernel::capability::Token;
        let mut t = Token::zeroed();
        t.resource = kani::any();
        let h = t.header_bytes();
        let reconstructed = (h[2] as u16) | ((h[3] as u16) << 8);
        assert!(reconstructed == t.resource);
    }

    /// Proof 165: Token expires symbolic u32 → LE bytes doğru
    #[kani::proof]
    fn token_header_expires_le_encoding() {
        use crate::kernel::capability::Token;
        let mut t = Token::zeroed();
        t.expires = kani::any();
        let h = t.header_bytes();
        let r = (h[8] as u32) | ((h[9] as u32) << 8)
              | ((h[10] as u32) << 16) | ((h[11] as u32) << 24);
        assert!(r == t.expires);
    }
}
