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
use crate::common::config::MAX_TASKS;
use crate::common::crypto::provider::HashProvider;
use crate::common::sync::SingleHartCell;

#[cfg(feature = "fast-crypto")]
use crate::common::crypto::Crypto;

/// MAC key — boot'ta provision_key() ile bir kez yazılır
static MAC_KEY: SingleHartCell<[u8; 32]> = SingleHartCell::new([0u8; 32]);
static KEY_READY: SingleHartCell<bool> = SingleHartCell::new(false);

/// Per-task nonce — replay guard (her task bağımsız monoton artan)
static LAST_NONCE: SingleHartCell<[u32; MAX_TASKS]> = SingleHartCell::new([0u32; MAX_TASKS]);

/// Token cache — statik, heap yok
static TOKEN_CACHE: SingleHartCell<TokenCache> = SingleHartCell::new(TokenCache::new());

/// MAC key provisioning — boot sequence'de BİR KEZ çağrılır
/// Tekrar çağrı yoksayılır (key rotation Sprint 13)
pub(crate) fn provision_key(key: &[u8; 32]) {
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
        if !*KEY_READY.get() {
            let mut i = 0;
            while i < 32 {
                (*MAC_KEY.get_mut())[i] = key[i];
                i += 1;
            }
            *KEY_READY.get_mut() = true;
        }
    }
}

/// Cache-only lookup — sys_cap_invoke fast path (~10c)
/// validate_full ile cache'e eklenmemiş token → false döner
#[must_use = "cache lookup result must be checked"]
pub(crate) fn validate_cached(token_id: u8, resource: u16, action: u8) -> bool {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache = TOKEN_CACHE.get();
        cache.lookup(token_id, resource, action)
    }
}

/// Full token validation — MAC hesapla, nonce kontrol, cache'e ekle (~400c)
/// Returns: true = geçerli + cache'e eklendi, false = RED (MAC/nonce/key fail)
#[cfg(feature = "fast-crypto")]
#[must_use = "validation result must be checked"]
pub(crate) fn validate_full(token: &Token) -> bool {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache = TOKEN_CACHE.get();
        if cache.lookup(token.id, token.resource, token.action) {
            return true;
        }
        if !*KEY_READY.get() {
            return false;
        }
        // Replay guard: per-task nonce, kesinlikle monoton artan
        let task_id = token.task_id as usize;
        if task_id >= MAX_TASKS { return false; }
        let last = core::ptr::read_volatile(
            &(*LAST_NONCE.get())[task_id]
        );
        if token.nonce <= last {
            return false; // replay veya stale token
        }
        // Expiry check: expires > 0 ise aktif, tick kontrolü yap
        if token.expires > 0 {
            let current_tick = crate::ipc::blackbox::get_tick();
            if current_tick > token.expires as u64 {
                return false; // expired token
            }
        }
        let header = token.header_bytes();
        let key = MAC_KEY.get();
        let expected = Crypto::keyed_hash(key, &header);
        if ct_eq_16(&token.mac, &expected) {
            core::ptr::write_volatile(
                &mut (*LAST_NONCE.get_mut())[task_id],
                token.nonce,
            );
            let cache_mut = TOKEN_CACHE.get_mut();
            cache_mut.insert(token.id, token.resource, token.action, token.expires);
            true
        } else {
            false
        }
    }
}

/// Token MAC hesapla ve token.mac alanına yaz
/// Kullanım: boot test, token üretimi (gerçek sistemde HSM yapar)
#[cfg(feature = "fast-crypto")]
pub(crate) fn sign_token(token: &mut Token) {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        if !*KEY_READY.get() {
            return;
        }
        let header = token.header_bytes();
        let key = MAC_KEY.get();
        token.mac = Crypto::keyed_hash(key, &header);
    }
}

/// Cache invalidate — task exit veya token revocation
pub(crate) fn invalidate_task(token_id: u8) {
    // SAFETY: Single-hart, no concurrent access to TOKEN_CACHE/MAC_KEY.
    unsafe {
        let cache_mut = TOKEN_CACHE.get_mut();
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
    // LLVM'in döngüyü memcmp'ye optimize etmesini engelle
    // black_box: değeri opaque yapar, derleyici analiz edemez
    core::hint::black_box(diff) == 0
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// Proof 137: ct_eq_16 aynı girdi → true
    #[kani::proof]
    fn ct_eq_16_same_input_true() {
        let mut a = [0u8; 16];
        let mut i = 0;
        while i < 16 { a[i] = kani::any(); i += 1; }
        let b = a;
        assert!(ct_eq_16(&a, &b));
    }

    /// Proof 138: ct_eq_16 tek byte fark → false
    #[kani::proof]
    fn ct_eq_16_single_byte_diff_false() {
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        let mut i = 0;
        while i < 16 { let v: u8 = kani::any(); a[i] = v; b[i] = v; i += 1; }
        let idx: usize = kani::any();
        kani::assume(idx < 16);
        b[idx] = b[idx].wrapping_add(1);
        assert!(!ct_eq_16(&a, &b));
    }
}
