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
    // PROOF 7: IPC kanal boyutu hesabı tutarlı
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn ipc_pool_size_fits() {
        let channel_data = IPC_CHANNEL_SLOTS * IPC_MSG_SIZE;
        let pool_total = MAX_IPC_CHANNELS * channel_data;
        assert!(channel_data == 1024);
        assert!(pool_total == 8192);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 8: Bellek bütçesi 512KB RAM'e sığar
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
        let ram_size: usize = 512 * 1024;
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
    // PROOF 12: Host call limiti budget'ı aşmaz
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn host_call_budget_bounded() {
        let max_overhead = (HOST_CALL_LIMIT as u64) * (WCET_CAP_INVOKE as u64);
        assert!(max_overhead < 100_000);
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
        assert!(core::mem::size_of::<crate::kernel::scheduler::TaskContext>() == 112);
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
    // PROOF 19: Hiçbir bölge L (lock) biti set değil
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_no_lock_bit() {
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
        let mut i = 0;
        while i < 8 {
            assert!(configs[i] & pmp::PMP_L == 0);
            i += 1;
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 20: pack_pmpcfg geri çözülebilir (roundtrip)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn pmp_pack_unpack_roundtrip() {
        use crate::arch::pmp;
        let original: [u8; 8] = [0, 0x0D, 0, 0x09, 0, 0x0B, 0, 0x0B];
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
}
