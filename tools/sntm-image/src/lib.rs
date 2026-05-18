//! Sipahi SNTM-SAFE image assembler — public API.
//!
//! Section 8 CR-7 doctrine: signed runtime artifact drift guard'a sokulmaz;
//! sign+verify roundtrip [10/10] gate. Image artifact ephemeral (.gitignored).

pub mod format;
pub mod sign;
