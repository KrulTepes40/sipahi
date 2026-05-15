//! Per-task PMP profile types + pure validation helpers.
//!
//! U-24 SNTM Phase 2: PmpProfile + Region high-level abstraction.
//! Encoding type arch::pmp::PmpEncoding'de (HW-level).
//! Manifest-driven build-time const tables; runtime integration (context
//! switch reload) Phase 3 (Sprint U-25).

#![allow(dead_code)] // U-24 sırasında bazıları henüz hot path'te kullanılmıyor

pub mod profile;
pub mod overlap;
