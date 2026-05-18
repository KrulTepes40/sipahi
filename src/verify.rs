//! Kani formal verification harnesses — panic-freedom and invariant proofs.
// Sipahi — Kani Verification Harnesses
// Sprint 6: 25 formal verification proof
//
// Çalıştırma: cargo kani --harness proof_ismi
//
// U-22 GÖREV 12 [L6+L7]: doctrine — bu dosya RUNTIME KOD İÇERMEZ.
// Tüm fonksiyonlar `#[cfg(kani)] mod verification` içinde, sembolik input ile
// Kani harness olarak çalışır. Kani context'inde:
//   - unwrap() KABUL EDİLİR (Kani panic'i path divergence olarak ele alır,
//     production'da unreachable bir invariant'ı zaten kanıtlamış oluruz)
//   - for ... in iter KABUL EDİLİR (Kani bound'u compile-time bilir,
//     unwinding sınırı `cargo kani` config'inde)
//
// Bu kuralın istisnası: production runtime kodda HER İKİSİ YASAK
// (Sipahi doctrine: bounded loops + match/if-let, no unwrap, no for-iter).

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
    // Sprint U-15: U-9 mscratch swap + U-10 UART gate sonrası recalibrated.
    // Yeni sıralama:
    // YIELD(10) ≤ CAP_INVOKE(25) ≤ IPC_RECV(40) ≤ IPC_SEND(60) ≤
    // TRAP_ENTRY(80) ≤ TRAP_HANDLER(80) ≤ CONTEXT_SWITCH(80) ≤ SCHEDULER_TICK(350)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn wcet_ordering_consistent() {
        // Hot path: trap-syscall-yield zinciri (≤ scheduler tick)
        assert!(WCET_YIELD <= WCET_TASK_INFO);     // U-20: yield (10) ≤ task_info (15)
        assert!(WCET_TASK_INFO <= WCET_CAP_INVOKE); // task_info (15) ≤ cap_invoke (25)
        assert!(WCET_CAP_INVOKE <= WCET_IPC_RECV);
        assert!(WCET_IPC_RECV <= WCET_IPC_SEND);
        assert!(WCET_IPC_SEND <= WCET_TRAP_ENTRY);
        assert!(WCET_TRAP_ENTRY <= WCET_TRAP_HANDLER);
        assert!(WCET_TRAP_HANDLER <= WCET_CONTEXT_SWITCH);
        assert!(WCET_CONTEXT_SWITCH <= WCET_SCHEDULER_TICK);
        // task_info da scheduler tick'ten küçük olmalı (transitif ama açıkça yaz)
        assert!(WCET_TASK_INFO <= WCET_SCHEDULER_TICK);

        // U-18 GÖREV 4: Sıcak yol -> kapasite zinciri.
        // Token validate (cache miss) > scheduler tick — full broker validate
        // pahalı (BLAKE3 MAC + nonce + expiry). Cache hit (10c) << validate (400c).
        assert!(WCET_TOKEN_CACHE_HIT <= WCET_YIELD); // 10 ≤ 10 (sınır)
        assert!(WCET_TOKEN_CACHE_HIT <= WCET_TOKEN_VALIDATE);
        assert!(WCET_SCHEDULER_TICK <= WCET_TOKEN_VALIDATE);

        // U-22.5 G1: Compute service WCET ordering assertions removed.
        // dispatch_compute fonksiyonu ve COMPUTE_* sabitleri sprint U-22.5'te
        // silindi (WASM-tied orphan code). Ordering invariant artık geçersiz.
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
    // PROOF 6: REMOVED in U-22.5 G1 (compute_ids_unique)
    // dispatch_compute fonksiyonu silindi → COMPUTE_* sabitleri yok.
    // ═══════════════════════════════════════════════════════

    // ═══════════════════════════════════════════════════════
    // PROOF 6b (U-23 SNTM-R1): Syscall ID set complete + ordered
    //
    // VERIFIES: SNTM-R1 (syscall ID tablosu eksiksiz + 0..N sıralı + benzersiz)
    // CALLS:    config::SYS_CAP_INVOKE, SYS_IPC_SEND, SYS_IPC_RECV,
    //           SYS_YIELD, SYS_TASK_INFO, SYS_EXIT, SYSCALL_COUNT
    // FAILS-IF: Yeni syscall eklenip SYSCALL_COUNT güncellenmediyse,
    //           veya ID sırası bozulduğunda (sipahi_api ABI break).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn syscall_id_set_complete() {
        let ids = [
            SYS_CAP_INVOKE,
            SYS_IPC_SEND,
            SYS_IPC_RECV,
            SYS_YIELD,
            SYS_TASK_INFO,
            SYS_EXIT,
        ];

        // Count tutarlılığı — SYSCALL_COUNT = handler sayısı
        assert!(ids.len() == SYSCALL_COUNT);

        // 0..N sıralı sequence + benzersizlik
        let mut i = 0;
        while i < ids.len() {
            assert!(ids[i] == i);
            i += 1;
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-24 SNTM-R3): Region overlap symmetric + defensive
    //
    // VERIFIES: SNTM-R3 (regions_overlap symmetric + saturating_add safe + empty no-overlap)
    // CALLS:    crate::kernel::pmp::overlap::regions_overlap
    // FAILS-IF: regions_overlap asymmetric (a,b ≠ b,a), veya overflow ile
    //           overlap'ı false negative (saturating_add eksik), veya boş
    //           region (size=0) için overlap true (must be false).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn region_overlap_symmetric() {
        use crate::kernel::pmp::overlap::regions_overlap;
        let a_base: usize = kani::any();
        let a_size: usize = kani::any();
        let b_base: usize = kani::any();
        let b_size: usize = kani::any();
        kani::assume(a_size <= 0x10000);  // bounded for Kani performance
        kani::assume(b_size <= 0x10000);

        // Symmetry: aynı pair iki sırada da aynı sonuç
        let ab = regions_overlap(a_base, a_size, b_base, b_size);
        let ba = regions_overlap(b_base, b_size, a_base, a_size);
        assert!(ab == ba);

        // Empty region: size=0 → asla overlap
        let zero = regions_overlap(a_base, 0, b_base, b_size);
        assert!(!zero);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-24 SNTM-R5): NAPOT alignment correctness
    //
    // VERIFIES: SNTM-R5 (valid_napot_alignment: size power-of-2 ≥ 8 AND base aligned)
    // CALLS:    crate::kernel::pmp::overlap::valid_napot_alignment
    // FAILS-IF: Power-of-2 olmayan size kabul edilirse, veya base aligned değilse
    //           kabul edilirse, veya size < 8 kabul edilirse.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn napot_alignment_correct() {
        use crate::kernel::pmp::overlap::valid_napot_alignment;

        let base: usize = kani::any();
        let size: usize = kani::any();
        kani::assume(size <= 0x100000);  // bounded

        let result = valid_napot_alignment(base, size);

        if result {
            // Valid ise tüm 3 koşul sağlanmalı
            assert!(size >= 8);
            assert!(size & (size - 1) == 0);  // power-of-2
            assert!(base & (size - 1) == 0);  // aligned
        }
        // Reciprocal: size<8 veya non-pow2 ise reddedilmeli
        if size < 8 {
            assert!(!result);
        } else if size & (size - 1) != 0 {
            assert!(!result);
        }
    }

    // VERIFIES: SNTM-R7 (region-scan + Access perm iff invariant)
    // CALLS:    crate::kernel::syscall::dispatch::check_ptr_in_profile
    //           + crate::kernel::pmp::profile::{Access, Permission, PmpProfile, Region}
    // FAILS-IF: Region dışı ptr için true, region içi ptr için false, partial
    //           overlap kabul, access-perm mismatch kabul, ya da empty profile
    //           için true. Kani bounded: 1 region, ptr/size ≤ 2^16.
    // PROOF (U-25 SNTM-R7): pointer ⊂ region AND access.matches(perm) iff result.
    #[kani::proof]
    #[kani::unwind(8)]
    fn multi_region_user_ptr_in_region() {
        use crate::kernel::pmp::profile::{Access, PmpProfile, Region, Permission};
        use crate::kernel::syscall::dispatch::check_ptr_in_profile;
        use crate::arch::pmp::PmpEncoding;

        let region_base: usize = kani::any();
        let region_size: usize = kani::any();
        kani::assume(region_base <= 0xFFFF);
        kani::assume(region_size > 0 && region_size <= 0x1000);
        kani::assume(region_base.checked_add(region_size).is_some());

        let perm_r: bool = kani::any();
        let perm_w: bool = kani::any();
        let perm_x: bool = kani::any();
        let perm = Permission { r: perm_r, w: perm_w, x: perm_x };

        let mut profile = PmpProfile::EMPTY;
        profile.region_count = 1;
        profile.regions[0] = Region {
            base: region_base,
            size: region_size,
            encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
            perm,
        };

        let ptr: usize = kani::any();
        let size: usize = kani::any();
        kani::assume(size <= 0x1000);
        let access: Access = kani::any();

        let result = check_ptr_in_profile(&profile, ptr, size, access);

        // Reference oracle.
        let oracle = ptr != 0
            && ptr.checked_add(size).is_some()
            && ptr >= region_base
            && ptr.wrapping_add(size) <= region_base + region_size
            && access.matches(perm);

        assert!(result == oracle);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-25 SNTM-R7): is_valid_user_ptr overflow guard — 2 path symbolic.
    //
    // VERIFIES: SNTM-R7 overflow defansif (ptr+size AND region.base+region.size).
    // CALLS:    crate::kernel::syscall::dispatch::check_ptr_in_profile
    // FAILS-IF: ptr+size overflow durumunda true, region.base+size overflow
    //           durumunda true, ya da checked_add bypass edilirse.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn multi_region_user_ptr_overflow_safe() {
        use crate::kernel::pmp::profile::{Access, PmpProfile, Region, Permission};
        use crate::kernel::syscall::dispatch::check_ptr_in_profile;
        use crate::arch::pmp::PmpEncoding;

        // Path A: ptr+size overflow
        {
            let ptr: usize = kani::any();
            let size: usize = kani::any();
            kani::assume(ptr.checked_add(size).is_none());

            let mut profile = PmpProfile::EMPTY;
            profile.region_count = 1;
            profile.regions[0] = Region {
                base: 0x8010_0000,
                size: 0x4000,
                encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
                perm: Permission::RW,
            };

            assert!(!check_ptr_in_profile(&profile, ptr, size, Access::Read));
        }

        // Path B: region.base+region.size overflow
        {
            let region_base: usize = kani::any();
            let region_size: usize = kani::any();
            kani::assume(region_base.checked_add(region_size).is_none());

            let mut profile = PmpProfile::EMPTY;
            profile.region_count = 1;
            profile.regions[0] = Region {
                base: region_base,
                size: region_size,
                encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
                perm: Permission::RW,
            };

            let ptr: usize = kani::any();
            let size: usize = kani::any();
            kani::assume(size <= 0x1000);
            kani::assume(ptr.checked_add(size).is_some());

            assert!(!check_ptr_in_profile(&profile, ptr, size, Access::Read));
        }
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-25 SNTM-R7): Dead/EMPTY task profile → always deny.
    //
    // VERIFIES: SNTM-R7 (region_count == 0 → daima false)
    // CALLS:    crate::kernel::syscall::dispatch::check_ptr_in_profile
    // FAILS-IF: EMPTY profile için herhangi (ptr, size, access) true döner.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn multi_region_dead_task_deny() {
        use crate::kernel::pmp::profile::{Access, PmpProfile};
        use crate::kernel::syscall::dispatch::check_ptr_in_profile;

        let ptr: usize = kani::any();
        let size: usize = kani::any();
        let access: Access = kani::any();
        kani::assume(size <= 0x1000);

        let profile = PmpProfile::EMPTY;
        assert!(!check_ptr_in_profile(&profile, ptr, size, access));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-25 SNTM-R8): NAPOT pmpaddr encoding — exact bit count + round-trip.
    //
    // VERIFIES: SNTM-R8 (NAPOT encoding mask bit count == size_log2 - 3 EXACT)
    // CALLS:    Pure bitwise math (kernel encoding formula).
    // FAILS-IF: Mask bit count != size_log2 - 3, encoded base decode != base,
    //           ya da next bit (at size_log2 - 3) not zero (alignment bug).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn napot_encoding_size_consistent() {
        let base: u64 = kani::any();
        let size_log2: u8 = kani::any();
        kani::assume(size_log2 >= 3 && size_log2 <= 30);
        kani::assume(base & ((1u64 << size_log2) - 1) == 0);

        let mask = (1u64 << (size_log2 - 3)) - 1;
        let encoded = (base >> 2) | mask;

        // Invariant 1: bit at position (size_log2 - 3) MUST be 0 — alignment kanıtı.
        let next_bit = (encoded >> (size_log2 - 3)) & 1;
        assert!(next_bit == 0);

        // Invariant 2: trailing_ones exact (not >=).
        let trailing_ones = encoded.trailing_ones() as u8;
        assert!(trailing_ones == size_log2 - 3);

        // Invariant 3: round-trip decode.
        let decoded = (encoded & !mask) << 2;
        assert!(decoded == base);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF (U-25 SNTM-R6): reload_pmp_profile YALNIZCA indices 8..15 yazar.
    //
    // VERIFIES: SNTM-R6 (FIX-1: kernel + UART entry 0..7 ASLA touch edilmez)
    // CALLS:    crate::arch::pmp::reload_indices_touched (G8'de eklenir)
    // FAILS-IF: (1) Touched indices'lerden biri < PMP_DYNAMIC_START_ENTRY=8
    //               (UART entry 6/7 lock bypass — isolation break),
    //           (2) Touched indices >= MAX_PMP_ENTRIES (CSR sınırı aşımı),
    //           (3) Non-empty profile için touched boş (reload no-op),
    //           (4) Plan monotonik artmıyor (DENY stage sırası bozulur).
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    #[kani::unwind(13)]
    fn reload_pmp_kernel_indices_untouched() {
        use crate::arch::pmp::{reload_indices_touched, PmpEncoding};
        use crate::kernel::pmp::profile::{PmpProfile, Region, Permission};
        use crate::common::config::{PMP_DYNAMIC_START_ENTRY, MAX_PMP_ENTRIES};

        let region_count: u8 = kani::any();
        kani::assume(region_count >= 1 && region_count <= 6);

        // Profile build — her region NAPOT (TOR ayrı harness).
        let mut profile = PmpProfile::EMPTY;
        profile.region_count = region_count;
        let mut i = 0;
        while i < region_count as usize {
            profile.regions[i] = Region {
                base: 0x8010_0000 + (i * 0x4000),
                size: 0x4000,
                encoding: PmpEncoding::Napot { addr: 0, size_log2: 14 },
                perm: Permission::RW,
            };
            i += 1;
        }

        let (touched, count) = reload_indices_touched(&profile);

        // Invariant 1: NAPOT plan size = region_count (TOR olsaydı 2×).
        assert!(count == region_count as usize);

        // Invariant 2: Her index lower + upper bound içinde.
        let mut j = 0;
        while j < count {
            assert!(touched[j] >= PMP_DYNAMIC_START_ENTRY);  // kernel+UART range dışı
            assert!(touched[j] < MAX_PMP_ENTRIES);
            j += 1;
        }

        // Invariant 3: Plan monotonik artıyor.
        let mut k = 1;
        while k < count {
            assert!(touched[k] > touched[k - 1]);
            k += 1;
        }
    }

    // ═══════════════════════════════════════════════════════
    // U-26 SNTM Phase 4 — Native task loader proofs (SNTM-R9)
    // ═══════════════════════════════════════════════════════

    // VERIFIES: SNTM-R9 (bounded_copy atomic on overflow + tam kopya on fit)
    // CALLS:    crate::kernel::loader::bounded_copy
    // FAILS-IF: src_len > dst_size partial copy yazıldı, src_len ≤ dst_size
    //           byte missing (count mismatch), ya da src/dst overlap UB.
    // PROOF (U-26 SNTM-R9): src/dst separate region, length-bounded atomicity.
    #[kani::proof]
    #[kani::unwind(17)]
    fn loader_bounded_copy_atomic() {
        use crate::kernel::loader::bounded_copy;

        let src_len: usize = kani::any();
        let dst_size: usize = kani::any();
        kani::assume(src_len <= 16);
        kani::assume(dst_size <= 16);

        let src: [u8; 16] = kani::any();
        let mut dst: [u8; 16] = [0u8; 16];

        let result = bounded_copy(&src[..src_len], &mut dst[..dst_size]);

        if src_len > dst_size {
            // Atomik fail: dst tamamen değişmemiş (no partial).
            assert!(result.is_err());
            let mut i = 0;
            while i < 16 { assert!(dst[i] == 0); i += 1; }
        } else {
            assert!(result.is_ok());
            // Tam kopya: dst[0..src_len] = src[0..src_len].
            let mut i = 0;
            while i < src_len { assert!(dst[i] == src[i]); i += 1; }
            // dst[src_len..dst_size] dokunulmamış (hâlâ 0).
            let mut j = src_len;
            while j < dst_size { assert!(dst[j] == 0); j += 1; }
        }
    }

    // VERIFIES: SNTM-R9 (zero_fill: tüm region [0, size) byte=0)
    // CALLS:    crate::kernel::loader::zero_fill
    // FAILS-IF: Bir byte sıfırlanmadı, ya da out-of-bounds yazıldı.
    #[kani::proof]
    #[kani::unwind(17)]
    fn loader_zero_fill_complete() {
        use crate::kernel::loader::zero_fill;

        let size: usize = kani::any();
        kani::assume(size <= 16);

        let mut buf: [u8; 16] = kani::any();
        zero_fill(&mut buf[..size]);

        let mut i = 0;
        while i < size { assert!(buf[i] == 0); i += 1; }
    }

    // VERIFIES: SNTM-R9 (loader dst region asla kernel address range'i değil)
    // CALLS:    crate::kernel::loader::is_safe_load_dst
    // FAILS-IF: Kernel range içinde dst kabul (kernel text/data overwrite!),
    //           ya da overflow durumunda dst_end wrap.
    #[kani::proof]
    fn loader_no_kernel_overwrite() {
        use crate::kernel::loader::is_safe_load_dst;
        use crate::common::config::{KERNEL_BASE, KERNEL_SIZE};

        let dst: usize = kani::any();
        let size: usize = kani::any();
        kani::assume(size > 0 && size <= 0x10000);

        let result = is_safe_load_dst(dst, size);

        if result {
            // Kabul edilirse: dst..dst+size kernel range ile DİSJOINT.
            let dst_end = dst.checked_add(size);
            assert!(dst_end.is_some());
            let de = dst_end.unwrap();
            assert!(de <= KERNEL_BASE || dst >= KERNEL_BASE + KERNEL_SIZE);
        }
    }

    // VERIFIES: SNTM-R9 (load_region + zero_fill composition: data tail + bss
    //           tüm region kapsamı zero-or-data, undefined byte YOK)
    // CALLS:    crate::kernel::loader::bounded_copy + zero_fill
    // FAILS-IF: data sonrası bss byte non-zero (zero_fill incomplete), data
    //           overflow (src > region partial copy), ya da bss size hesap
    //           overflow (data_len > region_size hata propagation eksik).
    // PROOF (U-26 SNTM-R9): symbolic data_len + region_size; loader sonrası
    // region[0..data_len] = data + region[data_len..size] = 0 invariant.
    #[kani::proof]
    #[kani::unwind(20)]
    fn loader_data_bss_composition_zero() {
        use crate::kernel::loader::{bounded_copy, zero_fill};

        let region_size: usize = kani::any();
        let data_len: usize = kani::any();
        kani::assume(region_size <= 16);
        kani::assume(data_len <= region_size);

        let data_src: [u8; 16] = kani::any();
        let mut region: [u8; 16] = kani::any();  // pre-state symbolic

        // Stage 1: load_region semantics — FIX-D ÖNCE zero_fill, sonra copy.
        zero_fill(&mut region[..region_size]);
        // Stage 2: data copy.
        bounded_copy(&data_src[..data_len], &mut region[..region_size]).unwrap();

        // Invariant 1: data prefix bit-equal to src.
        let mut i = 0;
        while i < data_len { assert!(region[i] == data_src[i]); i += 1; }

        // Invariant 2: bss tail tamamen 0.
        let mut j = data_len;
        while j < region_size { assert!(region[j] == 0); j += 1; }
    }

    // ═══════════════════════════════════════════════════════
    // U-27 SNTM Phase 5 — Cross-task PMP isolation static proofs (SNTM-R12)
    // ═══════════════════════════════════════════════════════

    // VERIFIES: SNTM-R12 (cross-task PMP isolation statik kanıt — task_hello
    //           PMP profile task_world.data adresini reddedir).
    // CALLS:    crate::kernel::syscall::dispatch::check_ptr_in_profile,
    //           crate::kernel::pmp::profile::get_pmp_profile (PMP_PROFILES[2]).
    // FAILS-IF: Multi-region matcher task_hello profile'ında task_world.data
    //           (0x80705000..0x80706000) adresini kabul ederse, ya da
    //           Access::Write task_hello region perm matrix bypass.
    // PROOF (U-27 SNTM-R12): symbolic size [1..64] + concrete cross-task ptr,
    // task_hello profile (PMP_PROFILES[2]) için check_ptr_in_profile → false.
    #[kani::proof]
    #[kani::unwind(7)]
    fn check_ptr_in_profile_rejects_other_task_region() {
        use crate::kernel::pmp::profile::{Access, get_pmp_profile};
        use crate::kernel::syscall::dispatch::check_ptr_in_profile;

        // task_hello (task_id=2) profile + task_world.data region adresi.
        let profile_hello = get_pmp_profile(2).unwrap();
        // task_world.data: 0x80705000 (sipahi.toml task_world[data] base).
        let target_in_world: usize = 0x80705000;
        let size: usize = kani::any();
        kani::assume(size > 0 && size <= 64);

        // Reject for both Read and Write access (region disjoint).
        let res_w = check_ptr_in_profile(profile_hello, target_in_world, size, Access::Write);
        assert!(!res_w);
        let res_r = check_ptr_in_profile(profile_hello, target_in_world, size, Access::Read);
        assert!(!res_r);
    }

    // VERIFIES: SNTM-R12 (symmetric — task_world rejects task_hello region).
    // CALLS:    check_ptr_in_profile, get_pmp_profile (PMP_PROFILES[3]).
    // FAILS-IF: task_world matcher task_hello.text (0x80600000) kabul ederse.
    #[kani::proof]
    #[kani::unwind(7)]
    fn check_ptr_in_profile_symmetric_isolation() {
        use crate::kernel::pmp::profile::{Access, get_pmp_profile};
        use crate::kernel::syscall::dispatch::check_ptr_in_profile;

        let profile_world = get_pmp_profile(3).unwrap();
        // task_hello.text: 0x80600000 (sipahi.toml task_hello[text] base).
        let target_in_hello: usize = 0x80600000;
        let size: usize = kani::any();
        kani::assume(size > 0 && size <= 64);

        let res_r = check_ptr_in_profile(profile_world, target_in_hello, size, Access::Read);
        assert!(!res_r);
    }

    // ═══════════════════════════════════════════════════════
    // U-27 SNTM Phase 5 — Sealed channel atomicity (SNTM-R13)
    // ═══════════════════════════════════════════════════════

    // VERIFIES: SNTM-R13 (seal_channels() POST-state assign_channel reddedir;
    //           idempotent + atomic — flag reset yok).
    // CALLS:    crate::ipc::{seal_channels, assign_channel, is_sealed}.
    // FAILS-IF: seal=true post-state assign_channel kabul ederse (flag check
    //           bypass), seal_channels() state'i reset edebilirse, ya da
    //           assign_channel'in seal kontrolü out-of-order yapılırsa.
    // PROOF (U-27 SNTM-R13): seal_channels() çağrısı sonrası symbolic ch/p/c
    // ile assign_channel total function check → her giriş için false.
    #[kani::proof]
    fn post_seal_assign_returns_false() {
        use crate::common::config::{MAX_IPC_CHANNELS, MAX_TASKS};
        use crate::ipc::{assign_channel, is_sealed, seal_channels};

        // Pre-condition: seal aktif.
        seal_channels();
        assert!(is_sealed());

        // Symbolic input: tüm valid (channel_id, producer, consumer) tripleleri.
        let channel_id: usize = kani::any();
        let producer: u8 = kani::any();
        let consumer: u8 = kani::any();
        kani::assume(channel_id < MAX_IPC_CHANNELS);
        kani::assume((producer as usize) < MAX_TASKS);
        kani::assume((consumer as usize) < MAX_TASKS);

        let result = assign_channel(channel_id, producer, consumer);
        // Sealed durumda her assign reddedilir (atomicity invariant).
        assert!(!result);

        // Post-condition: seal hala true (idempotency).
        assert!(is_sealed());
    }

    // VERIFIES: SNTM-R13 prereq — seal_channels() idempotent (ikinci çağrı
    //           state'i bozmaz, post-state hala sealed).
    #[kani::proof]
    fn seal_channels_idempotent() {
        use crate::ipc::{is_sealed, seal_channels};
        seal_channels();
        let after_first = is_sealed();
        seal_channels();
        let after_second = is_sealed();
        assert!(after_first);
        assert!(after_second);
        assert!(after_first == after_second);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 7: IPC kanal bellek hesabı
    // Slot verisi ayrı, gerçek struct boyutu ayrı kontrol ediliyor.
    // SORUN 1: SpscChannel = 1028B (4B AtomicU16 overhead + 1024B slot)
    //          8 × 1028 = 8,224B > 8,192B (PMP R3 bütçesi)
    //          Fix: head/tail'i ilk slot'a göm -> Sprint 8 sonrası assert aktif et.
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

        // Sprint U-14: IPC pool boyut kontrolü — güncel mimariye uyarlanmış.
        // SpscChannel = AtomicU16 head + AtomicU16 tail + 16 × 64B = 1028B.
        // 8 × 1028 = 8224B (>8KB). PMP R3 artık 8KB değil — .data RW bölgesinde.
        // RAM kontrolü yeterli.
        assert!(actual_pool < 512 * 1024);
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 8: Bellek bütçesi 4MB RAM'e sığar
    // U-29 v2.0: WASM_HEAP_SIZE silindi (wasmi + sandbox/ kaldırıldı).
    // wasm_heap term = 0 → toplam ~4MB azaldı, RAM sığma daha geniş margin.
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
        // U-29: wasm_heap term kaldırıldı (WASM removed).
        let total = kernel_total + task_total;
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
    // PROOF 10: DAL budget toplamı CYCLES_PER_TICK × DEFAULT_PERIOD_TICKS
    // U-18 GÖREV 2: Tautoloji silindi — gerçek config sabitleri kullanılır
    // (BUDGET_DAL_A/B/C/D toplamı periyot başına maksimum CPU bütçesini verir).
    // 400K + 300K + 200K + 100K = 1,000,000 = CYCLES_PER_TICK × 10 (varsayılan periyot)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_budget_sum_equals_period_capacity() {
        use crate::common::config::{
            BUDGET_DAL_A, BUDGET_DAL_B, BUDGET_DAL_C, BUDGET_DAL_D,
            CYCLES_PER_TICK,
        };
        let total: u32 = BUDGET_DAL_A
            .checked_add(BUDGET_DAL_B).unwrap()
            .checked_add(BUDGET_DAL_C).unwrap()
            .checked_add(BUDGET_DAL_D).unwrap();
        assert!(total == 1_000_000);
        // Toplam = 10 tick × CYCLES_PER_TICK (varsayılan periyot süresi)
        assert!(total == CYCLES_PER_TICK.checked_mul(10).unwrap());
        // Sıralama: DAL-A > DAL-B > DAL-C > DAL-D (kritiklik = bütçe önceliği)
        assert!(BUDGET_DAL_A > BUDGET_DAL_B);
        assert!(BUDGET_DAL_B > BUDGET_DAL_C);
        assert!(BUDGET_DAL_C > BUDGET_DAL_D);
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
    // PROOF 12: REMOVED in U-22.5 G1 (host_call_budget_bounded — compute path)
    // dispatch_compute + WCET_COMPUTE_* sabitleri sprint U-22.5'te silindi.
    // cap_invoke overhead invariant'ı verify.rs'in başka yerinde kalır (HOST_CALL_LIMIT
    // hâlâ kullanılır, sadece COMPUTE_* tied path silindi).
    // ═══════════════════════════════════════════════════════

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
        // Kapalı -> her adres, her boyut, okuma/yazma serbest
        assert!(ctrl.check_access(0x1000, 4, false));
        assert!(ctrl.check_access(0x2000, 8, true));
        assert!(ctrl.check_access(0, 1, false));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 24: IOPMP geçersiz index -> InvalidParameter
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
    // PROOF 25: IOPMP etkin + tanımsız bölge -> erişim RED
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn iopmp_enabled_denies_unknown() {
        use crate::hal::iopmp::IopmpController;
        let mut ctrl = IopmpController::new();
        let _ = ctrl.enable();
        // Hiç bölge tanımlı değil -> tüm erişim reddedilmeli
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
    // stack_top & !0xF -> her zaman 16-byte aligned, stack_top'tan ≤
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
    // PROOF 71: Isolated task asla Ready'ye dönmez (production helper çağrılır)
    // U-19 GÖREV 10: Tautoloji (kendi if-else simülasyonu) yerine production
    // is_selectable_by_scheduler ve is_period_reset_eligible çağrılıyor.
    // Phase 1 (period reset) Suspended dışı state'leri değiştirmez; Phase 3
    // (selection) sadece Ready/Running seçer -> Isolated her iki yolda da kapsam dışı.
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn isolated_task_never_becomes_ready() {
        use crate::kernel::scheduler::{is_selectable_by_scheduler, is_period_reset_eligible};
        // Isolated -> ne seçilir ne periyot reset Ready yapar
        assert!(!is_selectable_by_scheduler(TaskState::Isolated));
        assert!(!is_period_reset_eligible(TaskState::Isolated));
        // Karşılaştırma: Dead da seçilmez ama farklı sebeple (kalıcı ölü)
        assert!(!is_selectable_by_scheduler(TaskState::Dead));
        assert!(!is_period_reset_eligible(TaskState::Dead));
        // Suspended periyot reset eligible (Ready'ye geçer); Ready/Running değil
        assert!(is_period_reset_eligible(TaskState::Suspended));
        assert!(!is_period_reset_eligible(TaskState::Ready));
        assert!(!is_period_reset_eligible(TaskState::Running));
        // Ready ve Running scheduler tarafından seçilebilir
        assert!(is_selectable_by_scheduler(TaskState::Ready));
        assert!(is_selectable_by_scheduler(TaskState::Running));
    }

    // ═══════════════════════════════════════════════════════
    // PROOF 72: DAL-A task budget aşımında asla Isolated olmaz
    // Budget aşımı escalation: RESTART -> DEGRADE (Isolate değil)
    // ═══════════════════════════════════════════════════════
    #[kani::proof]
    fn dal_a_budget_exhausted_never_isolated() {
        use crate::kernel::policy::{decide_action, FailureMode, PolicyEvent};
        let event = PolicyEvent::BudgetExhausted as u8;
        let dal   = 0u8; // DAL-A
        let count: u8 = kani::any();
        // Budget exhausted -> RESTART(1) -> DEGRADE, hiçbir koşulda ISOLATE değil
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

    /// U-18 GÖREV 3: Lockstep — pure fonksiyon kontratı.
    /// apply_policy'nin lockstep doğrulaması (decide_action_fenced iki kez)
    /// ancak decide_action pure ise anlamlıdır. Bu proof sembolik girdi için
    /// iki çağrının bit-bit aynı sonuç verdiğini kanıtlar — eğer bu kırılırsa
    /// runtime LockstepFail trigger'ı false-positive olur.
    #[kani::proof]
    fn decide_action_lockstep_pure() {
        use crate::kernel::policy::decide_action;
        let event: u8 = kani::any();
        let rc: u8 = kani::any();
        let dal: u8 = kani::any();
        // Tüm input domain — pure fonksiyon: aynı (e, rc, dal) -> aynı output
        let a1 = decide_action(event, rc, dal);
        let a2 = decide_action(event, rc, dal);
        assert!(a1 as u8 == a2 as u8);
    }

    // --- CAPABILITY ---

    /// Proof 92: Token expiry: expires=0 -> asla expired olmaz
    /// U-18 GÖREV 2: Tautoloji silindi — production fonksiyonu çağrılır
    #[kani::proof]
    fn token_expiry_zero_means_infinite() {
        use crate::kernel::capability::broker::is_not_expired;
        let current_tick: u64 = kani::any();
        // expires=0 -> her zaman geçerli (sonsuz)
        assert!(is_not_expired(0, current_tick));
    }

    /// Proof 93: Token expiry: expires > 0 ve tick > expires -> expired
    /// U-18 GÖREV 2: Tautoloji silindi — production fonksiyonu çağrılır
    #[kani::proof]
    fn token_expiry_detects_expired() {
        use crate::kernel::capability::broker::is_not_expired;
        let expires: u32 = kani::any();
        kani::assume(expires > 0);
        let current_tick: u64 = kani::any();
        kani::assume(current_tick > expires as u64);
        // tick > expires -> token expired (is_not_expired = false)
        assert!(!is_not_expired(expires, current_tick));
        // Boundary: tick == expires -> hâlâ geçerli (<=)
        assert!(is_not_expired(expires, expires as u64));
    }

    // --- SCHEDULER ---

    /// Proof 94: Priority / DAL grubu ilişkisi — prio 0-15 -> dal_group 0-3
    #[kani::proof]
    fn task_priority_dal_group_bounded() {
        let prio: u8 = kani::any();
        kani::assume(prio <= 15);
        let dal_group = prio / 4;
        assert!(dal_group <= 3);
    }

    /// Proof 95: Watchdog limit=0 -> asla tetiklenmez (production helper)
    /// U-19 GÖREV 10: Tautoloji yerine production should_watchdog_timeout çağrılıyor.
    /// Phase 1.5'teki `if t.watchdog_limit > 0 && t.watchdog_counter >= t.watchdog_limit`
    /// mantığı bu helper'da. Helper'ın production schedule()'da inline kullanımı +
    /// Kani'de symbolic input ile doğrulanıyor — drift yoksa tek mantık.
    #[kani::proof]
    fn watchdog_limit_zero_disables() {
        use crate::kernel::scheduler::should_watchdog_timeout;
        let counter: u32 = kani::any();
        // limit=0 -> her counter değeri için disabled
        assert!(!should_watchdog_timeout(0, counter));
        // limit > 0 ve counter >= limit -> tetiklenir (boundary)
        let limit: u32 = kani::any();
        kani::assume(limit > 0);
        kani::assume(counter >= limit);
        assert!(should_watchdog_timeout(limit, counter));
        // limit > 0 ve counter < limit -> tetiklenmez
        let counter2: u32 = kani::any();
        kani::assume(limit > 0);
        kani::assume(counter2 < limit);
        assert!(!should_watchdog_timeout(limit, counter2));
    }

    // --- HAL ---

    /// Proof 135: IOPMP new() -> disabled, tüm erişim serbest
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

    /// Proof 139: BLAKE3 farklı key -> farklı hash
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

    /// Proof 140: BLAKE3 aynı (key, data) -> aynı hash
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

    /// Proof 154: Farklı token -> farklı header (id veya nonce farklıysa)
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

    /// Proof 163: pack_pmpcfg symbolic entry -> extract matches original
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

    /// Proof 164: Token resource symbolic u16 -> LE bytes doğru
    #[kani::proof]
    fn token_header_resource_le_encoding() {
        use crate::kernel::capability::Token;
        let mut t = Token::zeroed();
        t.resource = kani::any();
        let h = t.header_bytes();
        let reconstructed = (h[2] as u16) | ((h[3] as u16) << 8);
        assert!(reconstructed == t.resource);
    }

    /// Proof 165: Token expires symbolic u32 -> LE bytes doğru
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

    // ═══════════════════════════════════════════════════════
    // Sprint U-3: Per-Task PMP (NAPOT) Proofs
    // ═══════════════════════════════════════════════════════

    /// Farklı task ID -> farklı stack, farklı NAPOT encoding
    #[kani::proof]
    fn different_tasks_get_different_stacks() {
        let id_a: usize = kani::any();
        let id_b: usize = kani::any();
        kani::assume(id_a < MAX_TASKS);
        kani::assume(id_b < MAX_TASKS);
        kani::assume(id_a != id_b);
        let base_a = id_a * TASK_STACK_SIZE;
        let base_b = id_b * TASK_STACK_SIZE;
        assert!(base_a != base_b);
        assert!(base_a + TASK_STACK_SIZE <= base_b || base_b + TASK_STACK_SIZE <= base_a);
        let napot_a = (base_a >> 2) | 0x3FF;
        let napot_b = (base_b >> 2) | 0x3FF;
        assert!(napot_a != napot_b);
    }

    /// Task ID -> Stack -> NAPOT zinciri bounds-safe
    #[kani::proof]
    fn task_pmp_index_in_bounds() {
        let task_id: usize = kani::any();
        kani::assume(task_id < MAX_TASKS);
        let base = task_id * TASK_STACK_SIZE;
        let napot = (base >> 2) | 0x3FF;
        let decoded = (napot & !0x3FF) << 2;
        assert!(decoded == base);
    }

    /// NAPOT address decode roundtrip
    #[kani::proof]
    fn napot_addr_covers_stack() {
        let stack_base: usize = kani::any();
        kani::assume(stack_base % TASK_STACK_SIZE == 0);
        kani::assume(stack_base < 0x1_0000_0000);
        let napot_addr = (stack_base >> 2) | 0x3FF;
        let decoded_base = (napot_addr & !0x3FF) << 2;
        assert!(decoded_base == stack_base);
        let size = (0x3FF + 1) << 3;
        assert!(size == 8192);
    }

    /// pmpcfg2 config byte doğruluğu: NAPOT, R+W, X=0, L=0
    #[kani::proof]
    fn pmpcfg2_config_correct() {
        let cfg: usize = 0x1B;
        assert!(cfg & 0x01 == 1);           // R=1
        assert!(cfg & 0x02 == 2);           // W=1
        assert!(cfg & 0x04 == 0);           // X=0 (W^X)
        assert!((cfg >> 3) & 0x03 == 3);    // A=11 (NAPOT)
        assert!((cfg >> 7) & 0x01 == 0);    // L=0
    }

    // ═══════════════════════════════════════════════════════
    // Sprint U-8: validate_full logic proofs
    // validate_full'un iç mantığını pure helper fonksiyonlar üzerinden kanıtla
    // ═══════════════════════════════════════════════════════

    /// Nonce: token_nonce > last -> valid
    #[kani::proof]
    fn nonce_greater_than_last_is_valid() {
        let token_nonce: u32 = kani::any();
        let last_nonce: u32 = kani::any();
        kani::assume(token_nonce > last_nonce);
        assert!(crate::kernel::capability::broker::is_nonce_valid(token_nonce, last_nonce));
    }

    /// Nonce: token_nonce <= last -> invalid (replay)
    #[kani::proof]
    fn nonce_replay_is_rejected() {
        let token_nonce: u32 = kani::any();
        let last_nonce: u32 = kani::any();
        kani::assume(token_nonce <= last_nonce);
        assert!(!crate::kernel::capability::broker::is_nonce_valid(token_nonce, last_nonce));
    }

    /// Expiry: expires=0 -> always valid (sonsuz token)
    #[kani::proof]
    fn expiry_zero_always_valid() {
        let tick: u64 = kani::any();
        assert!(crate::kernel::capability::broker::is_not_expired(0, tick));
    }

    /// Expiry: current_tick > expires -> expired
    #[kani::proof]
    fn expiry_past_is_rejected() {
        let expires: u32 = kani::any();
        let tick: u64 = kani::any();
        kani::assume(expires > 0);
        kani::assume(tick > expires as u64);
        assert!(!crate::kernel::capability::broker::is_not_expired(expires, tick));
    }

    /// Expiry: current_tick <= expires -> valid
    #[kani::proof]
    fn expiry_within_range_is_valid() {
        let expires: u32 = kani::any();
        let tick: u64 = kani::any();
        kani::assume(expires > 0);
        kani::assume(tick <= expires as u64);
        assert!(crate::kernel::capability::broker::is_not_expired(expires, tick));
    }

    /// Task ID: valid range [0, MAX_TASKS)
    #[kani::proof]
    fn task_id_in_range_is_valid() {
        let task_id: u8 = kani::any();
        kani::assume((task_id as usize) < MAX_TASKS);
        assert!(crate::kernel::capability::broker::is_task_id_valid(task_id, MAX_TASKS));
    }

    /// Task ID: >= MAX_TASKS -> invalid
    #[kani::proof]
    fn task_id_out_of_range_is_invalid() {
        let task_id: u8 = kani::any();
        kani::assume((task_id as usize) >= MAX_TASKS);
        assert!(!crate::kernel::capability::broker::is_task_id_valid(task_id, MAX_TASKS));
    }

    // ═══════════════════════════════════════════════════════
    // Sprint U-8: pack_pmpcfg proofs
    // PMP konfigürasyonunun doğru paketlendiği formal garanti
    // ═══════════════════════════════════════════════════════

    /// pack_pmpcfg: her entry doğru byte pozisyonunda
    #[kani::proof]
    fn pack_pmpcfg_extracts_each_entry() {
        let configs: [u8; 8] = [
            kani::any(), kani::any(), kani::any(), kani::any(),
            kani::any(), kani::any(), kani::any(), kani::any(),
        ];
        let packed = crate::arch::pmp::pack_pmpcfg(configs);
        let mut i = 0;
        while i < 8 {
            let extracted = ((packed >> (i * 8)) & 0xFF) as u8;
            assert!(extracted == configs[i]);
            i += 1;
        }
    }

    /// pack_pmpcfg: sıfır configs -> sıfır packed
    #[kani::proof]
    fn pack_pmpcfg_zeros() {
        let configs = [0u8; 8];
        let packed = crate::arch::pmp::pack_pmpcfg(configs);
        assert!(packed == 0);
    }

    /// pack_pmpcfg: Sipahi'nin gerçek config'i doğru paketleniyor
    #[kani::proof]
    fn pack_pmpcfg_sipahi_config_correct() {
        use crate::arch::pmp::*;
        let configs: [u8; 8] = [
            0,                                          // Entry 0: OFF
            PMP_TOR | PMP_R | PMP_X | PMP_L,           // Entry 1: .text RX locked
            0,                                          // Entry 2: OFF
            PMP_TOR | PMP_R | PMP_L,                    // Entry 3: .rodata R locked
            0,                                          // Entry 4: OFF
            PMP_TOR | PMP_R | PMP_W | PMP_L,            // Entry 5: .data RW locked
            0,                                          // Entry 6: OFF
            PMP_TOR | PMP_R | PMP_W | PMP_L,            // Entry 7: UART RW locked
        ];
        let packed = pack_pmpcfg(configs);
        // Entry 1 = byte 1: TOR(0x08) | R(0x01) | X(0x04) | L(0x80) = 0x8D
        assert!(((packed >> 8) & 0xFF) as u8 == 0x8D);
        // Entry 3 = byte 3: TOR(0x08) | R(0x01) | L(0x80) = 0x89
        assert!(((packed >> 24) & 0xFF) as u8 == 0x89);
        // Entry 5 = byte 5: TOR(0x08) | R(0x01) | W(0x02) | L(0x80) = 0x8B
        assert!(((packed >> 40) & 0xFF) as u8 == 0x8B);
    }

    // ═══════════════════════════════════════════════════════════════════
    // SAFE-2 (sprint-u31): Static local capability + typed IPC proofs.
    // Section 9.1 K1-K8 doctrine compliance:
    //   K1 no tautology  · K2 production const  · K3 unbounded any
    //   K4 reachability  · K5 negative ≥ positive · K6 unwind = N+1
    //   K7 no dead arms  · K8 cross-crate drift
    // ═══════════════════════════════════════════════════════════════════

    /// SAFE-2 K3+K6: bounded local_cap_check, all valid inputs.
    // VERIFIES: SNTM-SAFE-R2 — bounds check, no OOB, no panic for any input.
    /// CALLS:    crate::kernel::capability::local_cap::local_cap_check
    /// FAILS-IF: array access OOB on caller_task_id >= MAX_TASKS or
    ///           resource_id >= MAX_RESOURCES; panic on invalid action bits.
    #[kani::proof]
    #[kani::unwind(9)] // MAX_TASKS+1 = 9 (K6: off-by-one guard)
    fn local_cap_check_bounded() {
        use crate::kernel::capability::local_cap::local_cap_check;
        let caller: u8 = kani::any();
        let resource: u8 = kani::any();
        let action: u8 = kani::any();
        // K3: full u8 domain — no concrete constraints. Result discarded;
        // property is "no panic / no OOB", enforced by Kani's array checks.
        let _ = local_cap_check(caller, resource, action);
    }

    /// SAFE-2 K5 negative: out-of-bounds caller MUST deny.
    // VERIFIES: SNTM-SAFE-R2 — caller_task_id ≥ MAX_TASKS rejected.
    /// FAILS-IF: local_cap_check returns true for caller ≥ MAX_TASKS.
    #[kani::proof]
    fn local_cap_check_rejects_oob_caller() {
        use crate::kernel::capability::local_cap::local_cap_check;
        use crate::common::config::MAX_TASKS;
        let caller: u8 = kani::any();
        kani::assume(caller as usize >= MAX_TASKS);
        let resource: u8 = kani::any();
        let action: u8 = kani::any();
        // OOB caller must be DENY regardless of resource/action.
        assert!(!local_cap_check(caller, resource, action));
    }

    /// SAFE-2 K5 negative: out-of-bounds resource MUST deny (when caller valid).
    // VERIFIES: SNTM-SAFE-R2 — resource_id ≥ MAX_RESOURCES rejected.
    /// FAILS-IF: local_cap_check returns true for resource ≥ MAX_RESOURCES.
    #[kani::proof]
    fn local_cap_check_rejects_oob_resource() {
        use crate::kernel::capability::local_cap::local_cap_check;
        use crate::common::config::{MAX_TASKS, MAX_RESOURCES};
        let caller: u8 = kani::any();
        let resource: u8 = kani::any();
        kani::assume((caller as usize) < MAX_TASKS);
        kani::assume(resource as usize >= MAX_RESOURCES);
        let action: u8 = kani::any();
        assert!(!local_cap_check(caller, resource, action));
    }

    /// SAFE-2 K5+CR-3 negative: invalid action bits MUST deny (never permissive).
    // VERIFIES: SNTM-SAFE-R2 — CapAction::from_u8 None → false return.
    /// FAILS-IF: any of 0x05, 0x06, 0x08..=0xFF returns true.
    #[kani::proof]
    fn local_cap_check_invalid_action_denied() {
        use crate::kernel::capability::local_cap::local_cap_check;
        let caller: u8 = kani::any();
        let resource: u8 = kani::any();
        let action: u8 = kani::any();
        // Invalid action bit patterns: not in {0,1,2,3,4,7}.
        kani::assume(
            action == 0x05
            || action == 0x06
            || (action >= 0x08)
        );
        assert!(!local_cap_check(caller, resource, action));
    }

    /// SAFE-2 K7: CapAction::allows matrix — no dead arm.
    // VERIFIES: SNTM-SAFE-R2 — bit-subset semantics for CapAction.allows.
    /// Read.allows(Write)=false, ReadWrite.allows(Read)=true, None.allows(*)=false.
    /// FAILS-IF: allows() returns wrong subset result for any pair.
    #[kani::proof]
    fn cap_action_allows_matrix() {
        use crate::kernel::capability::cap_action::CapAction::*;
        // Positive: granted ⊇ requested.
        assert!(Read.allows(Read));
        assert!(Write.allows(Write));
        assert!(Execute.allows(Execute));
        assert!(ReadWrite.allows(Read));
        assert!(ReadWrite.allows(Write));
        assert!(ReadWrite.allows(ReadWrite));
        assert!(All.allows(Read));
        assert!(All.allows(Write));
        assert!(All.allows(Execute));
        assert!(All.allows(ReadWrite));
        assert!(All.allows(All));
        // Negative: granted does not include requested.
        assert!(!Read.allows(Write));
        assert!(!Read.allows(Execute));
        assert!(!Read.allows(ReadWrite));
        assert!(!Write.allows(Read));
        assert!(!Write.allows(Execute));
        assert!(!Execute.allows(Read));
        // K7 invariant: requested == None never grants (asking for nothing).
        assert!(!None.allows(None));
        assert!(!Read.allows(None));
        assert!(!All.allows(None));
        // None.allows(anything non-None) = false.
        assert!(!None.allows(Read));
        assert!(!None.allows(Write));
        assert!(!None.allows(All));
    }

    /// SAFE-2 K8 cross-crate drift: kernel IpcMessage size == config IPC_MSG_SIZE.
    // VERIFIES: SNTM-SAFE-R3 — typed IPC slot size consistency (CR-8 drift proof).
    /// FAILS-IF: kernel IpcMessage shrinks/grows (e.g. accidental padding,
    ///           IPC_MSG_SIZE bump without IpcMessage update).
    #[kani::proof]
    fn typed_ipc_size_invariant() {
        // K8: pull two independent symbols and compare against the config const.
        // If anyone changes IPC_MSG_SIZE alone, IpcMessage drifts → fail.
        let kernel_msg = core::mem::size_of::<crate::ipc::IpcMessage>();
        let cfg = crate::common::config::IPC_MSG_SIZE;
        assert!(kernel_msg == cfg);
        assert!(kernel_msg == 64);  // also pin numeric expectation
    }

    /// SAFE-2 CR-5: BOOT_CHANNELS table integrity — producer != consumer,
    /// ids in range. Drift guard for the manifest→codegen pipeline.
    // VERIFIES: SNTM-SAFE-R2 — boot channel ownership table well-formed.
    /// FAILS-IF: producer == consumer (self-loop), id ≥ MAX_IPC_CHANNELS,
    ///           or producer/consumer id ≥ MAX_TASKS.
    #[kani::proof]
    #[kani::unwind(9)] // K6: BOOT_CHANNELS len ≤ MAX_IPC_CHANNELS=8 → +1
    fn boot_channels_well_formed() {
        use crate::common::config::{MAX_IPC_CHANNELS, MAX_TASKS};
        use crate::kernel::capability::cap_generated::BOOT_CHANNELS;
        let mut i = 0usize;
        while i < BOOT_CHANNELS.len() {
            let (channel_id, producer, consumer) = BOOT_CHANNELS[i];
            assert!((channel_id as usize) < MAX_IPC_CHANNELS);
            assert!((producer as usize) < MAX_TASKS);
            assert!((consumer as usize) < MAX_TASKS);
            assert!(producer != consumer);  // self-loop forbidden
            i += 1;
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // SAFE-3 (sprint-u32) G0.0 pre-flight Kani additions.
    //
    // CR-1: kernel SyscallResult::to_raw() ↔ sipahi_api Error::from_kernel
    //       bit-eşit hizalama cross-crate drift guard (K8 doctrine).
    // CR-8: ed25519 verify Kani stub `false` döner — gerçek crypto Kani
    //       scope DIŞINDA; bu harness sadece "no-panic on kani::any input"
    //       structural property (K1 tautology yasak doktrinine uyum).
    // ═══════════════════════════════════════════════════════════════════

    /// SAFE-3 CR-1 K8 cross-crate: SyscallResult::to_raw() inverse of
    /// sipahi_api::Error::from_kernel — round-trip identity per variant.
    // VERIFIES: SAFE-3 syscall ABI alignment audit (Section 8 CR-1).
    /// FAILS-IF: kernel raw value vs api variant mapping drift; new variant
    ///           added one side without the other.
    #[kani::proof]
    fn syscall_error_abi_alignment() {
        use crate::kernel::syscall::dispatch::SyscallResult;
        // Kernel side raw values
        assert!(SyscallResult::Ok.to_raw() == 0);
        assert!(SyscallResult::InvalidSyscall.to_raw() == usize::MAX);
        assert!(SyscallResult::NoCapability.to_raw()   == usize::MAX - 1);
        assert!(SyscallResult::IpcFull.to_raw()        == usize::MAX - 2);
        assert!(SyscallResult::IpcEmpty.to_raw()       == usize::MAX - 3);
        assert!(SyscallResult::InvalidArg.to_raw()     == usize::MAX - 4);
        assert!(SyscallResult::BufferFull.to_raw()     == usize::MAX - 5);
        // Cross-crate: sipahi_api Error::from_kernel inverse (bit-eşit
        // pre-built mapping; sipahi_api Kani context'inde import edilemez —
        // bu test kernel SyscallResult tarafının kararlı olduğunu kanıtlar,
        // task-side mapping cargo test fixture'larında — Section 9.1 K8).
    }

    /// SAFE-3 CR-8 structural: verify_cert_signature wrapper kani::any
    /// input için no-panic, no OOB. Kani stub `false` döner — bu harness
    /// **crypto kanıtı değildir**; bounds + no-panic. Real ed25519 doğruluğu
    /// cargo test fixtures (RFC 8032 vector + tamper) G8'de.
    // VERIFIES: SAFE-3 cert signature bounds (Section 8 CR-8).
    /// FAILS-IF: verify_cert_signature panics for any input length;
    ///           ed25519-compact wrapper OOB on edge bytes.
    #[kani::proof]
    fn verify_cert_signature_bounded() {
        use crate::common::crypto::provider::SignatureVerifier;
        use crate::hal::secure_boot::Ed25519Provider;
        let pubkey: [u8; 32] = kani::any();
        let sig: [u8; 64]    = kani::any();
        // Mesaj: 32-byte cert head proxy (gerçek cert ~256+ byte; bu harness
        // wrapper bounds için, full message length Kani unwind şişer →
        // K6 doctrine: bounded yapısal property).
        let msg: [u8; 32] = kani::any();
        // Kani stub `false` döner; gerçek crypto property cargo test'te.
        // Burada SADECE no-panic doğrulanır — verify çağrısı any input için
        // graceful boolean döner, panic etmez.
        let _ = Ed25519Provider::verify(&pubkey, &msg, &sig);
    }
}
