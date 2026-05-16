//! Manifest → kernel const table codegen.
//! U-25 SNTM Phase 3: sntm-validate --output-rs <path> tetikler.
//!
//! Output: src/kernel/pmp/generated.rs:
//!   pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] = [...]
//!
//! Drift detection: CI re-runs sntm-validate + git diff (G13).

use crate::manifest::{Manifest, RegionEntry, TaskEntry};
use crate::napot::{napot_pmpaddr, napot_size_log2};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

const MAX_TASKS: usize = 8;
const MAX_REGIONS: usize = 6;

const HEADER: &str = r#"//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml (run `bash scripts/regen_pmp_profiles.sh` or
//! `make regen-pmp` to regenerate).
//! U-25 SNTM Phase 3 codegen — sntm-validate --output-rs output.
//!
//! Drift detection: CI runs sntm-validate again + git diff.

use crate::arch::pmp::PmpEncoding;
use crate::kernel::pmp::profile::{Permission, PmpProfile, Region};

"#;

/// Manifest → generated.rs codegen entry point.
pub fn generate_pmp_profiles_rs(m: &Manifest, out_path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(out_path)?;
    file.write_all(HEADER.as_bytes())?;
    writeln!(
        file,
        "pub static PMP_PROFILES: [PmpProfile; {}] = [",
        MAX_TASKS
    )?;

    // Build task_id → task lookup
    let mut tid_map: HashMap<u8, &TaskEntry> = HashMap::new();
    for t in &m.tasks {
        tid_map.insert(t.task_id, t);
    }

    for slot in 0..MAX_TASKS {
        match tid_map.get(&(slot as u8)) {
            Some(t) => emit_task_profile(&mut file, t)?,
            None => writeln!(file, "    PmpProfile::EMPTY,")?,
        }
    }

    writeln!(file, "];")?;
    Ok(())
}

fn emit_task_profile(file: &mut std::fs::File, t: &TaskEntry) -> std::io::Result<()> {
    writeln!(file, "    // Task {} ({})", t.task_id, t.name)?;
    writeln!(file, "    PmpProfile {{")?;
    writeln!(file, "        region_count: {},", t.regions.len())?;
    writeln!(file, "        regions: [")?;
    for r in &t.regions {
        emit_region(file, r)?;
    }
    // Pad to MAX_REGIONS (6) entries
    for _ in t.regions.len()..MAX_REGIONS {
        writeln!(
            file,
            "            Region {{ base: 0, size: 0, encoding: PmpEncoding::Napot {{ addr: 0, size_log2: 0 }}, perm: Permission::NONE }},"
        )?;
    }
    writeln!(file, "        ],")?;
    writeln!(file, "    }},")?;
    Ok(())
}

fn emit_region(file: &mut std::fs::File, r: &RegionEntry) -> std::io::Result<()> {
    let perm_str = parse_perm(&r.perm);
    let encoding = match napot_size_log2(r.base as u64, r.size as u64) {
        Some(log2) => {
            let addr = napot_pmpaddr(r.base as u64, log2);
            format!(
                "PmpEncoding::Napot {{ addr: 0x{:x}, size_log2: {} }}",
                addr, log2
            )
        }
        None => {
            // NAPOT-uyumsuz → TOR fallback. Validator zaten reject ediyor
            // (check_napot_alignment), bu defansif kod yolu.
            format!(
                "PmpEncoding::Tor {{ lo: 0x{:x}, hi: 0x{:x} }}",
                r.base,
                r.base + r.size
            )
        }
    };
    writeln!(
        file,
        "            Region {{ base: 0x{:x}, size: 0x{:x}, encoding: {}, perm: {} }},",
        r.base, r.size, encoding, perm_str
    )?;
    Ok(())
}

fn parse_perm(s: &str) -> &'static str {
    match s {
        "RX" => "Permission::RX",
        "R" => "Permission::R",
        "RW" => "Permission::RW",
        _ => "Permission::NONE",  // unknown → conservative deny
    }
}
