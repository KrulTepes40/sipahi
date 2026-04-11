//! Ed25519 key provisioning — OTP fuse (production) / compile-time (QEMU).
#![allow(dead_code)]
// Sipahi — HAL Key Provisioning (Sprint 13)
// Doküman §SECURE_BOOT §KEY_PROVISIONING:
//
//   Key hiyerarşisi:
//     Root key:   OTP fuse'da (değiştirilemez, cihaz ömrü)
//                 QEMU v1.0: compile-time sabit test anahtarı
//     Module key: .rodata'da (root key ile imzalı, güncellenebilir)
//
//   Factory provisioning süreci (production v2.0):
//     1. HSM içinde Ed25519 key pair üret
//     2. Public key → OTP fuse'a yaz (bir kere, geri alınamaz)
//     3. Private key → HSM'de kalır (cihaza YAZILMAZ)
//     4. JTAG fuse yak → debug port kapat
//
//   v1.0 basitleştirme: QEMU'da OTP yok → key compile-time sabit
//   FPGA v2.0: OTP emülasyonu → fuse benzeri davranış
//
// Kani Proof 64: OTP_KEY_SIZE == 32 (Ed25519 public key boyutu)
// Kani Proof 65: get_root_public_key pointer bounded — her byte erişilebilir

/// Ed25519 public key boyutu (byte) — RFC 8032 tanımı
pub const OTP_KEY_SIZE: usize = 32;

/// Ed25519 imza boyutu (byte) — R (32B) + S (32B)
pub const SIGNATURE_SIZE: usize = 64;

// Compile-time guarantees — Ed25519 spec
const _: () = assert!(OTP_KEY_SIZE == 32);
const _: () = assert!(SIGNATURE_SIZE == 2 * OTP_KEY_SIZE);

/// QEMU geliştirme ortamı test public key — RFC 8032 Test Vector #1
///
/// Production'da OTP fuse'dan okunur; bu sabit SADECE QEMU/test ortamı içindir.
/// Release build'de test-keys feature olmadan derlenmez.
#[cfg(feature = "test-keys")]
pub const QEMU_TEST_PUBKEY: [u8; OTP_KEY_SIZE] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
    0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
    0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// QEMU geliştirme ortamı test imzası — RFC 8032 Test Vector #1 (mesaj: boş)
#[cfg(feature = "test-keys")]
pub const QEMU_TEST_SIGNATURE: [u8; SIGNATURE_SIZE] = [
    0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72,
    0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e, 0x82, 0x8a,
    0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74,
    0xd8, 0x73, 0xe0, 0x65, 0x22, 0x49, 0x01, 0x55,
    0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac,
    0xc6, 0x1e, 0x39, 0x70, 0x1c, 0xf9, 0xb4, 0x6b,
    0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24,
    0x65, 0x51, 0x41, 0x43, 0x8e, 0x7a, 0x10, 0x0b,
];

/// Production placeholder — OTP fuse'dan okunacak (v2.0)
#[cfg(not(feature = "test-keys"))]
pub const QEMU_TEST_PUBKEY: [u8; OTP_KEY_SIZE] = [0u8; OTP_KEY_SIZE];

/// Production placeholder — HSM tarafından üretilecek (v2.0)
#[cfg(not(feature = "test-keys"))]
pub const QEMU_TEST_SIGNATURE: [u8; SIGNATURE_SIZE] = [0u8; SIGNATURE_SIZE];

/// Root public key'i döndür.
///
/// QEMU v1.0: compile-time sabit (QEMU_TEST_PUBKEY)
/// Production v2.0: OTP fuse'dan oku — bu fonksiyon donanım kaydından okur.
///
/// Dönüş: &'static [u8; 32] — sıfır kopya, sıfır tahsis
#[inline]
pub fn get_root_public_key() -> &'static [u8; OTP_KEY_SIZE] {
    &QEMU_TEST_PUBKEY
}

// ═══════════════════════════════════════════════════════
// Kani Proofs — Sprint 13 (Proof 64-65)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // ─────────────────────────────────────────────────────


    // ─────────────────────────────────────────────────────
    // Kani Proof 65: get_root_public_key her byte'ına erişilebilir
    // ─────────────────────────────────────────────────────
    #[kani::proof]
    fn root_key_fully_accessible() {
        let key = get_root_public_key();

        // Dizi boyutu doğru
        assert!(key.len() == OTP_KEY_SIZE);

        // Tüm 32 indise bounded erişim (buffer overflow yok)
        let mut i = 0;
        while i < OTP_KEY_SIZE {
            let _ = key[i]; // panicsiz erişim
            i += 1;
        }
    }
}
