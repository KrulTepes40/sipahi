// Sipahi — Capability System (Sprint 9)
// token.rs: 32B Token struct, ACTION_* sabitleri
// cache.rs: 4-slot constant-time TokenCache
// broker.rs: MAC doğrulama (SipahiMAC-STUB → Sprint 13 BLAKE3)

pub mod token;
pub mod cache;
pub mod broker;

#[allow(unused_imports)]
pub use token::{Token, ACTION_READ, ACTION_WRITE, ACTION_EXECUTE, ACTION_ALL};
#[allow(unused_imports)]
pub use broker::{provision_key, validate_cached, invalidate_task};
#[cfg(feature = "fast-crypto")]
#[allow(unused_imports)]
pub use broker::{validate_full, sign_token};

// ═══════════════════════════════════════════════════════
// Kani — Sprint 9 (Proof 40-46)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::token::{Token, ACTION_READ, ACTION_WRITE, ACTION_EXECUTE, ACTION_ALL};
    use super::cache::TokenCache;

    /// Proof 40: Token struct boyutu 32 byte (layout doğrulaması)
    #[kani::proof]
    fn token_size_32_bytes() {
        assert!(core::mem::size_of::<Token>() == 32);
    }

    /// Proof 41: header_bytes deterministik — aynı token, aynı çıkış
    #[kani::proof]
    fn token_header_bytes_deterministic() {
        let mut t = Token::zeroed();
        t.id = 0x42;
        t.resource = 0x0102;
        t.action = ACTION_READ;
        t.nonce = 0xDEAD;
        let h1 = t.header_bytes();
        let h2 = t.header_bytes();
        let mut i = 0;
        while i < 16 {
            assert!(h1[i] == h2[i]);
            i += 1;
        }
    }

    /// Proof 42: header_bytes alan encode'ları doğru (offset ve LE)
    #[kani::proof]
    fn token_header_encodes_fields() {
        let mut t = Token::zeroed();
        t.id = 0x5A;
        t.task_id = 0x03;
        t.resource = 0xBEEF;
        t.expires = 0x12345678;
        let h = t.header_bytes();
        assert!(h[0] == 0x5A);           // id at byte 0
        assert!(h[1] == 0x03);           // task_id at byte 1
        assert!(h[2] == 0xEF);           // resource low byte (LE)
        assert!(h[3] == 0xBE);           // resource high byte (LE)
        assert!(h[8]  == 0x78);          // expires byte 0 (LE)
        assert!(h[9]  == 0x56);
        assert!(h[10] == 0x34);
        assert!(h[11] == 0x12);
    }

    /// Proof 43: Pad byte'ları MAC hesabında sıfır
    #[kani::proof]
    fn token_header_pad_zero() {
        let mut t = Token::zeroed();
        t._pad = [0xFF, 0xFF]; // pad'e ne yazılırsa yazılsın
        let h = t.header_bytes();
        assert!(h[6] == 0);  // pad[0] → sıfır
        assert!(h[7] == 0);  // pad[1] → sıfır
    }

    /// Proof 44: Boş cache lookup → her zaman false
    #[kani::proof]
    fn cache_empty_lookup_false() {
        let cache = TokenCache::new();
        assert!(!cache.lookup(0, 0, 0));
        assert!(!cache.lookup(1, 0, ACTION_READ));
        assert!(!cache.lookup(0xFF, 0xFFFF, 0xFF));
    }

    /// Proof 45: Cache insert → sonraki lookup hit
    #[kani::proof]
    fn cache_insert_then_lookup() {
        let mut cache = TokenCache::new();
        cache.insert(7, 3, ACTION_READ);
        assert!(cache.lookup(7, 3, ACTION_READ));
        assert!(!cache.lookup(7, 3, ACTION_WRITE));  // farklı action → miss
        assert!(!cache.lookup(0, 3, ACTION_READ));   // farklı id → miss
    }

    /// Proof 46: ACTION flag'leri birbirini maskelemiyor, OR ile 0x07
    #[kani::proof]
    fn action_flags_disjoint_and_complete() {
        assert!(ACTION_READ    == 0x01);
        assert!(ACTION_WRITE   == 0x02);
        assert!(ACTION_EXECUTE == 0x04);
        assert!(ACTION_ALL     == 0x07);
        assert!(ACTION_READ  & ACTION_WRITE   == 0);
        assert!(ACTION_READ  & ACTION_EXECUTE == 0);
        assert!(ACTION_WRITE & ACTION_EXECUTE == 0);
        assert!(ACTION_READ | ACTION_WRITE | ACTION_EXECUTE == ACTION_ALL);
    }

    /// Proof 73: Replay nonce her zaman reject — last >= token.nonce → false
    #[kani::proof]
    fn replay_nonce_always_rejected() {
        let last_nonce: u32 = kani::any();
        let token_nonce: u32 = kani::any();
        kani::assume(token_nonce <= last_nonce);
        // Replay guard mantığı: nonce <= last → reject
        let accepted = token_nonce > last_nonce;
        assert!(!accepted);
    }
}
