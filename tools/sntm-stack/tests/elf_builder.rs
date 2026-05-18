//! Minimal RV64 ELF builder for sntm-stack integration tests.
//!
//! Section 9.2 T3 doctrine (realistic fixture): real linked-style ELF üretir,
//! `.text` + `.stack_sizes` + `.symtab` + `.strtab` + `.shstrtab`. SAFE-4
//! Plan B parser tarafı son-uç doğrulanır (synthetic ELF → object crate
//! parse → analiz).

#![allow(dead_code)]

use std::io::Write;

const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const EM_RISCV: u16 = 243;
const ET_EXEC: u16 = 2;

const SHF_ALLOC: u64     = 0x2;
const SHF_EXECINSTR: u64 = 0x4;

const SHT_NULL: u32     = 0;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32   = 2;
const SHT_STRTAB: u32   = 3;

const STT_FUNC: u8  = 2;
const STB_GLOBAL: u8 = 1;

pub struct Func {
    pub name:  String,
    pub addr:  u64,
    pub bytes: Vec<u8>,
    pub frame: u32,
}

pub fn build_elf(text_base: u64, funcs: &[Func]) -> Vec<u8> {
    // Compose .text bytes — each function's bytes packed sequentially at its
    // addr offset relative to text_base. Pad with NOPs between if needed.
    let mut text_bytes: Vec<u8> = Vec::new();
    let mut sorted: Vec<&Func> = funcs.iter().collect();
    sorted.sort_by_key(|f| f.addr);
    for f in &sorted {
        let want_off = (f.addr - text_base) as usize;
        while text_bytes.len() < want_off {
            // pad with 0x13 = addi x0, x0, 0 (nop, 32-bit)
            text_bytes.extend_from_slice(&0x13u32.to_le_bytes());
            if text_bytes.len() > want_off + 64 {
                panic!("padding overshoot");
            }
        }
        text_bytes.extend_from_slice(&f.bytes);
    }

    // Compose .stack_sizes — for each function: 8 byte LE addr + ULEB128 size.
    let mut ss = Vec::new();
    for f in funcs {
        ss.extend_from_slice(&f.addr.to_le_bytes());
        write_uleb128(&mut ss, f.frame);
    }

    // .shstrtab: section name strings.
    let mut shstrtab = vec![0u8];
    let push_name = |buf: &mut Vec<u8>, s: &str| -> u32 {
        let off = buf.len() as u32;
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
        off
    };
    let text_name_off = push_name(&mut shstrtab, ".text");
    let ss_name_off = push_name(&mut shstrtab, ".stack_sizes");
    let symtab_name_off = push_name(&mut shstrtab, ".symtab");
    let strtab_name_off = push_name(&mut shstrtab, ".strtab");
    let shstrtab_name_off = push_name(&mut shstrtab, ".shstrtab");

    // .strtab: symbol name strings.
    let mut strtab = vec![0u8];
    let mut sym_name_offs: Vec<u32> = Vec::new();
    for f in funcs {
        sym_name_offs.push(strtab.len() as u32);
        strtab.extend_from_slice(f.name.as_bytes());
        strtab.push(0);
    }

    // .symtab entries — ELF64_Sym (24 byte) — null then each function.
    let mut symtab = vec![0u8; 24];
    for (i, f) in funcs.iter().enumerate() {
        let mut e = [0u8; 24];
        e[0..4].copy_from_slice(&sym_name_offs[i].to_le_bytes());
        e[4] = (STB_GLOBAL << 4) | STT_FUNC; // st_info
        e[5] = 0; // st_other
        e[6..8].copy_from_slice(&1u16.to_le_bytes()); // st_shndx → .text (1)
        e[8..16].copy_from_slice(&f.addr.to_le_bytes());
        e[16..24].copy_from_slice(&(f.bytes.len() as u64).to_le_bytes());
        symtab.extend_from_slice(&e);
    }

    // Assemble file: ELF header + sections (data) + shstrtab + strtab + symtab + section header table.
    let mut out: Vec<u8> = Vec::new();
    out.resize(64, 0); // header placeholder

    let align = |out: &mut Vec<u8>, n: usize| while out.len() % n != 0 { out.push(0); };

    align(&mut out, 16);
    let text_off = out.len() as u64;
    out.write_all(&text_bytes).unwrap();

    align(&mut out, 8);
    let ss_off = out.len() as u64;
    out.write_all(&ss).unwrap();

    align(&mut out, 1);
    let shstrtab_off = out.len() as u64;
    out.write_all(&shstrtab).unwrap();

    align(&mut out, 1);
    let strtab_off = out.len() as u64;
    out.write_all(&strtab).unwrap();

    align(&mut out, 8);
    let symtab_off = out.len() as u64;
    out.write_all(&symtab).unwrap();

    align(&mut out, 8);
    let shoff = out.len() as u64;

    // Section header table — 6 entries: null, .text, .stack_sizes, .shstrtab, .strtab, .symtab.
    let mk_sh = |name: u32, sh_type: u32, flags: u64, addr: u64, offset: u64,
                 size: u64, link: u32, info: u32, addralign: u64, entsize: u64| -> [u8; 64]
    {
        let mut sh = [0u8; 64];
        sh[0..4].copy_from_slice(&name.to_le_bytes());
        sh[4..8].copy_from_slice(&sh_type.to_le_bytes());
        sh[8..16].copy_from_slice(&flags.to_le_bytes());
        sh[16..24].copy_from_slice(&addr.to_le_bytes());
        sh[24..32].copy_from_slice(&offset.to_le_bytes());
        sh[32..40].copy_from_slice(&size.to_le_bytes());
        sh[40..44].copy_from_slice(&link.to_le_bytes());
        sh[44..48].copy_from_slice(&info.to_le_bytes());
        sh[48..56].copy_from_slice(&addralign.to_le_bytes());
        sh[56..64].copy_from_slice(&entsize.to_le_bytes());
        sh
    };

    out.extend_from_slice(&[0u8; 64]); // null section
    out.extend_from_slice(&mk_sh(text_name_off, SHT_PROGBITS,
        SHF_ALLOC | SHF_EXECINSTR, text_base, text_off,
        text_bytes.len() as u64, 0, 0, 4, 0));
    out.extend_from_slice(&mk_sh(ss_name_off, SHT_PROGBITS, 0, 0, ss_off,
        ss.len() as u64, 0, 0, 1, 0));
    out.extend_from_slice(&mk_sh(shstrtab_name_off, SHT_STRTAB, 0, 0,
        shstrtab_off, shstrtab.len() as u64, 0, 0, 1, 0));
    out.extend_from_slice(&mk_sh(strtab_name_off, SHT_STRTAB, 0, 0,
        strtab_off, strtab.len() as u64, 0, 0, 1, 0));
    out.extend_from_slice(&mk_sh(symtab_name_off, SHT_SYMTAB, 0, 0,
        symtab_off, symtab.len() as u64, 4, 1, 8, 24)); // link=strtab idx 4

    let shnum: u16 = 6;
    let shstrndx: u16 = 3;

    // ELF header.
    let mut h = [0u8; 64];
    h[0..4].copy_from_slice(b"\x7fELF");
    h[4] = ELFCLASS64;
    h[5] = ELFDATA2LSB;
    h[6] = EV_CURRENT;
    h[7] = 0;
    h[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
    h[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
    h[20..24].copy_from_slice(&(EV_CURRENT as u32).to_le_bytes());
    h[24..32].copy_from_slice(&text_base.to_le_bytes()); // e_entry
    h[32..40].copy_from_slice(&0u64.to_le_bytes());      // e_phoff
    h[40..48].copy_from_slice(&shoff.to_le_bytes());     // e_shoff
    h[48..52].copy_from_slice(&0u32.to_le_bytes());      // e_flags
    h[52..54].copy_from_slice(&64u16.to_le_bytes());     // e_ehsize
    h[54..56].copy_from_slice(&0u16.to_le_bytes());      // e_phentsize
    h[56..58].copy_from_slice(&0u16.to_le_bytes());      // e_phnum
    h[58..60].copy_from_slice(&64u16.to_le_bytes());     // e_shentsize
    h[60..62].copy_from_slice(&shnum.to_le_bytes());     // e_shnum
    h[62..64].copy_from_slice(&shstrndx.to_le_bytes());  // e_shstrndx
    out[..64].copy_from_slice(&h);

    out
}

fn write_uleb128(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            return;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

// Instruction encoders for crafting .text bytes in tests.

pub fn encode_addi_nop() -> u32 { 0x13 } // addi x0, x0, 0
pub fn encode_ret() -> u32 {
    // jalr x0, x1, 0
    (1 << 15) | (0 << 7) | 0x67
}
pub fn encode_auipc(rd: u32, imm20: u32) -> u32 {
    ((imm20 & 0xFFFFF) << 12) | (rd << 7) | 0x17
}
pub fn encode_jalr(rd: u32, rs1: u32, imm12: i32) -> u32 {
    let imm = (imm12 as u32) & 0xFFF;
    (imm << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67
}
