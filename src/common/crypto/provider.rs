// Kripto provider trait'leri
// Rust monomorphization → runtime branching YOK
// Seçilmeyen provider binary'de yer KAPLAMAZ

/// Hash/MAC provider trait
pub trait HashProvider {
    /// Keyed hash hesapla (token MAC için)
    /// Input: key + data → 16 byte MAC
    /// WCET: sabit (constant-time)
    fn keyed_hash(key: &[u8; 32], data: &[u8]) -> [u8; 16];
}

/// İmza doğrulama provider trait
pub trait SignatureVerifier {
    /// İmza doğrula (WASM modül + secure boot için)
    /// true = geçerli, false = REJECT
    fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool;
}

// Compile-time seçim:
//
// #[cfg(feature = "fast-crypto")]
// pub type Crypto = blake3::Blake3Provider;     // 350 cycle
//
// #[cfg(feature = "cnsa-crypto")]
// pub type Crypto = sha384::Sha384Provider;     // 1,500 cycle + Zknh HW
