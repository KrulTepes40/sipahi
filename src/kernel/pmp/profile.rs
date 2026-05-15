//! PMP profile types — manifest-generated, build-time const.
//!
//! Layer separation (SNTM design v0.8 §4.5.4):
//!   src/arch/pmp.rs       = RISC-V CSR low-level + PmpEncoding type
//!   src/kernel/pmp/*.rs   = High-level abstraction (PmpProfile, Region) + helpers
//!
//! v1.5 PMP_PROFILES const'u sntm-validate tarafından üretilir (Phase 4).
//! Şu an PLACEHOLDER empty array — Sprint U-25 runtime integration ekler.

use crate::arch::pmp::PmpEncoding;
use crate::common::config::MAX_TASKS;

/// PMP region permissions (RWX bitleri).
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Permission {
    pub r: bool,
    pub w: bool,
    pub x: bool,
}

impl Permission {
    pub const RX:   Self = Self { r: true,  w: false, x: true  };
    pub const R:    Self = Self { r: true,  w: false, x: false };
    pub const RW:   Self = Self { r: true,  w: true,  x: false };
    pub const NONE: Self = Self { r: false, w: false, x: false };
}

/// Tek region — task'a grant edilen tek PMP entry (NAPOT) veya
/// entry-çifti (TOR).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Region {
    pub base:     usize,
    pub size:     usize,
    pub encoding: PmpEncoding,
    pub perm:     Permission,
}

/// Task'ın tam PMP profili — max 6 region (text/rodata/data/stack/mmio/guard).
/// region_count actual sayı, regions[0..region_count] valid.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PmpProfile {
    pub region_count: u8,
    pub regions:      [Region; 6],
}

impl PmpProfile {
    /// Boş profile (placeholder) — region_count=0.
    pub const EMPTY: Self = Self {
        region_count: 0,
        regions: [Region {
            base: 0, size: 0,
            encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 },
            perm: Permission::NONE,
        }; 6],
    };

    /// Valid region slice (0..region_count).
    #[inline]
    pub fn active_regions(&self) -> &[Region] {
        let count = (self.region_count as usize).min(6);
        &self.regions[..count]
    }
}

/// Build-time const — Sprint U-24 placeholder, Sprint U-25 sntm-validate generate.
pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] =
    [PmpProfile::EMPTY; MAX_TASKS];

/// Caller task ID'ye göre PMP profile lookup.
///
/// U-24 placeholder: tüm task'lar EMPTY profile. U-25'te runtime reload
/// + Phase 4 manifest-generated tablo aktif olur.
#[inline]
#[must_use = "PMP profile lookup result must be checked"]
pub fn get_pmp_profile(task_id: u8) -> Option<&'static PmpProfile> {
    let idx = task_id as usize;
    if idx >= MAX_TASKS {
        return None;
    }
    Some(&PMP_PROFILES[idx])
}
