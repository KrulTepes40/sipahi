// Modüler Kriptografi — Compile-time trait seçimi
// v1.0: BLAKE3 + Ed25519 (fast-crypto)
// v2.0: SHA-384/Zknh + LMS (cnsa-crypto)
//
// Kullanım:
//   capability/broker.rs → Crypto::mac()
//   sandbox/loader.rs    → Signer::verify()
//
// Sprint 9'da implemente edilecek

pub mod provider;

/// Compile-time seçilen hash/MAC provider
/// fast-crypto: SipahiMAC-STUB (Sprint 9) → BLAKE3 (Sprint 13)
/// cnsa-crypto: SHA-384 + Zknh HW (v2.0, henüz yok)
#[cfg(feature = "fast-crypto")]
mod blake3_impl;

#[cfg(feature = "fast-crypto")]
pub use blake3_impl::Blake3Provider;

#[cfg(feature = "fast-crypto")]
pub type Crypto = Blake3Provider;
