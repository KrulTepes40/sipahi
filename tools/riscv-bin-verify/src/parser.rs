//! ELF parse — `object` crate üzerinden RV64 ELF dosyasını struct'lara serer.
//!
//! sntm-pack (tools/sntm-pack/src/pack.rs) ile aynı object 0.36.5 versiyonu;
//! ELF semantic değişimi riski yok. Verifier sadece read-only inspection
//! yapar (modify YOK).

use object::elf::{ET_DYN, ET_EXEC, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};
use object::read::elf::{ElfFile64, FileHeader, SectionHeader, Sym};
use object::{Endianness, Object, ObjectSection, ObjectSymbol};
use std::collections::BTreeMap;

/// In-memory ELF view — verifier'ın ihtiyaç duyduğu alanlar.
///
/// Owned data (Vec/String) — lib.rs API'sı `&[u8]` aldığı için lifetime
/// karmaşası önlenir. Verifier büyük binary'leri parse etmez; copy cost
/// önemsiz (task ELF tipik <2KB).
#[derive(Debug)]
pub struct ParsedElf {
    pub e_type:      u16,
    pub sections:    Vec<ParsedSection>,
    pub symbols:     Vec<ParsedSymbol>,
    /// (offset_in_text_section_bytes, raw 32 or 16 bit instr) — decoder
    /// G2'de bu listeyi gezecek. text section addr + offset = absolute addr.
    pub text_words:  Vec<TextInstr>,
    pub relocations: Vec<ParsedReloc>,
}

#[derive(Debug, Clone)]
pub struct ParsedSection {
    pub name:     String,
    pub sh_addr:  u64,
    pub sh_size:  u64,
    pub sh_flags: u64,
    /// True iff section name starts with `.text` family (.text, .text.*).
    pub is_text:  bool,
}

#[derive(Debug, Clone)]
pub struct ParsedSymbol {
    pub name:      String,
    pub st_value:  u64,
    pub st_shndx:  SymSection,
    pub st_type:   SymType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymSection {
    /// Refers to a normal section by index — verifier looks up sh_flags
    /// to decide if SHF_ALLOC (runtime-loaded).
    Index(u16),
    Undef,      // SHN_UNDEF — external reference
    Absolute,   // SHN_ABS — manifest region check'i ATLA
    Common,     // SHN_COMMON
    Reserved,   // diğer reserved indices
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymType {
    Func,
    Object,
    File,
    Section,
    NoType,
    Other,
}

/// Decoded instruction word for opcode scanning. Address = section.sh_addr +
/// offset. 32-bit instruction (`bytes[2..4]` == 0 if 16-bit RVC).
#[derive(Debug, Clone, Copy)]
pub struct TextInstr {
    pub abs_addr: u64,
    pub raw32:    u32,   // 32-bit padded; if RVC, high half = 0.
    pub is_rvc:   bool,  // true if 16-bit compressed instruction.
}

#[derive(Debug, Clone)]
pub struct ParsedReloc {
    pub r_type: u32,
    pub offset: u64,
    pub symbol: Option<String>,
}

pub fn parse_elf(bytes: &[u8]) -> Result<ParsedElf, String> {
    let elf = ElfFile64::<Endianness>::parse(bytes)
        .map_err(|e| format!("not a 64-bit ELF: {}", e))?;
    let endian = elf.endian();
    let e_type = elf.elf_header().e_type(endian);

    // Section table — name + addr + size + flags
    let mut sections = Vec::new();
    let mut section_addr_by_index: BTreeMap<u16, u64> = BTreeMap::new();
    let mut section_flags_by_index: BTreeMap<u16, u64> = BTreeMap::new();
    for (idx, section) in elf.sections().enumerate() {
        // object crate section index = 1-based for elf::SectionIndex; we
        // want raw u16 for symbol cross-reference.
        let name = section.name().unwrap_or("<noname>").to_string();
        let sh_addr  = section.address();
        let sh_size  = section.size();
        let sh_flags = section.elf_section_header().sh_flags(endian);
        let is_text = name == ".text" || name.starts_with(".text.");
        sections.push(ParsedSection {
            name: name.clone(),
            sh_addr,
            sh_size,
            sh_flags,
            is_text,
        });
        section_addr_by_index.insert(idx as u16 + 1, sh_addr);
        section_flags_by_index.insert(idx as u16 + 1, sh_flags);
    }

    // Symbol table — full list, filter doctrine (CR-11) regions.rs'de.
    let mut symbols = Vec::new();
    for sym in elf.symbols() {
        let name = sym.name().unwrap_or("<noname>").to_string();
        let st_value = sym.address();
        let raw = sym.elf_symbol();
        let st_shndx_raw = raw.st_shndx(endian);
        let st_shndx = match st_shndx_raw {
            0      => SymSection::Undef,    // SHN_UNDEF
            0xFFF1 => SymSection::Absolute, // SHN_ABS
            0xFFF2 => SymSection::Common,   // SHN_COMMON
            n if n >= 0xFF00 => SymSection::Reserved,
            n => SymSection::Index(n),
        };
        let st_info_type = raw.st_info() & 0x0F;
        let st_type = match st_info_type {
            1 => SymType::Object,
            2 => SymType::Func,
            3 => SymType::Section,
            4 => SymType::File,
            0 => SymType::NoType,
            _ => SymType::Other,
        };
        symbols.push(ParsedSymbol {
            name,
            st_value,
            st_shndx,
            st_type,
        });
    }

    // text words — collect 32-bit or RVC 16-bit instructions for opcode scan.
    let mut text_words = Vec::new();
    for section in elf.sections() {
        let name = section.name().unwrap_or("");
        if name != ".text" && !name.starts_with(".text.") {
            continue;
        }
        let data = match section.data() {
            Ok(d) => d,
            Err(_) => continue,
        };
        let base_addr = section.address();
        let mut offset = 0usize;
        while offset < data.len() {
            // RV instruction length encoding:
            //  - bits[1:0] != 0b11 → 16-bit RVC
            //  - bits[1:0] == 0b11 and bits[4:2] != 0b111 → 32-bit
            //  - else → 48/64-bit (not supported in RV64IMAC; reject in
            //    decoder)
            if offset + 2 > data.len() { break; }
            let lo = u16::from_le_bytes([data[offset], data[offset + 1]]) as u32;
            let is_rvc = (lo & 0b11) != 0b11;
            if is_rvc {
                text_words.push(TextInstr {
                    abs_addr: base_addr + offset as u64,
                    raw32: lo,
                    is_rvc: true,
                });
                offset += 2;
            } else {
                if offset + 4 > data.len() {
                    // Truncated instruction at end; record as malformed
                    // 32-bit (decoder reports parse error).
                    text_words.push(TextInstr {
                        abs_addr: base_addr + offset as u64,
                        raw32: lo,
                        is_rvc: false,
                    });
                    break;
                }
                let hi = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as u32;
                let raw32 = lo | (hi << 16);
                text_words.push(TextInstr {
                    abs_addr: base_addr + offset as u64,
                    raw32,
                    is_rvc: false,
                });
                offset += 4;
            }
        }
    }

    // Relocations — collected from any reloc section (REL or RELA).
    // SAFE-3 reject classes (G3): R_RISCV_RELAX residue + PIE markers.
    let relocations = Vec::new(); // G3'te doldurulur; G1 scaffold.

    Ok(ParsedElf {
        e_type,
        sections,
        symbols,
        text_words,
        relocations,
    })
}

/// Section index → SHF_ALLOC bit (1 = runtime-loaded).
pub fn section_is_alloc(parsed: &ParsedElf, idx: u16) -> bool {
    if idx == 0 { return false; }
    parsed
        .sections
        .get((idx - 1) as usize)
        .map(|s| s.sh_flags & (SHF_ALLOC as u64) != 0)
        .unwrap_or(false)
}

/// True iff e_type indicates a PIE/dynamic object (verifier rejects).
pub fn is_pie(parsed: &ParsedElf) -> bool {
    parsed.e_type == ET_DYN
}

/// True iff e_type indicates a normal static executable.
pub fn is_static_exec(parsed: &ParsedElf) -> bool {
    parsed.e_type == ET_EXEC
}

#[allow(dead_code)]
const _SHF_GUARD: () = {
    // Compile-time sanity: object crate flag values match ELF spec.
    assert!(SHF_WRITE as u64 == 0x1);
    assert!(SHF_ALLOC as u64 == 0x2);
    assert!(SHF_EXECINSTR as u64 == 0x4);
};
