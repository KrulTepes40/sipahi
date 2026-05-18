//! Minimal RV64 ELF byte builder for verifier integration tests.
//!
//! Section 8 FIX-H notu: pre-built fixture binaries daha sağlam ama toolchain
//! (riscv64-unknown-elf-as) bağımlılığı sprint scope'unu büyütür. Bu builder
//! sadece test'lerde kullanılır — production code path'i değil. Section 9.2 T3
//! "realistic fixture" doctrine için minimal valid ELF subset üretir
//! (single .text section + 1 symbol table + optional sections).

#![allow(dead_code)]

use std::io::Write;

const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ELFOSABI_NONE: u8 = 0;
const EM_RISCV: u16 = 243;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;

pub const SHF_WRITE: u64     = 0x1;
pub const SHF_ALLOC: u64     = 0x2;
pub const SHF_EXECINSTR: u64 = 0x4;

const SHT_NULL: u32     = 0;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32   = 2;
const SHT_STRTAB: u32   = 3;
const SHT_NOBITS: u32   = 8;
const SHT_RELA: u32     = 4;

pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16   = 0xFFF1;

pub const STT_NOTYPE: u8  = 0;
pub const STT_OBJECT: u8  = 1;
pub const STT_FUNC: u8    = 2;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8    = 4;
const STB_LOCAL: u8       = 0;
const STB_GLOBAL: u8      = 1;

pub struct ElfBuilder {
    pub e_type:    u16,
    pub entry:     u64,
    pub sections:  Vec<Section>,
    pub symbols:   Vec<Symbol>,
}

pub struct Section {
    pub name:    String,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub data:    Vec<u8>,
}

pub struct Symbol {
    pub name:     String,
    pub value:    u64,
    pub shndx:    u16,    // 1-based section index, or SHN_ABS/SHN_UNDEF
    pub st_type:  u8,
    pub binding:  u8,
}

impl ElfBuilder {
    pub fn new_exec() -> Self {
        Self {
            e_type: ET_EXEC,
            entry: 0x80600000,
            sections: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn new_pie() -> Self {
        let mut e = Self::new_exec();
        e.e_type = ET_DYN;
        e
    }

    pub fn add_text(&mut self, addr: u64, code: Vec<u8>) -> u16 {
        self.sections.push(Section {
            name: ".text".into(),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_EXECINSTR,
            sh_addr: addr,
            data: code,
        });
        self.sections.len() as u16
    }

    pub fn add_section(&mut self, name: &str, sh_type: u32, flags: u64, addr: u64, data: Vec<u8>) -> u16 {
        self.sections.push(Section {
            name: name.into(),
            sh_type,
            sh_flags: flags,
            sh_addr: addr,
            data,
        });
        self.sections.len() as u16
    }

    pub fn add_symbol(&mut self, name: &str, value: u64, shndx: u16, st_type: u8) {
        self.symbols.push(Symbol {
            name: name.into(),
            value,
            shndx,
            st_type,
            binding: STB_GLOBAL,
        });
    }

    pub fn build(&self) -> Vec<u8> {
        // ELF64 header is 64 bytes. Section headers follow data.
        // Layout:
        //   [0..64) ELF header
        //   section data (concatenated, aligned to 8 byte)
        //   shstrtab (section names)
        //   strtab (symbol names)
        //   symtab
        //   section header table

        // Build shstrtab + strtab first to know offsets.
        let mut shstrtab = vec![0u8]; // index 0 = empty string
        let mut shstr_off: Vec<u32> = Vec::with_capacity(self.sections.len());
        for sec in &self.sections {
            shstr_off.push(shstrtab.len() as u32);
            shstrtab.extend_from_slice(sec.name.as_bytes());
            shstrtab.push(0);
        }
        // .shstrtab itself name
        let shstrtab_name_off = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".shstrtab\0");
        let symtab_name_off = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".symtab\0");
        let strtab_name_off = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".strtab\0");

        let mut strtab = vec![0u8];
        let mut sym_name_off: Vec<u32> = Vec::with_capacity(self.symbols.len() + 1);
        sym_name_off.push(0); // null symbol
        for sym in &self.symbols {
            sym_name_off.push(strtab.len() as u32);
            strtab.extend_from_slice(sym.name.as_bytes());
            strtab.push(0);
        }

        // Allocate section data offsets in file.
        let mut out: Vec<u8> = Vec::new();
        out.resize(64, 0); // header placeholder

        let mut sec_file_off: Vec<u64> = Vec::with_capacity(self.sections.len() + 4);

        // Add user sections.
        for sec in &self.sections {
            align_to(&mut out, 8);
            sec_file_off.push(out.len() as u64);
            if sec.sh_type != SHT_NOBITS {
                out.write_all(&sec.data).unwrap();
            }
        }
        // Add .shstrtab
        align_to(&mut out, 1);
        let shstrtab_off = out.len() as u64;
        out.write_all(&shstrtab).unwrap();
        // Add .strtab
        align_to(&mut out, 1);
        let strtab_off = out.len() as u64;
        out.write_all(&strtab).unwrap();
        // Add .symtab
        align_to(&mut out, 8);
        let symtab_off = out.len() as u64;
        // Null symbol (16 bytes for ELF64 Sym: name, info, other, shndx, value, size)
        // ELF64_Sym = { st_name:4, st_info:1, st_other:1, st_shndx:2, st_value:8, st_size:8 } = 24 bytes
        out.extend_from_slice(&[0u8; 24]);
        for (i, sym) in self.symbols.iter().enumerate() {
            let st_name = sym_name_off[i + 1].to_le_bytes();
            let st_info = (sym.binding << 4) | (sym.st_type & 0xF);
            let st_other = 0u8;
            let st_shndx = sym.shndx.to_le_bytes();
            let st_value = sym.value.to_le_bytes();
            let st_size  = 0u64.to_le_bytes();
            out.extend_from_slice(&st_name);
            out.push(st_info);
            out.push(st_other);
            out.extend_from_slice(&st_shndx);
            out.extend_from_slice(&st_value);
            out.extend_from_slice(&st_size);
        }
        let symtab_size = 24 + 24 * self.symbols.len() as u64;

        // Section header table — start aligned
        align_to(&mut out, 8);
        let shoff = out.len() as u64;

        // ELF64 Section header = 64 bytes (sh_name, sh_type, sh_flags, sh_addr,
        // sh_offset, sh_size, sh_link, sh_info, sh_addralign, sh_entsize)

        // [0] null section
        out.extend_from_slice(&[0u8; 64]);

        // user sections (1..=N)
        for (i, sec) in self.sections.iter().enumerate() {
            let mut sh = [0u8; 64];
            sh[0..4].copy_from_slice(&shstr_off[i].to_le_bytes());
            sh[4..8].copy_from_slice(&sec.sh_type.to_le_bytes());
            sh[8..16].copy_from_slice(&sec.sh_flags.to_le_bytes());
            sh[16..24].copy_from_slice(&sec.sh_addr.to_le_bytes());
            sh[24..32].copy_from_slice(&sec_file_off[i].to_le_bytes());
            sh[32..40].copy_from_slice(&(sec.data.len() as u64).to_le_bytes());
            sh[40..44].copy_from_slice(&0u32.to_le_bytes()); // sh_link
            sh[44..48].copy_from_slice(&0u32.to_le_bytes()); // sh_info
            sh[48..56].copy_from_slice(&1u64.to_le_bytes()); // sh_addralign
            sh[56..64].copy_from_slice(&0u64.to_le_bytes()); // sh_entsize
            out.extend_from_slice(&sh);
        }

        let shstrtab_idx  = self.sections.len() as u16 + 1;
        let strtab_idx    = self.sections.len() as u16 + 2;
        let symtab_idx    = self.sections.len() as u16 + 3;

        // .shstrtab section header
        let mut sh = [0u8; 64];
        sh[0..4].copy_from_slice(&shstrtab_name_off.to_le_bytes());
        sh[4..8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
        sh[24..32].copy_from_slice(&shstrtab_off.to_le_bytes());
        sh[32..40].copy_from_slice(&(shstrtab.len() as u64).to_le_bytes());
        sh[48..56].copy_from_slice(&1u64.to_le_bytes());
        out.extend_from_slice(&sh);

        // .strtab section header
        let mut sh = [0u8; 64];
        sh[0..4].copy_from_slice(&strtab_name_off.to_le_bytes());
        sh[4..8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
        sh[24..32].copy_from_slice(&strtab_off.to_le_bytes());
        sh[32..40].copy_from_slice(&(strtab.len() as u64).to_le_bytes());
        sh[48..56].copy_from_slice(&1u64.to_le_bytes());
        out.extend_from_slice(&sh);

        // .symtab section header
        let mut sh = [0u8; 64];
        sh[0..4].copy_from_slice(&symtab_name_off.to_le_bytes());
        sh[4..8].copy_from_slice(&SHT_SYMTAB.to_le_bytes());
        sh[24..32].copy_from_slice(&symtab_off.to_le_bytes());
        sh[32..40].copy_from_slice(&symtab_size.to_le_bytes());
        sh[40..44].copy_from_slice(&(strtab_idx as u32).to_le_bytes()); // sh_link → strtab
        sh[44..48].copy_from_slice(&1u32.to_le_bytes()); // sh_info = first non-local index
        sh[48..56].copy_from_slice(&8u64.to_le_bytes()); // sh_addralign
        sh[56..64].copy_from_slice(&24u64.to_le_bytes()); // sh_entsize
        out.extend_from_slice(&sh);

        let shnum = self.sections.len() as u16 + 4;
        let shstrndx = shstrtab_idx;

        // Fill ELF header at offset 0
        let header = build_elf_header(self.e_type, self.entry, shoff, shnum, shstrndx);
        out[..64].copy_from_slice(&header);

        out
    }
}

fn build_elf_header(e_type: u16, entry: u64, shoff: u64, shnum: u16, shstrndx: u16) -> [u8; 64] {
    let mut h = [0u8; 64];
    // e_ident
    h[0..4].copy_from_slice(b"\x7fELF");
    h[4] = ELFCLASS64;
    h[5] = ELFDATA2LSB;
    h[6] = EV_CURRENT;
    h[7] = ELFOSABI_NONE;
    // h[8..16] = pad
    h[16..18].copy_from_slice(&e_type.to_le_bytes());
    h[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
    h[20..24].copy_from_slice(&(EV_CURRENT as u32).to_le_bytes());
    h[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
    h[32..40].copy_from_slice(&0u64.to_le_bytes()); // e_phoff (no program header)
    h[40..48].copy_from_slice(&shoff.to_le_bytes()); // e_shoff
    h[48..52].copy_from_slice(&0u32.to_le_bytes()); // e_flags
    h[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
    h[54..56].copy_from_slice(&0u16.to_le_bytes()); // e_phentsize
    h[56..58].copy_from_slice(&0u16.to_le_bytes()); // e_phnum
    h[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
    h[60..62].copy_from_slice(&shnum.to_le_bytes()); // e_shnum
    h[62..64].copy_from_slice(&shstrndx.to_le_bytes()); // e_shstrndx
    h
}

fn align_to(out: &mut Vec<u8>, align: usize) {
    while out.len() % align != 0 {
        out.push(0);
    }
}

// Instruction encoders used across fixtures.
pub fn encode_addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm = (imm as u32) & 0xFFF;
    (imm << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x13
}

pub fn encode_ecall() -> u32 { 0x0000_0073 }
pub fn encode_ebreak() -> u32 { 0x0010_0073 }
pub fn encode_mret() -> u32 { 0x3020_0073 }
pub fn encode_wfi() -> u32 { 0x1050_0073 }
pub fn encode_sfence_vma() -> u32 { 0x1200_0073 }
pub fn encode_csrrw(rd: u32, rs1: u32, csr: u32) -> u32 {
    ((csr & 0xFFF) << 20) | (rs1 << 15) | (1 << 12) | (rd << 7) | 0x73
}
pub fn encode_fadd_s() -> u32 {
    // fadd.s f0, f1, f2: funct7=0x00 fmt=00 → 0x00_2_1_0_53 = 0x002150
    (0x00 << 25) | (2 << 20) | (1 << 15) | (0 << 12) | (0 << 7) | 0x53
}
pub fn encode_flw(rd: u32, rs1: u32) -> u32 {
    (0 << 20) | (rs1 << 15) | (2 << 12) | (rd << 7) | 0x07
}
pub fn encode_jal(rd: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    let imm20    = (imm >> 20) & 0x1;
    let imm10_1  = (imm >> 1) & 0x3FF;
    let imm11    = (imm >> 11) & 0x1;
    let imm19_12 = (imm >> 12) & 0xFF;
    (imm20 << 31) | (imm10_1 << 21) | (imm11 << 20) | (imm19_12 << 12) | (rd << 7) | 0x6F
}

pub fn write_instr(buf: &mut Vec<u8>, raw: u32) {
    buf.extend_from_slice(&raw.to_le_bytes());
}

pub fn write_rvc(buf: &mut Vec<u8>, raw: u16) {
    buf.extend_from_slice(&raw.to_le_bytes());
}
