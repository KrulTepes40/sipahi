//! Bounded src → dst copy — atomic (no partial on overflow).
//!
//! VERIFIES: SNTM-R9 loader_bounded_copy_atomic (Kani-proven).
//! SAFETY/CORRECTNESS:
//!   - src.len() > dst.len() → no write (atomic deny)
//!   - src.len() ≤ dst.len() → src[..] copied verbatim, dst[src.len()..] untouched

#[derive(Debug, PartialEq, Eq)]
pub struct BoundedCopyError;

#[inline]
pub fn bounded_copy(src: &[u8], dst: &mut [u8]) -> Result<(), BoundedCopyError> {
    if src.len() > dst.len() {
        return Err(BoundedCopyError);
    }
    let mut i = 0;
    while i < src.len() {
        dst[i] = src[i];
        i += 1;
    }
    Ok(())
}
