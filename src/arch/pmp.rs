//! RISC-V PMP (Physical Memory Protection) register access and configuration.
// Sipahi — PMP (Physical Memory Protection) Register Access
// Sprint 5: RISC-V PMP CSR okuma/yazma
//
// RV64 PMP:
//   pmpcfg0  — pmpaddr0-7 config (8 entry × 8 bit = 64 bit)
//   pmpcfg2  — pmpaddr8-15 config
//   pmpaddr0-15 — bölge adresleri (adres >> 2)
//
// Config bit'leri (her entry 8 bit):
//   [0] R — read
//   [1] W — write
//   [2] X — execute
//   [4:3] A — address matching: 00=OFF, 01=TOR, 10=NA4, 11=NAPOT
//   [7] L — lock (M-mode dahil kilitler)
//
// TOR modu: bölge = [pmpaddr(i-1), pmpaddr(i))
// Sipahi TOR kullanır — R1=20KB, 2'nin kuvveti değil, NAPOT kullanılamaz

#[cfg(not(kani))]
use core::arch::asm;

// ═══════════════════════════════════════════════════════
// PMP Config sabitleri
// ═══════════════════════════════════════════════════════

pub const PMP_R: u8 = 1 << 0;   // Read
pub const PMP_W: u8 = 1 << 1;   // Write
pub const PMP_X: u8 = 1 << 2;   // Execute
pub const PMP_TOR: u8 = 1 << 3; // Top of Range mode
pub const PMP_L: u8 = 1 << 7;   // Lock (M-mode dahil)
#[allow(dead_code)]
pub const PMP_NAPOT: u8 = 0x18; // NAPOT mode — bit [4:3] = 11

/// Per-task stack NAPOT config: R+W, X=0 (W^X), L=0, NAPOT
/// 0b00011011 = 0x1B
pub const PMP_NAPOT_RW: usize = 0x1B;

/// 8KB NAPOT mask: (size >> 3) - 1 = (8192 >> 3) - 1 = 0x3FF
pub const PMP_NAPOT_MASK_8KB: usize = 0x3FF;

// ═══════════════════════════════════════════════════════
// PMP Encoding Type (U-24 SNTM Phase 2 — design v0.8 §4.5.4)
// ═══════════════════════════════════════════════════════

/// PMP encoding türü — NAPOT (1 entry) veya TOR çifti (2 entry).
/// SNTM design v0.5 §4.5.1 packing algorithm. Manifest'ten sntm-validate
/// üretir (Phase 4); kernel build-time const PMP_PROFILES tüketir.
///
/// U-24 SNTM Phase 2: type definition + arch::pmp layer'da kalır,
/// kernel/pmp/profile.rs `use crate::arch::pmp::PmpEncoding` ile import eder.
#[allow(dead_code)] // U-24: placeholder profile EMPTY, U-25 runtime tüketimi
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PmpEncoding {
    /// NAPOT — tek entry, size power-of-2, base size-aligned
    Napot { addr: usize, size_log2: u8 },
    /// TOR — iki entry (lo=OFF, hi=TOR), NAPOT-uyumsuz layout'lar için
    Tor { lo: usize, hi: usize },
}

// ═══════════════════════════════════════════════════════
// PMP Address Register Yazma (pmpaddr0-7)
// ═══════════════════════════════════════════════════════

/// pmpaddr register'ına yaz
/// addr: fiziksel adres (fonksiyon >> 2 yaparak yazar)
/// U-25 FIX-2: indices 8..15 boot-time zero için açıldı (verify_pmp_integrity
/// multi-region check için defansif initial state).
#[cfg(not(kani))]
pub fn write_pmpaddr(index: usize, addr: usize) {
    let shifted = addr >> 2; // PMP adresi 4-byte granülarite
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe {
        match index {
            0 => asm!("csrw pmpaddr0, {}", in(reg) shifted),
            1 => asm!("csrw pmpaddr1, {}", in(reg) shifted),
            2 => asm!("csrw pmpaddr2, {}", in(reg) shifted),
            3 => asm!("csrw pmpaddr3, {}", in(reg) shifted),
            4 => asm!("csrw pmpaddr4, {}", in(reg) shifted),
            5 => asm!("csrw pmpaddr5, {}", in(reg) shifted),
            6 => asm!("csrw pmpaddr6, {}", in(reg) shifted),
            7 => asm!("csrw pmpaddr7, {}", in(reg) shifted),
            8 => asm!("csrw pmpaddr8, {}", in(reg) shifted),
            9 => asm!("csrw pmpaddr9, {}", in(reg) shifted),
            10 => asm!("csrw pmpaddr10, {}", in(reg) shifted),
            11 => asm!("csrw pmpaddr11, {}", in(reg) shifted),
            12 => asm!("csrw pmpaddr12, {}", in(reg) shifted),
            13 => asm!("csrw pmpaddr13, {}", in(reg) shifted),
            14 => asm!("csrw pmpaddr14, {}", in(reg) shifted),
            15 => asm!("csrw pmpaddr15, {}", in(reg) shifted),
            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════
// PMP Config Register Yazma (pmpcfg0)
// ═══════════════════════════════════════════════════════

/// pmpcfg0 yaz — pmpaddr0-7 config'lerini ayarlar
/// RV64'te pmpcfg0 = 64 bit, 8 entry × 8 bit
#[cfg(not(kani))]
pub fn write_pmpcfg0(value: u64) {
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe {
        asm!("csrw pmpcfg0, {}", in(reg) value);
    }
}

/// pmpcfg0 oku
#[cfg(not(kani))]
pub fn read_pmpcfg0() -> u64 {
    let val: u64;
    // SAFETY: CSR read/write in M-mode — always accessible.
    unsafe {
        asm!("csrr {}, pmpcfg0", out(reg) val);
    }
    val
}

/// pmpcfg2 oku (task PMP, entry 8-15)
#[cfg(not(kani))]
pub fn read_pmpcfg2() -> usize {
    let val: usize;
    // SAFETY: CSR read in M-mode — always accessible.
    unsafe { asm!("csrr {}, pmpcfg2", out(reg) val); }
    val
}

/// pmpaddr8 oku (task stack NAPOT)
/// U-25: read_pmpaddr(8) ile değiştirilebilir; geriye uyumluluk için tutuldu.
#[cfg(not(kani))]
#[allow(dead_code)] // U-25: read_pmpaddr(8) ile değiştirildi, geriye uyumluluk
pub fn read_pmpaddr8() -> usize {
    let val: usize;
    // SAFETY: CSR read in M-mode — always accessible.
    unsafe { asm!("csrr {}, pmpaddr8", out(reg) val); }
    val
}

/// pmpaddr register'ını oku (0-15)
/// U-25 FIX-2: indices 8..15 verify_pmp_integrity multi-region için açıldı.
#[cfg(not(kani))]
pub fn read_pmpaddr(index: usize) -> usize {
    let val: usize;
    // SAFETY: CSR read in M-mode — always accessible.
    unsafe {
        match index {
            0 => asm!("csrr {}, pmpaddr0", out(reg) val),
            1 => asm!("csrr {}, pmpaddr1", out(reg) val),
            2 => asm!("csrr {}, pmpaddr2", out(reg) val),
            3 => asm!("csrr {}, pmpaddr3", out(reg) val),
            4 => asm!("csrr {}, pmpaddr4", out(reg) val),
            5 => asm!("csrr {}, pmpaddr5", out(reg) val),
            6 => asm!("csrr {}, pmpaddr6", out(reg) val),
            7 => asm!("csrr {}, pmpaddr7", out(reg) val),
            8 => asm!("csrr {}, pmpaddr8",  out(reg) val),
            9 => asm!("csrr {}, pmpaddr9",  out(reg) val),
            10 => asm!("csrr {}, pmpaddr10", out(reg) val),
            11 => asm!("csrr {}, pmpaddr11", out(reg) val),
            12 => asm!("csrr {}, pmpaddr12", out(reg) val),
            13 => asm!("csrr {}, pmpaddr13", out(reg) val),
            14 => asm!("csrr {}, pmpaddr14", out(reg) val),
            15 => asm!("csrr {}, pmpaddr15", out(reg) val),
            _ => { val = 0; }
        }
    }
    val
}

// ═══════════════════════════════════════════════════════
// Per-task PMP NAPOT reprogramlama
// ═══════════════════════════════════════════════════════

/// Per-task PMP NAPOT reprogramlama — deny-by-default sıra
/// 1. pmpcfg2 = 0 (eski config temizle)
/// 2. pmpaddr8 = NAPOT encoded adres
/// 3. pmpcfg2 = config (NAPOT RW)
/// 4. sfence.vma — PMP CSR ordering barrier (U-22.5 G5)
///
/// Caller: scheduler context switch + start_first_task
/// Interrupt: trap context'inde çağrılır, MIE=0
///
/// U-22.5 G5: SFENCE.VMA spec compliance (RISC-V Privileged Spec §3.7.2).
/// PMP CSR write'lar memory pipeline'daki erişimlerle sıralı değil;
/// CVA6 ve gerçek RISC-V çekirdeklerde speculative execution + memory
/// pipeline var → fence olmadan U-mode geri dönüşte eski PMP değerleri
/// kısa pencerede geçerli kalabilir → izolasyon ihlali.
///
/// QEMU TCG modu PMP fence enforce etmiyor (sessiz geçer), production
/// silikonda BUG. v1.1.1 patch'i SNTM'den bağımsız.
#[cfg(not(kani))]
pub fn write_per_task_napot(napot_addr: usize, cfg_val: usize) {
    // SAFETY: CSR write in M-mode, interrupt disabled (trap context).
    // sfence.vma zero, zero: tüm address translation cache flush
    // (PMP + virtual memory aynı barrier). M-mode + no paging → minimal etki,
    // sadece sıralama garantisi.
    unsafe {
        asm!("csrw pmpcfg2, zero");
        asm!("csrw pmpaddr8, {}", in(reg) napot_addr);
        asm!("csrw pmpcfg2, {}", in(reg) cfg_val);
        // U-22.5 G5: SFENCE.VMA zorunlu (PMP CSR ordering barrier)
        asm!("sfence.vma zero, zero");
    }
}

// ═══════════════════════════════════════════════════════
// Yardımcı: Config byte oluştur
// ═══════════════════════════════════════════════════════

/// 8 entry'lik config'i tek u64'e birleştir
pub const fn pack_pmpcfg(configs: [u8; 8]) -> u64 {
    let mut result: u64 = 0;
    let mut i = 0;
    while i < 8 {
        result |= (configs[i] as u64) << (i * 8);
        i += 1;
    }
    result
}

// ═══════════════════════════════════════════════════════
// U-25 SNTM Phase 3 — Multi-region PMP profile reload (§4.5.3)
// ═══════════════════════════════════════════════════════

/// U-25 SNTM-R6: Reload sırasında hangi pmpcfg indekslerine yazılacak —
/// Kani-friendly pure model. Real impl write yapmadan SADECE planı döner.
///
/// FIX-1: kernel (0..5) + UART (6,7) lock'lu — DAİMA `>= PMP_DYNAMIC_START_ENTRY=8`.
/// FIX-5: heapless dep YASAK → ([u8; 12], usize) sabit array + count tuple.
///
/// Return: (indices, count). count ≤ 12 (worst-case 6 region × TOR 2-entry).
#[must_use]
#[allow(dead_code)] // U-25 G8: Kani harness + G11 scheduler hook tüketir
pub fn reload_indices_touched(
    profile: &crate::kernel::pmp::profile::PmpProfile,
) -> ([u8; 12], usize) {
    use crate::common::config::{PMP_DYNAMIC_START_ENTRY, MAX_PMP_ENTRIES};

    let mut out = [0u8; 12];
    let mut count: usize = 0;
    let mut entry = PMP_DYNAMIC_START_ENTRY;
    let active = profile.active_regions();
    let mut i = 0;
    while i < active.len() && entry < MAX_PMP_ENTRIES && count < 12 {
        match active[i].encoding {
            PmpEncoding::Napot { .. } => {
                out[count] = entry;
                count += 1;
                entry += 1;
            }
            PmpEncoding::Tor { .. } => {
                out[count] = entry;
                count += 1;
                entry += 1;
                if entry < MAX_PMP_ENTRIES && count < 12 {
                    out[count] = entry;
                    count += 1;
                    entry += 1;
                }
            }
        }
        i += 1;
    }
    (out, count)
}

/// Permission → pmpcfg byte (R|W|X bit'leri). U-25 G8 helper.
#[inline]
#[allow(dead_code)] // U-25 G8: reload_pmp_profile tüketir, G11 scheduler hook ile aktif
const fn perm_to_cfg(p: crate::kernel::pmp::profile::Permission) -> u8 {
    (if p.r { PMP_R } else { 0 })
        | (if p.w { PMP_W } else { 0 })
        | (if p.x { PMP_X } else { 0 })
}

/// Multi-region PMP profile reload. SNTM design v0.8 §4.5.3 + U-25 FIX-1 + FIX-2.
///
/// SAFETY:
///   - Trap context / kernel boot — MIE=0, M-mode, single hart.
///   - Kernel + UART entry'leri (0..7) lock'lu → ASLA overwrite (FIX-1).
///   - Sadece pmpcfg2 + pmpaddr8..15 yazılır.
///   - DENY stage atomicity garantili (single actor, M-mode, MIE=0).
///   - Shadow update (FIX-2) sfence sonrası ZORUNLU — sonraki tick'in
///     verify_pmp_integrity'sini geçmek için.
#[cfg(not(kani))]
#[allow(dead_code)] // U-25 G8: G11 scheduler hook is_sntm_native=true ile çağırır
pub unsafe fn reload_pmp_profile(profile: &crate::kernel::pmp::profile::PmpProfile) {
    use crate::common::config::{PMP_DYNAMIC_START_ENTRY, MAX_PMP_ENTRIES};

    // Stage 1: DENY — pmpcfg2 = 0 (entry 8..15 hepsi OFF).
    // FIX-1: pmpcfg0 (entry 0..7) ASLA dokunulmaz.
    // SAFETY: M-mode CSR write — caller invariant MIE=0.
    unsafe { asm!("csrw pmpcfg2, zero"); }

    // Stage 2: Yeni profile'i sıralı yaz (entry 8'den başla).
    let active = profile.active_regions();
    let mut entry: u8 = PMP_DYNAMIC_START_ENTRY;
    let mut new_addrs: [usize; 8] = [0; 8];
    let mut new_cfg2: u64 = 0;
    let mut i = 0;
    while i < active.len() && entry < MAX_PMP_ENTRIES {
        let r = &active[i];
        match r.encoding {
            PmpEncoding::Napot { addr, size_log2: _ } => {
                // SAFETY: entry in 8..16 range — write_pmpaddr_dyn debug_assert ile guard.
                unsafe { write_pmpaddr_dyn(entry, addr); }
                let cfg_byte = perm_to_cfg(r.perm) | PMP_NAPOT;
                accumulate_cfg2(&mut new_cfg2, entry, cfg_byte);
                new_addrs[(entry - PMP_DYNAMIC_START_ENTRY) as usize] = addr;
                entry += 1;
            }
            PmpEncoding::Tor { lo, hi } => {
                let lo_enc = lo >> 2;
                // SAFETY: entry in 8..16 range.
                unsafe { write_pmpaddr_dyn(entry, lo_enc); }
                accumulate_cfg2(&mut new_cfg2, entry, 0);  // OFF (TOR base)
                new_addrs[(entry - PMP_DYNAMIC_START_ENTRY) as usize] = lo_enc;
                entry += 1;
                if entry < MAX_PMP_ENTRIES {
                    let hi_enc = hi >> 2;
                    // SAFETY: entry in 8..16 range.
                    unsafe { write_pmpaddr_dyn(entry, hi_enc); }
                    let cfg_byte = perm_to_cfg(r.perm) | PMP_TOR;
                    accumulate_cfg2(&mut new_cfg2, entry, cfg_byte);
                    new_addrs[(entry - PMP_DYNAMIC_START_ENTRY) as usize] = hi_enc;
                    entry += 1;
                }
            }
        }
        i += 1;
    }

    // Stage 3: pmpcfg2 toplu yaz (8 byte = 8 entry config, tek atomic write).
    // SAFETY: M-mode CSR write.
    unsafe { asm!("csrw pmpcfg2, {}", in(reg) new_cfg2); }

    // Stage 4: SFENCE.VMA — RISC-V Priv Spec §3.7.2 PMP ordering.
    // SAFETY: M-mode fence instruction.
    unsafe { asm!("sfence.vma zero, zero"); }

    // Stage 5 (FIX-2): Shadow update — verify_pmp_integrity için.
    crate::kernel::memory::update_dynamic_pmp_shadow(&new_addrs, new_cfg2 as usize);
}

/// U-25 G8 helper: pmpcfg2 byte accumulator (entry idx → byte position).
#[inline]
#[allow(dead_code)] // U-25 G8: reload_pmp_profile tüketir
fn accumulate_cfg2(cfg2: &mut u64, idx: u8, byte: u8) {
    use crate::common::config::{PMP_DYNAMIC_START_ENTRY, MAX_PMP_ENTRIES};
    debug_assert!((PMP_DYNAMIC_START_ENTRY..MAX_PMP_ENTRIES).contains(&idx));
    let shift = ((idx - PMP_DYNAMIC_START_ENTRY) as u64) * 8;
    *cfg2 |= (byte as u64) << shift;
}

/// U-25 G8 helper: pmpaddr8..15 writer — FIX-1: entry < 8 ASLA match etmez.
/// SAFETY: Caller debug_assert ile entry range'i doğrular; out-of-range no-op.
#[cfg(not(kani))]
#[allow(dead_code)] // U-25 G8: reload_pmp_profile tüketir
unsafe fn write_pmpaddr_dyn(idx: u8, val: usize) {
    use crate::common::config::{PMP_DYNAMIC_START_ENTRY, MAX_PMP_ENTRIES};
    debug_assert!((PMP_DYNAMIC_START_ENTRY..MAX_PMP_ENTRIES).contains(&idx));
    // SAFETY: M-mode CSR write, caller MIE=0.
    unsafe {
        match idx {
            8  => asm!("csrw pmpaddr8,  {}", in(reg) val),
            9  => asm!("csrw pmpaddr9,  {}", in(reg) val),
            10 => asm!("csrw pmpaddr10, {}", in(reg) val),
            11 => asm!("csrw pmpaddr11, {}", in(reg) val),
            12 => asm!("csrw pmpaddr12, {}", in(reg) val),
            13 => asm!("csrw pmpaddr13, {}", in(reg) val),
            14 => asm!("csrw pmpaddr14, {}", in(reg) val),
            15 => asm!("csrw pmpaddr15, {}", in(reg) val),
            _ => {
                // FIX-1 defansif: kernel/UART range — no-op.
                debug_assert!(false, "write_pmpaddr_dyn: idx out of dynamic range");
            }
        }
    }
}
