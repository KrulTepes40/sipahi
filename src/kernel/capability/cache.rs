// Sipahi — Token Cache (Sprint 9)
// 4-slot constant-time lookup — her zaman 4 entry tarar, erken çıkış YOK
// Hit ~10c (4 × branch-free compare), miss = caller'a döner
//
// Round-robin eviction: next_slot % 4

use crate::common::types::ResourceId;

const CACHE_SLOTS: usize = 4;

#[derive(Clone, Copy)]
struct CacheEntry {
    valid:    bool,
    token_id: u8,
    resource: ResourceId,
    action:   u8,
}

impl CacheEntry {
    const fn empty() -> Self {
        CacheEntry { valid: false, token_id: 0, resource: 0, action: 0 }
    }
}

pub struct TokenCache {
    entries:   [CacheEntry; CACHE_SLOTS],
    next_slot: u8, // round-robin eviction pointer
}

impl TokenCache {
    pub const fn new() -> Self {
        TokenCache {
            entries:   [CacheEntry::empty(); CACHE_SLOTS],
            next_slot: 0,
        }
    }

    /// Sabit zamanlı lookup — 4 slot her zaman taranır, erken çıkış YOK
    /// Dal-free: bitwise AND ile hit accumulate
    pub fn lookup(&self, token_id: u8, resource: ResourceId, action: u8) -> bool {
        let mut found: u8 = 0;
        let mut i = 0;
        while i < CACHE_SLOTS {
            let e = &self.entries[i];
            let hit = (e.valid as u8)
                & ((e.token_id == token_id) as u8)
                & ((e.resource == resource) as u8)
                & ((e.action    == action)   as u8);
            found |= hit;
            i += 1;
        }
        found != 0
    }

    /// Round-robin insert — en eski entry üzerine yazar
    pub fn insert(&mut self, token_id: u8, resource: ResourceId, action: u8) {
        let slot = (self.next_slot as usize) % CACHE_SLOTS;
        self.entries[slot] = CacheEntry { valid: true, token_id, resource, action };
        self.next_slot = self.next_slot.wrapping_add(1);
    }

    /// Token ID'ye göre invalidate — task exit veya token revocation
    pub fn invalidate(&mut self, token_id: u8) {
        let mut i = 0;
        while i < CACHE_SLOTS {
            if self.entries[i].token_id == token_id {
                self.entries[i] = CacheEntry::empty();
            }
            i += 1;
        }
    }
}
