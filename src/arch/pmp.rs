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

// ═══════════════════════════════════════════════════════
// PMP Address Register Yazma (pmpaddr0-7)
// ═══════════════════════════════════════════════════════

/// pmpaddr register'ına yaz
/// addr: fiziksel adres (fonksiyon >> 2 yaparak yazar)
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
            _ => {} // 8-15 Sprint 5'te kullanılmıyor
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
#[cfg(not(kani))]
pub fn read_pmpaddr8() -> usize {
    let val: usize;
    // SAFETY: CSR read in M-mode — always accessible.
    unsafe { asm!("csrr {}, pmpaddr8", out(reg) val); }
    val
}

/// pmpaddr register'ını oku (0-7)
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
            _ => { val = 0; }
        }
    }
    val
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
