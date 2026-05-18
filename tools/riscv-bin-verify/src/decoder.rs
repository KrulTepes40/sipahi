//! RV64IMAC instruction decoder — 32-bit + RVC 16-bit.
//!
//! SAFE-3 (sprint-u32, Section 8 CR-10): Forbidden opcode reject precision.
//! `ecall` ALLOW (task API zorunlu), `ebreak` REJECT, CSR/mret/sret/sfence/wfi
//! REJECT, F/D 32-bit + compressed FP (c.fld/c.fsd/c.flw/c.fsw/c.fldsp/...)
//! REJECT.
//!
//! Reference: RISC-V Unprivileged ISA Manual v2.2, Privileged Manual v1.12,
//! Compressed ISA Ch 16.

/// Decoded instruction category — opcodes.rs forbidden lookup için.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `ecall` — task syscall API, ALLOW (Section 8 CR-10).
    Ecall,
    /// `ebreak` — debug breakpoint, REJECT.
    Ebreak,
    /// CSR access: csrrw, csrrs, csrrc, csrrwi, csrrsi, csrrci. REJECT.
    Csr,
    /// `mret` — M-mode return. REJECT.
    Mret,
    /// `sret` — S-mode return. REJECT.
    Sret,
    /// `uret` — U-mode return (deprecated). REJECT.
    Uret,
    /// `wfi` — wait for interrupt (M/S-mode). REJECT.
    Wfi,
    /// `sfence.vma` or `sfence.w.inval`. REJECT.
    SfenceVma,
    SfenceWInval,
    /// F-extension single-precision (flw/fsw/fadd.s/...). REJECT.
    FloatF,
    /// D-extension double-precision (fld/fsd/fadd.d/...). REJECT.
    FloatD,
    /// Compressed FP load/store (c.flw/c.fsw/c.fld/c.fsd/c.fldsp/c.fsdsp/
    /// c.flwsp/c.fswsp). REJECT.
    CompressedFloat,
    /// `jal` immediate — verifier kernel-range target reject (G4).
    Jal { target: u64 },
    /// `jalr` — indirect call. Section 4 FIX-A: register-tracked best-effort
    /// warning, not REJECT in v1.8.
    Jalr,
    /// Plain ALU / load / store / branch — ALLOW.
    Plain,
    /// Unknown opcode or malformed encoding.
    Unknown,
}

/// 32-bit instruction decode. `pc` is the absolute address for jal target
/// calculation.
pub fn decode32(raw: u32, pc: u64) -> Op {
    let opcode = raw & 0x7F;
    match opcode {
        // ── SYSTEM (0x73) — funct12 + funct3 ile alt-instruction parse ──
        0x73 => decode_system(raw),
        // ── OP-FP (0x53) — F + D float arithmetic ──
        0x53 => decode_op_fp(raw),
        // ── LOAD-FP (0x07) — flw/fld ──
        0x07 => decode_load_fp(raw),
        // ── STORE-FP (0x27) — fsw/fsd ──
        0x27 => decode_store_fp(raw),
        // ── JAL (0x6F) — immediate target ──
        0x6F => {
            let target = decode_jal_target(raw, pc);
            Op::Jal { target }
        }
        // ── JALR (0x67) — indirect ──
        0x67 => Op::Jalr,
        _ => Op::Plain,
    }
}

fn decode_system(raw: u32) -> Op {
    let funct3 = (raw >> 12) & 0x7;
    if funct3 == 0 {
        // System (PRIV/TRAP) — distinguish via funct12 + rs1 + rd.
        let funct12 = (raw >> 20) & 0xFFF;
        let rs1     = (raw >> 15) & 0x1F;
        let rd      = (raw >> 7)  & 0x1F;
        if rs1 != 0 || rd != 0 {
            // funct3=0 with non-zero rs1/rd is reserved / malformed.
            return Op::Unknown;
        }
        return match funct12 {
            0x000 => Op::Ecall,
            0x001 => Op::Ebreak,
            0x102 => Op::Sret,
            0x202 => Op::Uret,
            0x302 => Op::Mret,
            0x105 => Op::Wfi,
            // SFENCE.VMA = funct7=0x09, rs2/rs1 arbitrary; funct12 = 0x009..
            // SFENCE.W.INVAL = funct12=0x180 (Zicbom?). For SAFE-3 we use
            // funct7-based detection below — funct3==0 with high bits
            // matching sfence.
            _ => {
                // SFENCE.VMA family: funct7 == 0x09 (i.e. funct12 high 7 bits)
                let funct7 = (funct12 >> 5) & 0x7F;
                if funct7 == 0x09 {
                    Op::SfenceVma
                } else if funct7 == 0x0C {
                    // SFENCE.W.INVAL (proposed Zifencei extension)
                    Op::SfenceWInval
                } else {
                    Op::Unknown
                }
            }
        };
    }
    // funct3 1..7 → CSR family (csrrw=1, csrrs=2, csrrc=3, csrrwi=5,
    // csrrsi=6, csrrci=7). funct3=4 unused.
    match funct3 {
        1 | 2 | 3 | 5 | 6 | 7 => Op::Csr,
        _ => Op::Unknown,
    }
}

fn decode_op_fp(_raw: u32) -> Op {
    // OP-FP (0x53) — F + D + Q extension arithmetic.
    // funct7[1:0] = 0b00 → F (single), 0b01 → D (double), 0b10 → H (half),
    // 0b11 → Q (quad). RV64IMAC izinli olduğundan F/D/H/Q hepsi REJECT.
    let funct7 = (_raw >> 25) & 0x7F;
    let fmt    = funct7 & 0b11;
    match fmt {
        0b00 => Op::FloatF,
        0b01 => Op::FloatD,
        _    => Op::FloatF,  // H/Q — F sınıfında işle (yine REJECT)
    }
}

fn decode_load_fp(_raw: u32) -> Op {
    // LOAD-FP (0x07) — width: funct3 (2 = flw, 3 = fld, 4 = flq, 1 = flh)
    let funct3 = (_raw >> 12) & 0x7;
    match funct3 {
        2 => Op::FloatF,   // flw
        3 => Op::FloatD,   // fld
        _ => Op::FloatF,   // flh/flq — F sınıfı REJECT
    }
}

fn decode_store_fp(_raw: u32) -> Op {
    // STORE-FP (0x27) — width: funct3 (2 = fsw, 3 = fsd, 4 = fsq, 1 = fsh)
    let funct3 = (_raw >> 12) & 0x7;
    match funct3 {
        2 => Op::FloatF,
        3 => Op::FloatD,
        _ => Op::FloatF,
    }
}

fn decode_jal_target(raw: u32, pc: u64) -> u64 {
    // JAL immediate decode (RV64I, sign-extended):
    //   imm[20|10:1|11|19:12] = raw[31|30:21|20|19:12]
    let imm20    = ((raw >> 31) & 0x1) << 20;
    let imm10_1  = ((raw >> 21) & 0x3FF) << 1;
    let imm11    = ((raw >> 20) & 0x1) << 11;
    let imm19_12 = ((raw >> 12) & 0xFF) << 12;
    let imm_unsigned = imm20 | imm19_12 | imm11 | imm10_1;
    // Sign-extend 21-bit immediate to i64.
    let imm = if imm20 != 0 {
        (imm_unsigned | 0xFFE0_0000) as i32 as i64
    } else {
        imm_unsigned as i64
    };
    (pc as i64).wrapping_add(imm) as u64
}

/// 16-bit RVC decode — compressed FP load/store family REJECT (CR-10).
///
/// Reference: RV Compressed ISA Manual v1.0 Ch 16. Major class field
/// = bits[1:0] (quadrant), funct3 = bits[15:13].
pub fn decode_rvc(raw: u16) -> Op {
    // RV64IMAC RVC encoding map. RV32 vs RV64 ambiguity exists for two
    // pairs — verifier targets RV64IMAC explicitly (rust-toolchain.toml +
    // sipahi.toml platform.target = riscv64imac), so funct3=011/111 in
    // Q0/Q2 = integer load/store (c.ld/c.sd/c.ldsp/c.sdsp) ALLOW.
    let quadrant = raw & 0b11;
    let funct3   = (raw >> 13) & 0x7;
    match (quadrant, funct3) {
        // ── Quadrant 0 (Q0) ──
        (0b00, 0b001) => Op::CompressedFloat, // c.fld     (RV64 D-ext only)
        (0b00, 0b101) => Op::CompressedFloat, // c.fsd     (RV64 D-ext only)
        // c.ld (0b011) / c.sd (0b111) RV64 integer → ALLOW
        // ── Quadrant 2 (Q2, SP-relative) ──
        (0b10, 0b001) => Op::CompressedFloat, // c.fldsp   (D-ext)
        (0b10, 0b101) => Op::CompressedFloat, // c.fsdsp   (D-ext)
        // c.ldsp (0b011) / c.sdsp (0b111) RV64 integer → ALLOW
        _ => Op::Plain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_ecall() {
        // ecall = 0x00000073
        assert_eq!(decode32(0x0000_0073, 0x1000), Op::Ecall);
    }

    #[test]
    fn decode_ebreak() {
        // ebreak = 0x00100073
        assert_eq!(decode32(0x0010_0073, 0x1000), Op::Ebreak);
    }

    #[test]
    fn decode_csrrw() {
        // csrrw a0, mstatus, t0 — funct3=1, csr=0x300, rs1=5, rd=10
        let raw = (0x300 << 20) | (5 << 15) | (1 << 12) | (10 << 7) | 0x73;
        assert_eq!(decode32(raw, 0x1000), Op::Csr);
    }

    #[test]
    fn decode_mret() {
        // mret = 0x30200073
        assert_eq!(decode32(0x3020_0073, 0x1000), Op::Mret);
    }

    #[test]
    fn decode_wfi() {
        // wfi = 0x10500073
        assert_eq!(decode32(0x1050_0073, 0x1000), Op::Wfi);
    }

    #[test]
    fn decode_sfence_vma() {
        // sfence.vma = 0x12000073 (funct7=0x09, funct3=0, rs1=rs2=rd=0)
        assert_eq!(decode32(0x1200_0073, 0x1000), Op::SfenceVma);
    }

    #[test]
    fn decode_fadd_s() {
        // fadd.s f0, f1, f2 — OP-FP, funct7=0x00 (fadd, fmt=S)
        let raw = (0x00 << 25) | (2 << 20) | (1 << 15) | (0 << 7) | 0x53;
        assert_eq!(decode32(raw, 0x1000), Op::FloatF);
    }

    #[test]
    fn decode_flw() {
        // flw f0, 0(a0) — LOAD-FP, funct3=2
        let raw = (10 << 15) | (2 << 12) | (0 << 7) | 0x07;
        assert_eq!(decode32(raw, 0x1000), Op::FloatF);
    }

    #[test]
    fn decode_fsd() {
        // fsd f1, 0(a0) — STORE-FP, funct3=3
        let raw = (1 << 20) | (10 << 15) | (3 << 12) | 0x27;
        assert_eq!(decode32(raw, 0x1000), Op::FloatD);
    }

    #[test]
    fn decode_plain_addi() {
        // addi a0, a0, 1 — opcode 0x13
        let raw = (1 << 20) | (10 << 15) | (0 << 12) | (10 << 7) | 0x13;
        assert_eq!(decode32(raw, 0x1000), Op::Plain);
    }

    #[test]
    fn decode_jal_pc_relative() {
        // jal x0, +8 → imm = 8 → bits encoded
        let imm = 8u32;
        let imm20    = (imm >> 20) & 0x1;
        let imm10_1  = (imm >> 1) & 0x3FF;
        let imm11    = (imm >> 11) & 0x1;
        let imm19_12 = (imm >> 12) & 0xFF;
        let raw = (imm20 << 31) | (imm10_1 << 21) | (imm11 << 20) | (imm19_12 << 12) | 0x6F;
        let op = decode32(raw, 0x1000);
        match op {
            Op::Jal { target } => assert_eq!(target, 0x1008),
            other => panic!("expected Jal, got {:?}", other),
        }
    }

    #[test]
    fn decode_rvc_fld() {
        // c.fld (Q0, funct3=0b001) — 0x_0_001_xxxxx_xxxxx_00 = 0x2000 base
        let raw: u16 = 0x2000;
        assert_eq!(decode_rvc(raw), Op::CompressedFloat);
    }

    #[test]
    fn decode_rvc_fsdsp() {
        // c.fsdsp (Q2, funct3=0b101) — 0b101_xxxxxxxxx_10 = 0xA002 base
        let raw: u16 = 0xA002;
        assert_eq!(decode_rvc(raw), Op::CompressedFloat);
    }

    #[test]
    fn decode_rvc_ld_rv64_allowed() {
        // c.ld (Q0, funct3=0b011) — RV64 integer load, ALLOW.
        // In RV32 this same encoding is c.flw (FP); verifier targets RV64.
        let raw: u16 = 0b011_000_000_00_000_00; // funct3=011, quadrant=00
        assert_eq!(decode_rvc(raw), Op::Plain);
    }

    #[test]
    fn decode_rvc_sd_rv64_allowed() {
        // c.sd (Q0, funct3=0b111) — RV64 integer store, ALLOW.
        let raw: u16 = 0b111_000_000_00_000_00;
        assert_eq!(decode_rvc(raw), Op::Plain);
    }

    #[test]
    fn decode_rvc_ldsp_rv64_allowed() {
        // c.ldsp (Q2, funct3=0b011) — RV64 SP-relative load, ALLOW.
        let raw: u16 = 0b011_0_00000_00000_10;
        assert_eq!(decode_rvc(raw), Op::Plain);
    }

    #[test]
    fn decode_rvc_sdsp_rv64_allowed() {
        // c.sdsp (Q2, funct3=0b111) — RV64 SP-relative store, ALLOW.
        let raw: u16 = 0b111_000000_00000_10;
        assert_eq!(decode_rvc(raw), Op::Plain);
    }

    #[test]
    fn decode_rvc_plain_addi4spn() {
        // c.addi4spn (Q0, funct3=0b000) — plain, ALLOW
        let raw: u16 = 0x0040; // some non-zero immediate
        assert_eq!(decode_rvc(raw), Op::Plain);
    }
}
