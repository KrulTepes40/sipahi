// Sipahi — SipahiMAC-STUB (Sprint 9)
// Sprint 13'te gerçek BLAKE3 (harici crate) ile değiştirilecek.
// API: HashProvider trait — broker.rs bu dosyayı görmez, sadece Crypto alias'ı kullanır.
//
// STUB garantileri:
//   - Deterministik: aynı (key, data) → aynı çıkış
//   - Sabit zamanlı: data değerlerine bağlı dal YOK
//   - Key bağımlı: farklı key → farklı çıkış
//   - KRİPTOGRAFİK DEĞİL: Sprint 13'e kadar production kullanımı YASAK

use super::provider::HashProvider;

pub struct Blake3Provider;

impl HashProvider for Blake3Provider {
    /// Keyed hash (stub) — 16 byte çıkış
    /// Algoritma: key-initialized state, data mixing, final XOR fold
    fn keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 16] {
        // State: key'in iki yarısının XOR fold'u
        let mut state = [0u8; 16];
        let mut i = 0;
        while i < 16 {
            state[i] = key[i] ^ key[i + 16];
            i += 1;
        }

        // Data absorb — sabit zamanlı mixing
        let mut di = 0;
        while di < data.len() {
            let slot = di % 16;
            // rotate_left(1) + XOR: data değerine bağlı branch yok
            state[slot] = state[slot]
                .wrapping_add(data[di])
                .rotate_left(1)
                ^ key[slot];
            di += 1;
        }

        // Final: key'in ikinci yarısıyla XOR
        let mut fi = 0;
        while fi < 16 {
            state[fi] ^= key[fi + 16];
            fi += 1;
        }

        state
    }
}
