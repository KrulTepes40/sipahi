//! riscv-bin-verify integration tests.
//!
//! Section 9.2 T1-T8 doctrine:
//!   T1 Pozitif + negatif çift
//!   T2 Error mesaj string assert
//!   T3 Realistic fixture (synthetic ELF builder, RV64 instruction encodings)
//!   T4 Temp dir izolasyon (tempfile bağımlılığı bu sprintte minimal)
//!   T5 Determinism — no random/timestamp
//!   T6 Cross-platform — pure Rust byte builder, hiçbir external tool
//!   T8 Drift fail simulation — synthetic violation builders deliberate
//!
//! Section 8 CR-10 + CR-11 fixtures dahil. Minimum ≥20 fixture.

mod elf_builder;

use elf_builder::{
    encode_addi, encode_csrrw, encode_ebreak, encode_ecall, encode_fadd_s, encode_flw,
    encode_jal, encode_mret, encode_sfence_vma, encode_wfi, write_instr, write_rvc,
    ElfBuilder, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHN_ABS, STT_FILE, STT_FUNC, STT_OBJECT,
};
use riscv_bin_verify::{verify_elf_bytes, Category};
use riscv_bin_verify::manifest::{KernelEntry, Manifest, RegionEntry, TaskEntry};

// ─── Manifest helper ──────────────────────────────────────────────

fn demo_manifest() -> Manifest {
    Manifest {
        kernel: KernelEntry {
            name: "sipahi".into(),
            version: "1.8.0".into(),
            reserved_size: 0x60_0000,
        },
        tasks: vec![TaskEntry {
            name: "demo".into(),
            task_id: 2,
            regions: vec![
                RegionEntry { name: "text".into(),   base: 0x80600000, size: 0x4000, perm: "RX".into() },
                RegionEntry { name: "rodata".into(), base: 0x80604000, size: 0x1000, perm: "R".into() },
                RegionEntry { name: "data".into(),   base: 0x80605000, size: 0x1000, perm: "RW".into() },
                RegionEntry { name: "stack".into(),  base: 0x80610000, size: 0x2000, perm: "RW".into() },
            ],
        }],
    }
}

fn build_minimal_text(addr: u64, instrs: Vec<u32>) -> ElfBuilder {
    let mut elf = ElfBuilder::new_exec();
    let mut code = Vec::new();
    for raw in instrs { write_instr(&mut code, raw); }
    let text_idx = elf.add_text(addr, code);
    elf.add_symbol("_start", addr, text_idx, STT_FUNC);
    elf
}

fn assert_pass(report: &riscv_bin_verify::VerifyReport, fixture: &str) {
    assert!(
        report.passed(),
        "{fixture}: expected PASS, got {} violations: {:?}",
        report.violations.len(),
        report.violations
    );
}

fn assert_fails_with(
    report: &riscv_bin_verify::VerifyReport,
    fixture: &str,
    category: Category,
    msg_fragment: &str,
) {
    assert!(
        !report.passed(),
        "{fixture}: expected FAIL but PASS (no violations)"
    );
    assert!(
        report.violations.iter().any(|v| v.category == category
            && v.message.contains(msg_fragment)),
        "{fixture}: no violation with category={:?} containing '{}' — got: {:?}",
        category, msg_fragment, report.violations
    );
}

// ─── PASS cases (positive baseline) ────────────────────────────────

#[test]
fn fixture_01_plain_addi_pass() {
    // Simplest valid program: 2 addi instructions in .text.
    let elf = build_minimal_text(0x80600000, vec![
        encode_addi(10, 10, 1),
        encode_addi(10, 10, 1),
    ]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "plain_addi");
}

#[test]
fn fixture_02_ecall_allowed_pass() {
    // CR-10: ecall is the task syscall API — ALLOW.
    let elf = build_minimal_text(0x80600000, vec![
        encode_addi(17, 0, 3),  // li a7, 3 (SYS_YIELD)
        encode_ecall(),
        encode_addi(10, 10, 1),
    ]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "ecall_allowed");
}

#[test]
fn fixture_03_jal_to_task_region_pass() {
    // jal to addr 0x80600008 (within demo task .text) — ALLOW.
    let elf = build_minimal_text(0x80600000, vec![
        encode_jal(1, 0x8),
        encode_addi(0, 0, 0),
        encode_addi(0, 0, 0),
    ]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "jal_to_task_region");
}

// ─── PrivilegedOp REJECT (CR-10) ───────────────────────────────────

#[test]
fn fixture_04_ebreak_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_ebreak()]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "ebreak", Category::PrivilegedOp, "ebreak");
}

#[test]
fn fixture_05_csrrw_forbidden() {
    // csrrw a0, mstatus(0x300), t0 — privileged CSR write.
    let elf = build_minimal_text(0x80600000, vec![encode_csrrw(10, 5, 0x300)]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "csrrw", Category::PrivilegedOp, "csr");
}

#[test]
fn fixture_06_mret_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_mret()]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "mret", Category::PrivilegedOp, "mret");
}

#[test]
fn fixture_07_wfi_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_wfi()]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "wfi", Category::PrivilegedOp, "wfi");
}

#[test]
fn fixture_08_sfence_vma_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_sfence_vma()]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "sfence.vma", Category::PrivilegedOp, "sfence");
}

// ─── FloatOp REJECT (CR-10) ────────────────────────────────────────

#[test]
fn fixture_09_fadd_s_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_fadd_s()]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "fadd.s", Category::FloatOp, "F-extension");
}

#[test]
fn fixture_10_flw_forbidden() {
    let elf = build_minimal_text(0x80600000, vec![encode_flw(0, 10)]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "flw", Category::FloatOp, "F-extension");
}

#[test]
fn fixture_11_compressed_fld_forbidden() {
    // c.fld (Q0, funct3=0b001) — 0b001_000_000_00_000_00 = 0x2000.
    let mut elf = ElfBuilder::new_exec();
    let mut code = Vec::new();
    write_rvc(&mut code, 0x2000);
    let text_idx = elf.add_text(0x80600000, code);
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "c.fld", Category::FloatOp, "RVC FP");
}

#[test]
fn fixture_12_compressed_fsdsp_forbidden() {
    // c.fsdsp (Q2, funct3=0b101) — 0b101_000000_00000_10 = 0xA002.
    let mut elf = ElfBuilder::new_exec();
    let mut code = Vec::new();
    write_rvc(&mut code, 0xA002);
    let text_idx = elf.add_text(0x80600000, code);
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "c.fsdsp", Category::FloatOp, "RVC FP");
}

// ─── ForbiddenSection ──────────────────────────────────────────────

#[test]
fn fixture_13_init_array_forbidden() {
    let mut elf = ElfBuilder::new_exec();
    let text_idx = elf.add_text(0x80600000, encode_addi(0, 0, 0).to_le_bytes().to_vec());
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    elf.add_section(".init_array", 1, SHF_ALLOC | SHF_WRITE, 0x80605000, vec![0u8; 8]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "init_array", Category::ForbiddenSection, ".init_array");
}

#[test]
fn fixture_14_got_forbidden() {
    let mut elf = ElfBuilder::new_exec();
    let text_idx = elf.add_text(0x80600000, encode_addi(0, 0, 0).to_le_bytes().to_vec());
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    elf.add_section(".got", 1, SHF_ALLOC | SHF_WRITE, 0x80605000, vec![0u8; 8]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "got", Category::ForbiddenSection, ".got");
}

// ─── WxViolation ───────────────────────────────────────────────────

#[test]
fn fixture_15_wx_violation() {
    let mut elf = ElfBuilder::new_exec();
    let text_idx = elf.add_text(0x80600000, encode_addi(0, 0, 0).to_le_bytes().to_vec());
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    // Custom section with SHF_WRITE + SHF_EXECINSTR.
    elf.add_section(
        ".rwx_evil", 1, SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR,
        0x80605000, vec![0u8; 8]
    );
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "wx", Category::WxViolation, "SHF_WRITE + SHF_EXECINSTR");
}

// ─── Relocation / PIE ──────────────────────────────────────────────

#[test]
fn fixture_16_pie_rejected() {
    let mut elf = ElfBuilder::new_pie();
    let text_idx = elf.add_text(0x80600000, encode_addi(0, 0, 0).to_le_bytes().to_vec());
    elf.add_symbol("_start", 0x80600000, text_idx, STT_FUNC);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "pie", Category::Relocation, "ET_DYN");
}

// ─── RegionBoundary (CR-11) ────────────────────────────────────────

#[test]
fn fixture_17_symbol_oob_rejected() {
    let mut elf = build_minimal_text(0x80600000, vec![encode_addi(0, 0, 0)]);
    // Defined function symbol at completely out-of-region address.
    let alloc_data_idx = elf.add_section(".rodata", 1, SHF_ALLOC, 0x80604000, vec![0u8; 0x10]);
    elf.add_symbol("rogue_data", 0x80999999, alloc_data_idx, STT_OBJECT);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "oob_symbol", Category::RegionBoundary, "rogue_data");
}

#[test]
fn fixture_18_debug_file_symbol_ignored() {
    // CR-11: STT_FILE symbol must be filtered out, even at random address.
    let mut elf = build_minimal_text(0x80600000, vec![encode_addi(0, 0, 0)]);
    elf.add_symbol("source.rs", 0xDEAD_BEEF, SHN_ABS, STT_FILE);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "debug_file_symbol_filter");
}

#[test]
fn fixture_19_edge_symbol_at_region_end_pass() {
    // CR-11: linker-emitted edge symbols (_etext, _end, etc.) — region
    // boundary equality should not trip false positive.
    let mut elf = build_minimal_text(0x80600000, vec![encode_addi(0, 0, 0)]);
    let alloc_data_idx = elf.add_section(".rodata", 1, SHF_ALLOC, 0x80604000, vec![0u8; 0x10]);
    // _etext at exactly region.end = 0x80600000 + 0x4000 = 0x80604000 (would be off-by-one trip).
    elf.add_symbol("_etext", 0x80604000, alloc_data_idx, STT_OBJECT);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "edge_symbol_filter");
}

#[test]
fn fixture_20_abs_symbol_ignored() {
    // CR-11: SHN_ABS symbol with bogus address — filter must SKIP region check.
    let mut elf = build_minimal_text(0x80600000, vec![encode_addi(0, 0, 0)]);
    elf.add_symbol("ABS_CONST", 0x4242, SHN_ABS, STT_OBJECT);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_pass(&report, "abs_symbol_filter");
}

// ─── KernelRangeJal (FIX-A) ────────────────────────────────────────

#[test]
fn fixture_21_jal_kernel_range_rejected() {
    // jal from 0x80600000 with imm = -0x100000 → target = 0x80500000.
    // 0x80500000 is in kernel range [0x80000000, 0x80600000) (reserved_size=6MB).
    // JAL has ±1MB range; -0x100000 = -1MB = JAL edge negative.
    let elf = build_minimal_text(0x80600000, vec![encode_jal(1, -0x100000)]);
    let bytes = elf.build();
    let report = verify_elf_bytes(&bytes, &demo_manifest(), "demo").unwrap();
    assert_fails_with(&report, "jal_kernel", Category::KernelRangeJal, "kernel range");
}
