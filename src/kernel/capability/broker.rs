//! Token MAC validation with BLAKE3, nonce replay guard, and 4-slot cache.
// Sipahi — Capability Broker (Sprint 9)
// Token doğrulama: MAC + constant-time compare + cache
//
// İki yol:
//   validate_full  — MAC hesapla, cache'e ekle  (~400c, miss path)
//   validate_cached — sadece cache bak          (~10c,  hit path, syscall'dan)
//
// WCET: validate_cached = WCET_TOKEN_CACHE_HIT = 10c
//       validate_full    = WCET_TOKEN_VALIDATE  = 400c

use super::token::Token;
use super::cache::TokenCache;
use crate::common::crypto::provider::HashProvider;

#[cfg(feature = "fast-crypto")]
use crate::common::crypto::Crypto;

/// MAC key — boot'ta provision_key() ile bir kez yazılır
static mut MAC_KEY: [u8; 32] = [0u8; 32];
static mut KEY_READY: bool = false;

/// Son kabul edilen nonce — replay guard (monoton artan)
static mut LAST_NONCE: u32 = 0;

/// Token cache — statik, heap yok
static mut TOKEN_CACHE: TokenCache = TokenCache::new();

/// MAC key provisioning — boot sequence'de BİR KEZ çağrılır
/// Tekrar çağrı yoksayılır (key rotation Sprint 13)
pub fn provision_key(key: &[u8; 32]) {
    // Sıfır key → güvenli varsayılan: KEY_READY false kalır
    let mut all_zero = true;
    let mut j = 0;
    while j < 32 {
        if key[j] != 0 { all_zero = false; }
        j += 1;
    }
    if all_zero { return; }

    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        if !KEY_READY {
            let mut i = 0;
            while i < 32 {
                MAC_KEY[i] = key[i];
                i += 1;
            }
            KEY_READY = true;
        }
    }
}

/// Cache-only lookup — sys_cap_invoke fast path (~10c)
/// validate_full ile cache'e eklenmemiş token → false döner
pub fn validate_cached(token_id: u8, resource: u16, action: u8) -> bool {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache = &*core::ptr::addr_of!(TOKEN_CACHE);
        cache.lookup(token_id, resource, action)
    }
}

/// Full token validation — MAC hesapla, nonce kontrol, cache'e ekle (~400c)
/// Returns: true = geçerli + cache'e eklendi, false = RED (MAC/nonce/key fail)
#[cfg(feature = "fast-crypto")]
pub fn validate_full(token: &Token) -> bool {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache = &*core::ptr::addr_of!(TOKEN_CACHE);
        if cache.lookup(token.id, token.resource, token.action) {
            return true;
        }
        if !KEY_READY {
            return false;
        }
        // Replay guard: nonce kesinlikle monoton artan olmalı
        let last = core::ptr::read_volatile(core::ptr::addr_of!(LAST_NONCE));
        if token.nonce <= last {
            return false; // replay veya stale token
        }
        let header = token.header_bytes();
        let key = &*core::ptr::addr_of!(MAC_KEY);
        let expected = Crypto::keyed_hash(key, &header);
        if ct_eq_16(&token.mac, &expected) {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST_NONCE), token.nonce);
            let cache_mut = &mut *core::ptr::addr_of_mut!(TOKEN_CACHE);
            cache_mut.insert(token.id, token.resource, token.action);
            true
        } else {
            false
        }
    }
}

/// Token MAC hesapla ve token.mac alanına yaz
/// Kullanım: boot test, token üretimi (gerçek sistemde HSM yapar)
#[cfg(feature = "fast-crypto")]
pub fn sign_token(token: &mut Token) {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        if !KEY_READY {
            return;
        }
        let header = token.header_bytes();
        let key = &*core::ptr::addr_of!(MAC_KEY);
        token.mac = Crypto::keyed_hash(key, &header);
    }
}

/// Cache invalidate — task exit veya token revocation
pub fn invalidate_task(token_id: u8) {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache_mut = &mut *core::ptr::addr_of_mut!(TOKEN_CACHE);
        cache_mut.invalidate(token_id);
    }
}

/// Constant-time 16-byte compare — timing attack önlemi
/// Tüm 16 byte her zaman karşılaştırılır, erken çıkış YOK
#[inline(always)]
fn ct_eq_16(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut diff: u8 = 0;
    let mut i = 0;
    while i < 16 {
        diff |= a[i] ^ b[i];
        i += 1;
    }
    diff == 0
}
