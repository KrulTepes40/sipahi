//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml (run `bash scripts/regen_pmp_profiles.sh` or
//! `make regen-pmp` to regenerate).
//! U-25 SNTM Phase 3 codegen — sntm-validate --output-rs output.
//!
//! Drift detection: CI runs sntm-validate again + git diff.

use crate::arch::pmp::PmpEncoding;
use crate::kernel::pmp::profile::{Permission, PmpProfile, Region};

pub static PMP_PROFILES: [PmpProfile; 8] = [
    // Task 0 (task_hello)
    PmpProfile {
        region_count: 4,
        regions: [
            Region { base: 0x80100000, size: 0x4000, encoding: PmpEncoding::Napot { addr: 0x200407ff, size_log2: 14 }, perm: Permission::RX },
            Region { base: 0x80104000, size: 0x1000, encoding: PmpEncoding::Napot { addr: 0x200411ff, size_log2: 12 }, perm: Permission::R },
            Region { base: 0x80105000, size: 0x1000, encoding: PmpEncoding::Napot { addr: 0x200415ff, size_log2: 12 }, perm: Permission::RW },
            Region { base: 0x80110000, size: 0x2000, encoding: PmpEncoding::Napot { addr: 0x200443ff, size_log2: 13 }, perm: Permission::RW },
            Region { base: 0, size: 0, encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 }, perm: Permission::NONE },
            Region { base: 0, size: 0, encoding: PmpEncoding::Napot { addr: 0, size_log2: 0 }, perm: Permission::NONE },
        ],
    },
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
    PmpProfile::EMPTY,
];
