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
// pub mod blake3;       // Sprint 9
// pub mod sha384;       // v2.0
