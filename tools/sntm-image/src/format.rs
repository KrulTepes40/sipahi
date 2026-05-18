//! sipahi-image-v1 layout — Section 4 FIX-C doctrine (custom flat layout).
//!
//! Layout:
//!   ┌────────────────────────────────┐
//!   │ Header (64 bytes)              │ magic "SIPI1" + abi_version + offsets
//!   ├────────────────────────────────┤
//!   │ kernel.elf  (aligned 64 byte)  │
//!   ├────────────────────────────────┤
//!   │ task_N.text.bin                │ per-task, aligned 64 byte
//!   │ task_N.rodata.bin              │
//!   │ task_N.data.bin                │
//!   ├────────────────────────────────┤
//!   │ task_N.cert.bin (424B) + sig   │ per-task
//!   ├────────────────────────────────┤
//!   │ Tail signature (64 bytes)      │ ed25519(SHA-512(header..last_cert_sig))
//!   └────────────────────────────────┘

use std::io::Write;

pub const IMAGE_MAGIC: [u8; 5] = *b"SIPI1";
pub const IMAGE_ABI_VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
pub const TAIL_SIG_SIZE: usize = 64;
pub const ALIGN: usize = 64;

/// Image header layout (64 bytes total):
///   [0..5)   magic "SIPI1"
///   [5..6)   reserved 0
///   [6..8)   reserved 0
///   [8..12)  abi_version (u32 LE)
///   [12..16) reserved (u32 LE)
///   [16..24) manifest_hash[0..8]  — first 8 bytes of BLAKE3(sipahi.toml)
///   [24..32) kernel_offset (u64 LE) — byte offset of kernel within image
///   [32..40) kernel_size (u64 LE)
///   [40..48) body_offset (u64 LE)  — byte offset where task bodies begin
///   [48..56) body_size (u64 LE)
///   [56..64) tail_sig_offset (u64 LE) — byte offset where tail sig lives
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub abi_version:      u32,
    pub manifest_hash8:   [u8; 8],
    pub kernel_offset:    u64,
    pub kernel_size:      u64,
    pub body_offset:      u64,
    pub body_size:        u64,
    pub tail_sig_offset:  u64,
}

impl Header {
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut h = [0u8; HEADER_SIZE];
        h[0..5].copy_from_slice(&IMAGE_MAGIC);
        // 5..8 reserved 0
        h[8..12].copy_from_slice(&self.abi_version.to_le_bytes());
        // 12..16 reserved
        h[16..24].copy_from_slice(&self.manifest_hash8);
        h[24..32].copy_from_slice(&self.kernel_offset.to_le_bytes());
        h[32..40].copy_from_slice(&self.kernel_size.to_le_bytes());
        h[40..48].copy_from_slice(&self.body_offset.to_le_bytes());
        h[48..56].copy_from_slice(&self.body_size.to_le_bytes());
        h[56..64].copy_from_slice(&self.tail_sig_offset.to_le_bytes());
        h
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_SIZE {
            return Err(format!("header truncated: {} bytes", bytes.len()));
        }
        if bytes[0..5] != IMAGE_MAGIC {
            return Err(format!(
                "magic mismatch: expected {:?}, got {:?}",
                IMAGE_MAGIC, &bytes[0..5]
            ));
        }
        let abi_version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let mut manifest_hash8 = [0u8; 8];
        manifest_hash8.copy_from_slice(&bytes[16..24]);
        let kernel_offset = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let kernel_size = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
        let body_offset = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
        let body_size = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
        let tail_sig_offset = u64::from_le_bytes(bytes[56..64].try_into().unwrap());
        Ok(Header {
            abi_version,
            manifest_hash8,
            kernel_offset,
            kernel_size,
            body_offset,
            body_size,
            tail_sig_offset,
        })
    }
}

/// Per-task payload inputs for image assembly.
#[derive(Debug, Clone)]
pub struct TaskPayload {
    pub name:        String,
    pub task_id:     u8,
    pub text_bin:    Vec<u8>,
    pub rodata_bin:  Vec<u8>,
    pub data_bin:    Vec<u8>,
    pub cert_bin:    Vec<u8>,  // 424 bytes
    pub cert_sig:    Vec<u8>,  // 64 bytes
}

/// Image assembly inputs.
pub struct ImageInputs<'a> {
    pub manifest_hash: [u8; 32],
    pub kernel:        &'a [u8],
    pub tasks:         &'a [TaskPayload],
}

/// Image (header + body) — caller appends ed25519 sig separately.
pub fn assemble(inputs: &ImageInputs) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(64 + inputs.kernel.len() + 4096);

    // Reserve header (filled at end with correct offsets).
    out.resize(HEADER_SIZE, 0u8);

    // Kernel
    align_to(&mut out, ALIGN);
    let kernel_offset = out.len() as u64;
    out.extend_from_slice(inputs.kernel);
    let kernel_size = inputs.kernel.len() as u64;

    // Body (tasks)
    align_to(&mut out, ALIGN);
    let body_offset = out.len() as u64;
    for task in inputs.tasks {
        // task text
        align_to(&mut out, ALIGN);
        out.extend_from_slice(&task.text_bin);
        // task rodata
        align_to(&mut out, ALIGN);
        out.extend_from_slice(&task.rodata_bin);
        // task data
        align_to(&mut out, ALIGN);
        out.extend_from_slice(&task.data_bin);
        // task cert + sig
        align_to(&mut out, ALIGN);
        out.extend_from_slice(&task.cert_bin);
        out.extend_from_slice(&task.cert_sig);
    }
    let body_size = (out.len() as u64) - body_offset;

    // Tail sig placeholder
    align_to(&mut out, ALIGN);
    let tail_sig_offset = out.len() as u64;

    // Patch header
    let mut manifest_hash8 = [0u8; 8];
    manifest_hash8.copy_from_slice(&inputs.manifest_hash[0..8]);
    let header = Header {
        abi_version: IMAGE_ABI_VERSION,
        manifest_hash8,
        kernel_offset,
        kernel_size,
        body_offset,
        body_size,
        tail_sig_offset,
    };
    let header_bytes = header.to_bytes();
    out[..HEADER_SIZE].copy_from_slice(&header_bytes);

    Ok(out)
}

fn align_to(out: &mut Vec<u8>, alignment: usize) {
    while out.len() % alignment != 0 {
        out.push(0);
    }
}

#[allow(dead_code)]
pub fn align_unused(out: &mut Vec<u8>, alignment: usize) {
    align_to(out, alignment)
}

pub fn write_image(out_path: &std::path::Path, body: &[u8], tail_sig: &[u8; TAIL_SIG_SIZE])
    -> Result<(), String>
{
    let mut f = std::fs::File::create(out_path)
        .map_err(|e| format!("create {}: {}", out_path.display(), e))?;
    f.write_all(body)
        .map_err(|e| format!("write body: {}", e))?;
    f.write_all(tail_sig)
        .map_err(|e| format!("write sig: {}", e))?;
    Ok(())
}
