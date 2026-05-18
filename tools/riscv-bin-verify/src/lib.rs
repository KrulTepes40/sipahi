//! Sipahi SNTM-SAFE RISC-V ELF binary verifier — public library API.
//!
//! SAFE-3 (sprint-u32) — §17.3 of SIPAHI_SNTM_DESIGN.md. Build-time check:
//! task ELF dosyasını manifest [[task.region]] + safe-tier kuralları üzerinden
//! denetler. Runtime cost SIFIR; reject sınıfları:
//!
//!   - Privileged ops (CSR/mret/sfence/wfi)   — Section 8 CR-10
//!   - F/D float ops + compressed FP variants — Section 8 CR-10
//!   - Forbidden sections (.got/.plt/.eh_frame/.init_array/...)
//!   - W^X violation (writable + executable section)
//!   - Relocation residue (R_RISCV_RELAX, PIC/PIE)
//!   - Region boundary (defined alloc symbols ∉ manifest regions, CR-11)
//!   - Immediate jal kernel-range targets (forward-edge CFI scope, FIX-A)
//!
//! Sub-workspace doctrine: bu crate kernel build tree'sinden BAĞIMSIZ host
//! tool. `cargo +stable build --release --target x86_64-unknown-linux-gnu`
//! ile derlenir.

pub mod decoder;   // RV64IMAC 32-bit + RVC 16-bit decode (G2)
pub mod manifest;  // sipahi.toml [[task]] + [[task.region]] parse
pub mod opcodes;   // SYSTEM funct12 + F/D family forbidden tables (G2, CR-10)
pub mod parser;    // ELF parse (object crate, sntm-pack pattern)
pub mod regions;   // Symbol filter + region boundary check (G4, CR-11)
pub mod sections;  // Forbidden section + W^X + relocation residue (G3)

use std::path::Path;

/// Single violation report — kategori + insan-okunabilir mesaj.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub category: Category,
    pub message:  String,
}

/// Verification verdict for one task ELF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    /// Privileged opcode (CSR, mret, sfence, wfi, ebreak).
    PrivilegedOp,
    /// F/D float op (RV64IMAC dışı; F + D + compressed FP).
    FloatOp,
    /// Forbidden section name (.got, .init_array, ...).
    ForbiddenSection,
    /// W^X (writable + executable section flag combo).
    WxViolation,
    /// Relocation residue (R_RISCV_RELAX, PIC/PIE).
    Relocation,
    /// Defined symbol outside manifest [[task.region]] addresses.
    RegionBoundary,
    /// Indirect jal/jalr target in kernel range.
    KernelRangeJal,
    /// ELF parse / structural error.
    ParseError,
}

/// Verification report — `violations.is_empty()` → PASS.
#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    pub task_name:  String,
    pub elf_bytes:  usize,
    pub violations: Vec<Violation>,
}

impl VerifyReport {
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Top-level verify API for integration tests + main.rs.
///
/// Reads ELF + manifest from disk, runs all check passes, returns report.
/// Caller decides exit code: PASS → 0, violations → 1, parse error → 2.
pub fn verify_elf(elf_path: &Path, manifest_path: &Path, task_name: &str)
    -> Result<VerifyReport, String>
{
    let elf_bytes = std::fs::read(elf_path)
        .map_err(|e| format!("cannot read ELF {}: {}", elf_path.display(), e))?;
    let manifest_str = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("cannot read manifest {}: {}", manifest_path.display(), e))?;
    let manifest: manifest::Manifest = toml::from_str(&manifest_str)
        .map_err(|e| format!("manifest parse error: {}", e))?;

    verify_elf_bytes(&elf_bytes, &manifest, task_name)
}

/// Pure in-memory verify — useful for integration tests (pre-built ELF
/// fixtures, no filesystem). Section 9.2 T3 doctrine (realistic fixture).
pub fn verify_elf_bytes(
    elf_bytes: &[u8],
    manifest: &manifest::Manifest,
    task_name: &str,
) -> Result<VerifyReport, String> {
    let mut report = VerifyReport {
        task_name: task_name.to_string(),
        elf_bytes: elf_bytes.len(),
        violations: Vec::new(),
    };

    let parsed = parser::parse_elf(elf_bytes)
        .map_err(|e| format!("ELF parse failed: {}", e))?;

    // G2 (CR-10): instruction-level forbidden opcode scan
    let opcode_violations = opcodes::scan_forbidden_opcodes(&parsed);
    report.violations.extend(opcode_violations);

    // G3: section + W^X + relocation
    let section_violations = sections::check_sections(&parsed);
    report.violations.extend(section_violations);

    // G4 (CR-11): symbol region boundary
    let region_violations = regions::check_regions(&parsed, manifest, task_name);
    report.violations.extend(region_violations);

    Ok(report)
}
