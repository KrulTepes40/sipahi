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

/// U-25 SNTM-R7: User pointer doğrulamasında talep edilen erişim türü.
/// SNTM design v0.8 §5.2 — `ipc_send` Read, `ipc_recv` Write, future
/// inline-code execute Read+Execute, vb.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Access {
    Read    = 0,
    Write   = 1,
    Execute = 2,
}

impl Access {
    /// Talep edilen erişim Permission flag'leriyle uyumlu mu.
    /// SAFETY/CORRECTNESS: Pure function — branch tablosu, side-effect yok.
    #[inline]
    #[must_use]
    pub const fn matches(self, perm: Permission) -> bool {
        match self {
            Access::Read    => perm.r,
            Access::Write   => perm.w,
            Access::Execute => perm.x,
        }
    }
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

/// Caller task ID'ye göre PMP profile lookup.
///
/// U-24: placeholder EMPTY array. U-25 SNTM Phase 3: `generated.rs`
/// sntm-validate codegen output'undan okur (manifest-driven).
/// Drift detection CI gate'i `git diff src/kernel/pmp/generated.rs` ile.
#[inline]
#[must_use = "PMP profile lookup result must be checked"]
pub fn get_pmp_profile(task_id: u8) -> Option<&'static PmpProfile> {
    let idx = task_id as usize;
    if idx >= MAX_TASKS {
        return None;
    }
    Some(&crate::kernel::pmp::generated::PMP_PROFILES[idx])
}
