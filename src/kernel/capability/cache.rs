//! Constant-time 4-slot token lookup cache — O(1) hit path.
// Sipahi — Token Cache (Sprint 9)
// 4-slot constant-time lookup — her zaman 4 entry tarar, erken çıkış YOK
// Hit ~10c (4 × branch-free compare), miss = caller'a döner
//
// Round-robin eviction: next_slot % 4

use crate::common::types::ResourceId;

const CACHE_SLOTS: usize = 4;

#[derive(Clone, Copy)]
struct CacheEntry {
    valid:         bool,
    owner_task_id: u8,
    token_id:      u8,
    resource:      ResourceId,
    action:        u8,
    expires:       u32,
}

impl CacheEntry {
    const fn empty() -> Self {
        CacheEntry { valid: false, owner_task_id: 0, token_id: 0, resource: 0, action: 0, expires: 0 }
    }
}

pub struct TokenCache {
    entries:   [CacheEntry; CACHE_SLOTS],
    next_slot: u8, // round-robin eviction pointer
}

impl Default for TokenCache {
    fn default() -> Self { Self::new() }
}

impl TokenCache {
    pub const fn new() -> Self {
        TokenCache {
            entries:   [CacheEntry::empty(); CACHE_SLOTS],
            next_slot: 0,
        }
    }

    /// Sabit zamanlı lookup — 4 slot her zaman taranır, erken çıkış YOK
    /// Branch-free: bitwise AND ile hit accumulate + TTL kontrolü
    /// get_tick() bir kez çağrılır — 4× volatile read yerine 1×
    pub fn lookup(&self, task_id: u8, token_id: u8, resource: ResourceId, action: u8) -> bool {
        let now = crate::ipc::blackbox::get_tick(); // bir kez, döngü dışında
        let mut found: u8 = 0;
        let mut i = 0;
        while i < CACHE_SLOTS {
            let e = &self.entries[i];
            // Branch-free expiry check:
            // expires == 0 → sonsuz (is_infinite=1, not_expired irrelevant)
            // expires > 0 → now <= expires ise geçerli
            let expires = e.expires as u64;
            let is_infinite = (expires == 0) as u8;
            let not_expired = (now <= expires) as u8;
            let expiry_ok = is_infinite | not_expired;

            let hit = (e.valid as u8)
                & ((e.owner_task_id == task_id) as u8)
                & ((e.token_id == token_id) as u8)
                & ((e.resource == resource) as u8)
                & ((e.action   == action)   as u8)
                & expiry_ok;
            found |= hit;
            i += 1;
        }
        found != 0
    }

    /// Round-robin insert — en eski entry üzerine yazar
    pub fn insert(&mut self, task_id: u8, token_id: u8, resource: ResourceId, action: u8, expires: u32) {
        let slot = (self.next_slot as usize) % CACHE_SLOTS;
        self.entries[slot] = CacheEntry { valid: true, owner_task_id: task_id, token_id, resource, action, expires };
        self.next_slot = self.next_slot.wrapping_add(1);
    }

    /// Token ID'ye göre invalidate — token revocation
    /// Sprint U-14: runtime'da şu an çağıran yok, Kani proof'larda kullanılıyor.
    /// Future-proofing: explicit token revocation için API.
    #[allow(dead_code)]
    pub fn invalidate_by_token(&mut self, token_id: u8) {
        let mut i = 0;
        while i < CACHE_SLOTS {
            if self.entries[i].token_id == token_id {
                self.entries[i] = CacheEntry::empty();
            }
            i += 1;
        }
    }

    /// Owner task_id'ye göre invalidate — task isolate/exit
    /// Sprint U-14: Task izole edildiğinde o task'ın TÜM entry'leri temizlenir
    pub fn invalidate_by_owner(&mut self, owner_task_id: u8) {
        let mut i = 0;
        while i < CACHE_SLOTS {
            if self.entries[i].owner_task_id == owner_task_id {
                self.entries[i] = CacheEntry::empty();
            }
            i += 1;
        }
    }
}

// ═══════════════════════════════════════════════════════
// Sprint U-14: Kani test helper'lar
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
impl TokenCache {
    /// Kani proof helper — test entry ekle (private field'lara erişim)
    pub fn test_insert(&mut self, slot: usize, owner: u8, token: u8) {
        if slot < CACHE_SLOTS {
            self.entries[slot].valid = true;
            self.entries[slot].owner_task_id = owner;
            self.entries[slot].token_id = token;
            self.entries[slot].resource = 0;
            self.entries[slot].action = 0;
            self.entries[slot].expires = 0;
        }
    }

    pub fn test_is_valid(&self, slot: usize) -> bool {
        slot < CACHE_SLOTS && self.entries[slot].valid
    }
}

#[cfg(kani)]
mod verification {
    use super::TokenCache;

    /// Sprint U-14: invalidate_by_owner tüm entry'leri temizler
    #[kani::proof]
    fn invalidate_by_owner_clears_all() {
        let mut cache = TokenCache::new();
        let task_id: u8 = kani::any();
        kani::assume(task_id < 8);
        cache.test_insert(0, task_id, 1);
        cache.test_insert(1, task_id, 2);
        cache.invalidate_by_owner(task_id);
        assert!(!cache.test_is_valid(0));
        assert!(!cache.test_is_valid(1));
    }

    /// Sprint U-14: invalidate_by_owner başka task'ı korur
    #[kani::proof]
    fn invalidate_by_owner_preserves_others() {
        let mut cache = TokenCache::new();
        let task_a: u8 = kani::any();
        let task_b: u8 = kani::any();
        kani::assume(task_a < 8 && task_b < 8 && task_a != task_b);
        cache.test_insert(0, task_a, 1);
        cache.test_insert(1, task_b, 2);
        cache.invalidate_by_owner(task_a);
        assert!(!cache.test_is_valid(0));  // A temizlendi
        assert!(cache.test_is_valid(1));   // B korundu
    }
}
