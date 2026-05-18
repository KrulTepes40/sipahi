//! Image-level ed25519 sign + verify (tail signature).
//!
//! Section 4 FIX-C doctrine: sig kapsamı = header + body (tail_sig_offset
//! öncesi her şey). Roundtrip [10/10] gate doğrular.

use crate::format::{Header, HEADER_SIZE, IMAGE_MAGIC, TAIL_SIG_SIZE};

/// Sign image body (everything up to tail_sig_offset) with the given PEM priv.
pub fn sign_image(body_bytes: &[u8], priv_key_pem: &str) -> Result<[u8; TAIL_SIG_SIZE], String> {
    let key = parse_ed25519_priv_pem(priv_key_pem)?;
    let sig = key.sign(body_bytes, None);
    let sig_bytes: &[u8] = sig.as_ref();
    let arr: [u8; 64] = sig_bytes.try_into()
        .map_err(|_| "ed25519 sig length mismatch")?;
    Ok(arr)
}

/// Verify image — parses header, computes body slice, verifies sig.
pub fn verify_image(image: &[u8], pub_key_pem: &str) -> Result<(), String> {
    if image.len() < HEADER_SIZE + TAIL_SIG_SIZE {
        return Err(format!("image too short: {} bytes", image.len()));
    }
    let header = Header::from_bytes(&image[..HEADER_SIZE])?;
    let tail_off = header.tail_sig_offset as usize;
    if tail_off + TAIL_SIG_SIZE > image.len() {
        return Err(format!(
            "tail offset {} + sig {} > image len {}",
            tail_off, TAIL_SIG_SIZE, image.len()
        ));
    }
    if image[0..5] != IMAGE_MAGIC {
        return Err("magic mismatch".into());
    }

    let body = &image[..tail_off];
    let sig_bytes = &image[tail_off..tail_off + TAIL_SIG_SIZE];

    let pubkey = parse_ed25519_pub_pem(pub_key_pem)?;
    let sig = ed25519_compact::Signature::from_slice(sig_bytes)
        .map_err(|e| format!("sig parse: {:?}", e))?;
    pubkey.verify(body, &sig)
        .map_err(|e| format!("ed25519 verify FAIL: {:?}", e))
}

// ── PEM parsers ─ (sntm-cert-gen chain.rs ile bilinçli duplicate; SAFE-4
// `sntm-crypto` shared crate ile birleştirilebilir — bu sprintte SCOPE OUT.)

fn parse_ed25519_priv_pem(pem: &str) -> Result<ed25519_compact::SecretKey, String> {
    let der = decode_pem_block(pem, "PRIVATE KEY")?;
    if der.len() < 32 { return Err("PEM too short".into()); }
    let seed_offset = der.len() - 32;
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&der[seed_offset..]);
    let kp = ed25519_compact::KeyPair::from_seed(
        ed25519_compact::Seed::from_slice(&seed)
            .map_err(|e| format!("seed parse: {:?}", e))?,
    );
    Ok(kp.sk)
}

fn parse_ed25519_pub_pem(pem: &str) -> Result<ed25519_compact::PublicKey, String> {
    let der = decode_pem_block(pem, "PUBLIC KEY")?;
    if der.len() < 32 { return Err("PEM too short".into()); }
    let key_offset = der.len() - 32;
    ed25519_compact::PublicKey::from_slice(&der[key_offset..])
        .map_err(|e| format!("pubkey parse: {:?}", e))
}

fn decode_pem_block(pem: &str, label: &str) -> Result<Vec<u8>, String> {
    let begin = format!("-----BEGIN {}-----", label);
    let end   = format!("-----END {}-----", label);
    let start = pem.find(&begin).ok_or_else(|| format!("missing {}", begin))?;
    let end_i = pem.find(&end).ok_or_else(|| format!("missing {}", end))?;
    let body = &pem[start + begin.len()..end_i];
    let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    base64_decode(&cleaned)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let table: [i8; 256] = {
        let mut t = [-1i8; 256];
        let alpha = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < alpha.len() {
            t[alpha[i] as usize] = i as i8;
            i += 1;
        }
        t
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for &b in s.as_bytes() {
        if b == b'=' { break; }
        let v = table[b as usize];
        if v < 0 { return Err(format!("invalid base64 char: {}", b as char)); }
        buf = (buf << 6) | (v as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}
