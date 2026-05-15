//! Pure region overlap + NAPOT alignment helpers.
//!
//! U-24 SNTM Phase 2: build-time + runtime invariant kontrolleri.
//! Kani proof'lar (verify.rs SNTM-R3, SNTM-R5) bu helper'ları doğrular
//! (symbolic input). Kernel self-test'ler (tests/mod.rs) table-driven
//! basic semantics test eder. sntm-validate tool (G9) DUPLICATE pure
//! fn implement eder (no_std vs std crossing, U-24 pragmatik karar).

/// İki region adres aralığı kesişiyor mu (half-open [base, base+size)).
///
/// SAFETY/CORRECTNESS:
///   - saturating_add overflow yok (cosmic ray + bug-late-injection defansif)
///   - Symmetric: regions_overlap(a, b) == regions_overlap(b, a) (SNTM-R3)
///   - Empty region (size=0): asla overlap (false)
#[inline]
#[must_use]
pub const fn regions_overlap(
    a_base: usize, a_size: usize,
    b_base: usize, b_size: usize,
) -> bool {
    if a_size == 0 || b_size == 0 {
        return false;
    }
    let a_end = a_base.saturating_add(a_size);
    let b_end = b_base.saturating_add(b_size);
    !(a_end <= b_base || b_end <= a_base)
}

/// NAPOT-uyumlu mu: size power-of-2 ≥ 8 byte VE base aligned to size.
/// SNTM design v0.5 §4.5.1 NAPOT decision tree.
#[inline]
#[must_use]
pub const fn valid_napot_alignment(base: usize, size: usize) -> bool {
    if size < 8 {
        return false;
    }
    // power-of-2 check: size & (size-1) == 0
    if size & (size - 1) != 0 {
        return false;
    }
    // base aligned to size
    base & (size - 1) == 0
}
