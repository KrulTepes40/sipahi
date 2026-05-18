//! TaskCertificate struct definition + ABI invariants.
//!
//! repr(C) layout — Section 8 CR-9 forensics metadata only.
//! Padding manuel; cross-platform sabit boyut. Drift guard tests/integration.rs.

/// SNTM-SAFE TaskCertificate ABI version. Mevcut: 1. Field eklenirse +1,
/// ABI breaking change. Kernel cert opaque blob — parse YOK, ed25519 verify
/// yeterli (CR-9 doctrine).
pub const ABI_VERSION: u32 = 1;

/// SNTM-SAFE Range — `allowed_mmio` entries için.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range64 {
    pub base: u64,
    pub size: u64,
}

/// TaskCertificate — build-time forensics metadata bundle.
///
/// §17.4 of SIPAHI_SNTM_DESIGN.md. Bu struct kernel'in cert blob halini
/// AYNI binary layout ile yansıtır; kernel parse etmez (CR-9 doctrine),
/// sadece ed25519 verify. Forensics tool host-side bu struct'ı deserialize
/// edip DAL audit raporu üretir.
///
/// **Padding doctrine:** her u8/u16 sonrası explicit `_pad*` alignment'ı
/// koruyor — repr(C) compiler'a güvenmeyiz; cross-compiler determinizm için.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCertificate {
    // ── Kimlik ──
    pub task_id:        u8,
    pub _pad1:          [u8; 7],          // align(8) → next u8[32]
    pub task_name_hash: [u8; 32],         // BLAKE3(task name)

    // ── Tedarik zinciri ──
    pub source_commit:  [u8; 32],         // git rev-parse HEAD raw bytes
    pub toolchain_hash: [u8; 32],         // BLAKE3(rust-toolchain.toml)
    pub manifest_hash:  [u8; 32],         // BLAKE3(sipahi.toml)

    // ── Build-time invariants ──
    pub pmp_profile_hash:      [u8; 32],  // BLAKE3(PMP_PROFILES[task_id] bytes)
    pub allowed_syscalls:      u8,        // bitmap: SYS_*=0..5 (CR-9: forensics only)
    pub _pad2:                 [u8; 7],
    pub allowed_channels:      [u8; 8],   // channel id list; 0xFF = empty slot
    pub allowed_mmio:          [Range64; 4],
    pub max_stack_bytes:       u32,       // manifest stack_size; SAFE-4 call-stack refine
    pub forbidden_opcode_scan: u8,        // 1 = riscv-bin-verify PASS, 0 = FAIL
    pub _pad3:                 u8,        // align(2) for next u16 — explicit
    pub unsafe_count:          u16,       // task-lint output (cfg-aware count)

    // ── Binary section hashes (sntm-pack output BLAKE3) ──
    pub text_hash:   [u8; 32],
    pub rodata_hash: [u8; 32],
    pub data_hash:   [u8; 32],

    // ── Kani proof IDs (cargo kani --list parse + sembol hash) ──
    pub kani_proof_ids: [u32; 16],

    // ── Format version ──
    pub abi_version: u32,
    pub _pad4:       u32,                 // align(8) tail
}

/// CR-8 cross-crate drift invariant: cert binary layout must be exactly
/// CERT_SIZE bytes. Field eklenince ABI breaking change → +1 ABI_VERSION
/// + this constant güncellenir.
///
/// Hand-calc: 1+7 + 32 + 32+32+32 + 32 + 1+7+8 + 4*16 + 4+1+2+1 + 32+32+32
///          + 64 + 4+4
///          = 8 + 32 + 96 + 32 + 16 + 64 + 8 + 96 + 64 + 8 = 424 bytes
///
/// Actually compute: explicit padding makes this deterministic. Below static
/// assert verifies at compile time.
pub const CERT_SIZE: usize = 424;

#[allow(dead_code)]
const _ASSERT_SIZE: () = {
    let actual = core::mem::size_of::<TaskCertificate>();
    if actual != CERT_SIZE {
        // Compile-time fail with clear error.
        panic!("TaskCertificate size mismatch: declared CERT_SIZE != size_of");
    }
};

impl TaskCertificate {
    /// Serialize to raw bytes — `repr(C)` direct cast. Used for ed25519 sign
    /// message (deterministic byte layout).
    pub fn as_bytes(&self) -> [u8; CERT_SIZE] {
        // SAFETY: TaskCertificate is repr(C) with explicit padding fields
        // (_pad*), no uninit bytes. CERT_SIZE compile-time verified.
        unsafe {
            *(self as *const Self as *const [u8; CERT_SIZE])
        }
    }

    /// Deserialize from raw bytes — slice length must match CERT_SIZE.
    /// Returns None if length mismatch (defensive).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != CERT_SIZE { return None; }
        // SAFETY: length verified; TaskCertificate is repr(C) Copy.
        Some(unsafe {
            *(bytes.as_ptr() as *const Self)
        })
    }
}
