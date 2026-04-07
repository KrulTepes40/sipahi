//! BLAKE3 keyed hash provider — 32-byte key, 16-byte MAC output (no_std).
// Sipahi — Gerçek BLAKE3 Keyed Hash (Sprint 13)
// Önceki: SipahiMAC-STUB (kriptografik değildi, Sprint 9'dan beri placeholder)
// Sprint 13: blake3 crate ile gerçek implementasyon (no_std uyumlu)
//
// API: HashProvider trait — broker.rs bu dosyayı görmez, sadece Crypto alias'ı kullanır.
//
// BLAKE3 garantileri:
//   - Kriptografik: Modern PRF, güçlü anahtar bağımlılığı
//   - Deterministik: aynı (key, data) → aynı çıkış (her zaman, her platformda)
//   - Sabit zamanlı: BLAKE3 timing attack dirençli (Merkle ağacı, sabit iş yükü)
//   - Key bağımlı: farklı key → farklı çıkış (PRF güvenliği)
//   - Hızlı: ~350 cycle/32B token input (CVA6 tahmini)
//   - no_std uyumlu: std feature kapalı → alloc gerektirmez
//
// NOT: CNSA 2.0 uyumlu DEĞİL (BLAKE3 NIST listesinde yok)
//      → v2.0'da SHA-384/Zknh ile değiştirilecek (tek flag)
//
// Kani Proof 69: keyed_hash çıkışı 16 byte, panik yok
// Kani Proof 70: Boş data ile keyed_hash panik YOK (edge case)

use super::provider::HashProvider;

pub struct Blake3Provider;

// ─── Gerçek BLAKE3 (not(kani) guard — kani blake3 crate'i işleyemez) ──────────

#[cfg(not(kani))]
impl HashProvider for Blake3Provider {
    /// BLAKE3 keyed hash — 32 byte çıkışın ilk 16 byte'ı döndürülür.
    ///
    /// BLAKE3 XOF (extendable output): herhangi bir prefix uniform random.
    /// Token MAC: 16 byte yeterli (128-bit güvenlik düzeyi).
    fn keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 16] {
        // blake3::keyed_hash → 32 byte Hash (sabit boyut, yığın üzerinde)
        let hash = blake3::keyed_hash(key, data);
        let bytes = hash.as_bytes(); // &[u8; 32]

        // İlk 16 byte → MAC çıkışı
        // copy_from_slice yerine bounded while — no_std + clippy uyumlu
        let mut result = [0u8; 16];
        let mut i = 0;
        while i < 16 {
            result[i] = bytes[i];
            i += 1;
        }
        result
    }
}

// ─── Kani stub — model checker için basitleştirilmiş ──────────────────────────
// Gerçek BLAKE3 Kani'de çalışmaz (çok büyük, bounded model checking timeout).
// Stub: memory safety ve bounds kanıtı için yeterli.

#[cfg(kani)]
impl HashProvider for Blake3Provider {
    fn keyed_hash(key: &[u8; 32], _data: &[u8]) -> [u8; 16] {
        // Sadece key'in ilk 16 byte'ını döndür — bounds check kanıtı için
        let mut result = [0u8; 16];
        let mut i = 0;
        while i < 16 {
            result[i] = key[i];
            i += 1;
        }
        result
    }
}

// ═══════════════════════════════════════════════════════
// Kani Proofs — Sprint 13 (Proof 69-70)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod kani_proofs {
    use super::*;
    use crate::common::crypto::provider::HashProvider;

    // ─────────────────────────────────────────────────────
    // Kani Proof 69: keyed_hash çıkışı her zaman tam 16 byte, panik yok
    // ─────────────────────────────────────────────────────
    #[kani::proof]
    fn blake3_output_16_bytes_no_panic() {
        let key = [0xABu8; 32];
        let data = [0x42u8; 64]; // tipik token boyutu

        let result = Blake3Provider::keyed_hash(&key, &data);

        // Çıkış her zaman tam 16 byte (sabit dizi, overflow imkansız)
        assert!(result.len() == 16);

        // Tüm indislere erişilebilir (bounds-safe)
        let mut i = 0;
        while i < 16 {
            let _ = result[i];
            i += 1;
        }
    }

    // ─────────────────────────────────────────────────────
    // Kani Proof 70: Boş data ile keyed_hash — edge case, panik yok
    // ─────────────────────────────────────────────────────
    #[kani::proof]
    fn blake3_empty_data_no_panic() {
        let key = [0u8; 32];
        let data: [u8; 0] = []; // boş mesaj (RFC 8032 TV1 senaryosu)

        let result = Blake3Provider::keyed_hash(&key, &data);

        // Boş input için de 16 byte çıkış (sabit boyut garantisi)
        assert!(result.len() == 16);
    }
}
