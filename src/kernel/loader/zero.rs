//! Region zero-fill — bss + uninitialized data clearing.
//!
//! VERIFIES: SNTM-R9 loader_zero_fill_complete (Kani-proven).
//! SAFETY/CORRECTNESS: All bytes in [0, len) set to 0.

#[inline]
pub fn zero_fill(buf: &mut [u8]) {
    let mut i = 0;
    while i < buf.len() {
        buf[i] = 0;
        i += 1;
    }
}
