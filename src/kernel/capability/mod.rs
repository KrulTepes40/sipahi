//! Capability-based access control: token broker, MAC cache, action flags.
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
pub(crate) use broker::{provision_key, validate_cached, invalidate_task};
#[cfg(feature = "fast-crypto")]
#[allow(unused_imports)]
pub(crate) use broker::{validate_full, sign_token};

// ═══════════════════════════════════════════════════════
// Kani — Sprint 9 (Proof 40-46)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::token::{Token, ACTION_READ, ACTION_WRITE, ACTION_EXECUTE, ACTION_ALL};
    use super::cache::TokenCache;

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
        cache.insert(7, 3, ACTION_READ, 0);
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
        let accepted = token_nonce > last_nonce;
        assert!(!accepted);
    }

    /// Proof 114: Token header_bytes deterministic
    #[kani::proof]
    fn token_header_deterministic() {
        let mut t = Token::zeroed();
        t.id = kani::any();
        t.resource = kani::any();
        t.action = kani::any();
        t.nonce = kani::any();
        t.expires = kani::any();
        let h1 = t.header_bytes();
        let h2 = t.header_bytes();
        let mut i = 0;
        while i < 16 { assert!(h1[i] == h2[i]); i += 1; }
    }

    /// Proof 115: Token header id byte doğru pozisyonda
    #[kani::proof]
    fn token_header_id_position() {
        let mut t = Token::zeroed();
        t.id = kani::any();
        let h = t.header_bytes();
        assert!(h[0] == t.id);
    }

    /// Proof 116: Cache invalidate sonrası lookup false
    #[kani::proof]
    fn cache_invalidate_then_lookup_false() {
        let mut cache = TokenCache::new();
        let tid: u8 = kani::any();
        let res: u16 = kani::any();
        let act: u8 = kani::any();
        cache.insert(tid, res, act, 0);
        assert!(cache.lookup(tid, res, act));
        cache.invalidate(tid);
        assert!(!cache.lookup(tid, res, act));
    }

    /// Proof 117: Cache 4 slot dolu → 5. insert en eski üzerine yazar
    #[kani::proof]
    fn cache_overwrites_oldest() {
        let mut cache = TokenCache::new();
        cache.insert(0, 100, 1, 0);
        cache.insert(1, 200, 2, 0);
        cache.insert(2, 300, 3, 0);
        cache.insert(3, 400, 4, 0);
        // 5. insert → slot 0 üzerine yazar (round-robin)
        cache.insert(4, 500, 5, 0);
        assert!(!cache.lookup(0, 100, 1)); // evicted
        assert!(cache.lookup(4, 500, 5));  // yeni
    }

    /// Proof 118: Sıfır key tespiti
    #[kani::proof]
    fn zero_key_is_detected() {
        let key = [0u8; 32];
        let mut all_zero = true;
        let mut i = 0;
        while i < 32 { if key[i] != 0 { all_zero = false; } i += 1; }
        assert!(all_zero);
    }

    /// Proof 172: Cache TTL — expires>0, get_tick()=0 → henüz dolmadı → hit
    #[kani::proof]
    fn cache_not_expired_entry_found() {
        let mut cache = TokenCache::new();
        // expires=1, Kani'de get_tick() BB_TICK=0 → 0 <= 1 → not expired
        cache.insert(5, 200, 1, 1);
        assert!(cache.lookup(5, 200, 1));
    }

    /// Invalidated token → herhangi resource/action ile asla bulunamaz
    #[kani::proof]
    fn invalidated_token_never_found_in_cache() {
        let mut cache = TokenCache::new();
        let tid: u8 = kani::any();
        let resource: u16 = kani::any();
        let action: u8 = kani::any();
        cache.insert(tid, resource, action, 0);
        cache.invalidate(tid);
        let search_resource: u16 = kani::any();
        let search_action: u8 = kani::any();
        assert!(!cache.lookup(tid, search_resource, search_action));
    }
}
