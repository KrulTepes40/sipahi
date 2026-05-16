//! NAPOT encoding helper — kernel pmp.rs ile DUPLICATE pure logic (host-side).
//!
//! SNTM design v0.5 §4.5.1 packing algorithm.
//! Size'dan size_log2 türetme + base validation (size-aligned).

/// (base, size) → Some(size_log2) eğer NAPOT-uyumlu, None değilse.
/// Size power-of-2 ≥ 8 byte AND base size-aligned. size_log2 = log2(size).
#[must_use]
pub fn napot_size_log2(base: u64, size: u64) -> Option<u8> {
    if size < 8 { return None; }
    if size & (size - 1) != 0 { return None; }
    if base & (size - 1) != 0 { return None; }
    Some(size.ilog2() as u8)
}

/// NAPOT pmpaddr encoding: (base >> 2) | ((1 << (size_log2 - 3)) - 1).
/// SNTM design §4.5.3 reload sequence + RISC-V Priv Spec §3.7.1.
#[must_use]
pub fn napot_pmpaddr(base: u64, size_log2: u8) -> u64 {
    // SAFETY/CORRECTNESS: size_log2 ≥ 3 (size ≥ 8 byte = 2^3) — caller
    // napot_size_log2 ile doğrulamış olmalı; debug_assert ekstra guard.
    debug_assert!(size_log2 >= 3 && size_log2 <= 63);
    let mask = (1u64 << (size_log2 - 3)) - 1;
    (base >> 2) | mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn napot_8byte_min() {
        assert_eq!(napot_size_log2(0x8010_0000, 8), Some(3));
    }

    #[test]
    fn napot_16k_aligned() {
        // 0x4000 = 16K = 2^14
        assert_eq!(napot_size_log2(0x8010_0000, 0x4000), Some(14));
    }

    #[test]
    fn napot_non_pow2_reject() {
        assert_eq!(napot_size_log2(0x8010_0000, 6 * 1024), None);
        assert_eq!(napot_size_log2(0x8010_0000, 0x3000), None);  // 12K
    }

    #[test]
    fn napot_unaligned_reject() {
        // 0x4000 NAPOT needs base aligned to 0x4000; 0x80100001 is off by 1.
        assert_eq!(napot_size_log2(0x8010_0001, 0x4000), None);
        // 0x10000 (64K) needs base 0x10000-aligned; 0x80108000 is 0x8000-aligned.
        assert_eq!(napot_size_log2(0x8010_8000, 0x1_0000), None);
    }

    #[test]
    fn napot_size_too_small() {
        for s in [0u64, 1, 2, 3, 4, 5, 6, 7] {
            assert_eq!(napot_size_log2(0x8010_0000, s), None, "size={}", s);
        }
    }

    #[test]
    fn napot_8k_pmpaddr_bits() {
        // 8KB @ 0x80100000:
        // size_log2=13, mask = (1 << 10) - 1 = 0x3FF
        // base >> 2 = 0x80100000 >> 2 = 0x20040000
        // encoded = 0x20040000 | 0x3FF = 0x200403FF
        let enc = napot_pmpaddr(0x8010_0000, 13);
        assert_eq!(enc, 0x2004_03FF);
    }

    #[test]
    fn napot_16k_pmpaddr_bits() {
        // 16KB @ 0x80100000:
        // size_log2=14, mask = (1 << 11) - 1 = 0x7FF
        // encoded = 0x20040000 | 0x7FF = 0x200407FF
        let enc = napot_pmpaddr(0x8010_0000, 14);
        assert_eq!(enc, 0x2004_07FF);
    }

    #[test]
    fn napot_round_trip_decode() {
        // Encoded → trailing_ones = size_log2 - 3 (exact, base size-aligned)
        for (base, size, expected_log2) in [
            (0x8010_0000u64, 8u64,       3u8),
            (0x8010_0000,    0x10,       4),
            (0x8010_0000,    0x4000,     14),
            (0x8010_0000,    0x1_0000,   16),
        ] {
            let log2 = napot_size_log2(base, size).unwrap();
            assert_eq!(log2, expected_log2);
            let enc = napot_pmpaddr(base, log2);
            // Mask exact bit count:
            assert_eq!(enc.trailing_ones() as u8, log2 - 3);
            // Decoded base bit-equal:
            let mask = (1u64 << (log2 - 3)) - 1;
            let decoded = (enc & !mask) << 2;
            assert_eq!(decoded, base);
        }
    }
}
