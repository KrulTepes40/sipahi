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

/// QEMU geliştirme ortamı test public key — RFC 8032 Test Vector #1
///
/// Production'da OTP fuse'dan okunur; bu sabit SADECE QEMU test ortamı içindir.
///
/// RFC 8032 Section 6.1, Test Vector 1:
///   Private key: 9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae3d55
///   Public key:  d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
pub const QEMU_TEST_PUBKEY: [u8; OTP_KEY_SIZE] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
    0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
    0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// QEMU geliştirme ortamı test imzası — RFC 8032 Test Vector #1 (mesaj: boş)
///
/// Production'da HSM tarafından üretilir; kernel binary hash'inin imzasıdır.
///
/// RFC 8032 Section 6.1, Test Vector 1 Signature:
///   e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555...
///   ...fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b
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
    // Kani Proof 64: OTP key boyutu tam 32 byte (Ed25519 spec)
    // ─────────────────────────────────────────────────────
    #[kani::proof]
    fn otp_key_size_is_32() {
        // Ed25519 public key: sabit 32 byte (Curve25519 nokta koordinatı)
        assert!(OTP_KEY_SIZE == 32);

        // Test anahtarının fiziksel boyutu sabitle eşleşiyor
        assert!(core::mem::size_of_val(&QEMU_TEST_PUBKEY) == 32);
        assert!(core::mem::size_of_val(&QEMU_TEST_PUBKEY) == OTP_KEY_SIZE);

        // İmza boyutu = 2 × key boyutu (R + S, her biri 32 byte)
        assert!(SIGNATURE_SIZE == 64);
        assert!(SIGNATURE_SIZE == 2 * OTP_KEY_SIZE);
    }

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
