//! Supply chain hash + signing pipeline.
//!
//! Inputs (deterministic):
//!   - sipahi.toml manifest                 → manifest_hash
//!   - rust-toolchain.toml                  → toolchain_hash
//!   - git rev-parse HEAD                   → source_commit
//!   - task name                            → task_name_hash
//!   - per-task PMP_PROFILES[task_id] bytes → pmp_profile_hash
//!   - task .text / .rodata / .data .bin    → text_hash / rodata_hash / data_hash
//!
//! Output: serialized TaskCertificate (CERT_SIZE bytes) + ed25519 signature
//! over those bytes. Section 8 CR-7 doctrine: ephemeral key, sign+verify
//! roundtrip (NOT git diff drift guard).

use std::path::Path;

use crate::cert::{Range64, TaskCertificate, ABI_VERSION, CERT_SIZE};

pub fn blake3_file(path: &Path) -> Result<[u8; 32], String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

pub fn blake3_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

pub fn git_head_bytes(repo_root: &Path) -> Result<[u8; 32], String> {
    // Read .git/HEAD; if it's "ref: refs/heads/X", chase the ref.
    let head_path = repo_root.join(".git").join("HEAD");
    let head = std::fs::read_to_string(&head_path)
        .map_err(|e| format!("read {}: {}", head_path.display(), e))?;
    let head = head.trim();
    let hash_hex = if let Some(rest) = head.strip_prefix("ref: ") {
        let ref_path = repo_root.join(".git").join(rest);
        std::fs::read_to_string(&ref_path)
            .map_err(|e| format!("read {}: {}", ref_path.display(), e))?
            .trim()
            .to_string()
    } else {
        head.to_string()
    };
    // Hex string → 20-byte raw; pad to 32 (BLAKE3 dimensions).
    let mut out = [0u8; 32];
    let raw = hex_decode(&hash_hex)
        .ok_or_else(|| format!("invalid git commit hex: {}", hash_hex))?;
    let n = raw.len().min(32);
    out[..n].copy_from_slice(&raw[..n]);
    Ok(out)
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Build a TaskCertificate for one task. Caller provides each pre-computed
/// hash; this function just assembles the struct with explicit padding zero.
#[allow(clippy::too_many_arguments)]
pub fn build_cert(
    task_id: u8,
    task_name: &str,
    source_commit: [u8; 32],
    toolchain_hash: [u8; 32],
    manifest_hash: [u8; 32],
    pmp_profile_hash: [u8; 32],
    allowed_syscalls: u8,
    allowed_channels: [u8; 8],
    allowed_mmio: [Range64; 4],
    max_stack_bytes: u32,
    forbidden_opcode_scan: u8,
    unsafe_count: u16,
    text_hash: [u8; 32],
    rodata_hash: [u8; 32],
    data_hash: [u8; 32],
    kani_proof_ids: [u32; 16],
) -> TaskCertificate {
    TaskCertificate {
        task_id,
        _pad1: [0u8; 7],
        task_name_hash: blake3_bytes(task_name.as_bytes()),
        source_commit,
        toolchain_hash,
        manifest_hash,
        pmp_profile_hash,
        allowed_syscalls,
        _pad2: [0u8; 7],
        allowed_channels,
        allowed_mmio,
        max_stack_bytes,
        forbidden_opcode_scan,
        unsafe_count,
        _pad3: 0,
        text_hash,
        rodata_hash,
        data_hash,
        kani_proof_ids,
        abi_version: ABI_VERSION,
        _pad4: 0,
    }
}

/// Sign a serialized cert with the given ed25519 PEM private key file.
/// Output: 64-byte raw signature (RFC 8032 wire format).
pub fn sign_cert(cert_bytes: &[u8; CERT_SIZE], priv_key_pem: &str) -> Result<[u8; 64], String> {
    let key = parse_ed25519_priv_pem(priv_key_pem)?;
    let sig = key.sign(cert_bytes, None);
    let sig_bytes: &[u8] = sig.as_ref();
    let arr: [u8; 64] = sig_bytes.try_into()
        .map_err(|_| "ed25519 signature length mismatch")?;
    Ok(arr)
}

/// Verify a cert + signature against a public key PEM.
pub fn verify_cert(
    cert_bytes: &[u8; CERT_SIZE],
    sig_bytes: &[u8; 64],
    pub_key_pem: &str,
) -> Result<(), String> {
    let pubkey = parse_ed25519_pub_pem(pub_key_pem)?;
    let sig = ed25519_compact::Signature::from_slice(sig_bytes)
        .map_err(|e| format!("signature parse: {:?}", e))?;
    pubkey.verify(cert_bytes, &sig)
        .map_err(|e| format!("ed25519 verify FAIL: {:?}", e))
}

/// Parse OpenSSL Ed25519 private key PEM (PKCS#8 wrapped).
fn parse_ed25519_priv_pem(pem: &str) -> Result<ed25519_compact::SecretKey, String> {
    let der = decode_pem_block(pem, "PRIVATE KEY")?;
    // PKCS#8 Ed25519 layout (RFC 8410):
    //   SEQUENCE {
    //     INTEGER 0,
    //     SEQUENCE { OID 1.3.101.112 (Ed25519) },
    //     OCTET STRING { OCTET STRING { 32-byte seed } }
    //   }
    // The seed is the last 32 bytes of the DER for OpenSSL output.
    if der.len() < 32 {
        return Err(format!("PEM too short: {} bytes", der.len()));
    }
    let seed_offset = der.len() - 32;
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&der[seed_offset..]);
    let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(&seed)
        .map_err(|e| format!("seed parse: {:?}", e))?);
    Ok(kp.sk)
}

/// Parse OpenSSL Ed25519 public key PEM (X.509 SubjectPublicKeyInfo wrapped).
fn parse_ed25519_pub_pem(pem: &str) -> Result<ed25519_compact::PublicKey, String> {
    let der = decode_pem_block(pem, "PUBLIC KEY")?;
    if der.len() < 32 {
        return Err(format!("PEM too short: {} bytes", der.len()));
    }
    let key_offset = der.len() - 32;
    ed25519_compact::PublicKey::from_slice(&der[key_offset..])
        .map_err(|e| format!("pubkey parse: {:?}", e))
}

fn decode_pem_block(pem: &str, label: &str) -> Result<Vec<u8>, String> {
    let begin = format!("-----BEGIN {}-----", label);
    let end   = format!("-----END {}-----", label);
    let start_idx = pem.find(&begin)
        .ok_or_else(|| format!("PEM missing {}", begin))?;
    let end_idx = pem.find(&end)
        .ok_or_else(|| format!("PEM missing {}", end))?;
    let body = &pem[start_idx + begin.len()..end_idx];
    let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    base64_decode(&cleaned)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    // Minimal base64 decoder (no padding handling beyond '=').
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
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for &b in bytes {
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
