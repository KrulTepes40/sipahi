//! IOPMP (I/O Physical Memory Protection) stub — software emulation for v1.0.
#![allow(dead_code)] // Stub — activated when iopmp feature + real hardware.
// Sipahi — IOPMP Stub (Sprint 6)
//
// IOPMP = I/O Physical Memory Protection
// DMA aygıtlarının belleğe erişimini kısıtlar.
//
// v1.0: STUB — CVA6'da IOPMP yok, QEMU virt'te de yok.
// Gerçek IOPMP desteği v2.0 (FPGA + CVA6-IOPMP).
//
// Bu modül `iopmp` feature flag'i ile aktifleşir.
// Feature kapalıysa derlenmez bile.
//
// Stub amacı:
// 1. API'yi şimdiden tanımla — Sprint 12'de WASM DMA koruması için hazır
// 2. Kani proof'ları ile API sözleşmesini doğrula
// 3. Gerçek implementasyon geldiğinde sadece iç kısım değişir

use crate::common::error::SipahiError;

/// IOPMP bölge konfigürasyonu
#[derive(Clone, Copy)]
pub struct IopmpRegion {
    /// Bölge başlangıç adresi
    pub base: usize,
    /// Bölge boyutu (byte)
    pub size: usize,
    /// Okuma izni
    pub read: bool,
    /// Yazma izni
    pub write: bool,
}

impl IopmpRegion {
    pub const fn new(base: usize, size: usize, read: bool, write: bool) -> Self {
        IopmpRegion { base, size, read, write }
    }
}

/// Maksimum IOPMP bölge sayısı
pub const IOPMP_MAX_REGIONS: usize = 4;

/// IOPMP controller stub
pub struct IopmpController {
    regions: [Option<IopmpRegion>; IOPMP_MAX_REGIONS],
    enabled: bool,
}

impl IopmpController {
    pub const fn new() -> Self {
        IopmpController {
            regions: [None; IOPMP_MAX_REGIONS],
            enabled: false,
        }
    }

    /// IOPMP'yi etkinleştir (STUB: sadece flag set eder)
    pub fn enable(&mut self) -> Result<(), SipahiError> {
        self.enabled = true;
        Ok(())
    }

    /// Bölge ekle
    pub fn add_region(&mut self, index: usize, region: IopmpRegion) -> Result<(), SipahiError> {
        if index >= IOPMP_MAX_REGIONS {
            return Err(SipahiError::InvalidParameter);
        }
        self.regions[index] = Some(region);
        Ok(())
    }

    /// Bölge sil
    pub fn remove_region(&mut self, index: usize) -> Result<(), SipahiError> {
        if index >= IOPMP_MAX_REGIONS {
            return Err(SipahiError::InvalidParameter);
        }
        self.regions[index] = None;
        Ok(())
    }

    /// Erişim kontrolü (STUB: bölge tanımlıysa izin ver)
    pub fn check_access(&self, addr: usize, size: usize, write: bool) -> bool {
        if !self.enabled {
            return true; // IOPMP kapalı → tüm erişim serbest
        }
        let mut i = 0;
        while i < IOPMP_MAX_REGIONS {
            if let Some(region) = &self.regions[i] {
                // Overflow koruması — safety-critical'da zorunlu
                let end = match region.base.checked_add(region.size) {
                    Some(e) => e,
                    None => { i += 1; continue; } // overflow → bu bölgeyi atla
                };
                let access_end = match addr.checked_add(size) {
                    Some(e) => e,
                    None => return false, // erişim adresi overflow → RED
                };
                // Bölge içinde mi?
                if addr >= region.base && access_end <= end {
                    if write {
                        return region.write;
                    }
                    return region.read;
                }
            }
            i += 1;
        }
        false // Tanımsız bölge → erişim RED
    }

    /// IOPMP etkin mi?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
