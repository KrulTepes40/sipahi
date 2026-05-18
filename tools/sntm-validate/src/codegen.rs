//! Manifest → kernel const table codegen.
//! U-25 SNTM Phase 3: sntm-validate --output-rs <path> tetikler.
//!
//! Output: src/kernel/pmp/generated.rs:
//!   pub static PMP_PROFILES: [PmpProfile; MAX_TASKS] = [...]
//!
//! Drift detection: CI re-runs sntm-validate + git diff (G13).

use crate::manifest::{ChannelEntry, Manifest, RegionEntry, TaskEntry};
use crate::napot::{napot_pmpaddr, napot_size_log2};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

const MAX_TASKS: usize = 8;
const MAX_REGIONS: usize = 6;
const MAX_RESOURCES: usize = 4;

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

// ─── SAFE-2 (sprint-u31): cap_generated.rs + channels.rs codegen ──

const CAP_HEADER: &str = r#"//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml [[resource]] + [[task.local_cap]] + [[channel]] entries.
//! Run `bash scripts/regen_safe_codegen.sh` to regenerate.
//! SAFE-2 (sprint-u31): static local capability table + boot channel ownership.
//!
//! Drift detection: CI runs sntm-validate --output-cap-table + git diff.

use crate::kernel::capability::cap_action::CapAction;

"#;

/// SAFE-2: emit `src/kernel/capability/cap_generated.rs`.
///
/// Outputs:
///   pub static LOCAL_CAP_TABLE: [[CapAction; MAX_RESOURCES]; MAX_TASKS] = [...];
///   pub static BOOT_CHANNELS: &[(u8, u8, u8)] = &[(channel_id, producer_task_id, consumer_task_id), ...];
pub fn generate_cap_table_rs(m: &Manifest, out_path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(out_path)?;
    file.write_all(CAP_HEADER.as_bytes())?;

    // Build task_id → (name, local_caps) lookup.
    let mut tid_map: HashMap<u8, &TaskEntry> = HashMap::new();
    for t in &m.tasks {
        tid_map.insert(t.task_id, t);
    }

    writeln!(
        file,
        "pub static LOCAL_CAP_TABLE: [[CapAction; {}]; {}] = [",
        MAX_RESOURCES, MAX_TASKS
    )?;
    for slot in 0..MAX_TASKS as u8 {
        let row: [&str; MAX_RESOURCES] = match tid_map.get(&slot) {
            Some(t) => {
                let mut r = ["CapAction::None"; MAX_RESOURCES];
                for g in &t.local_caps {
                    if (g.resource_id as usize) < MAX_RESOURCES {
                        r[g.resource_id as usize] = action_to_variant(&g.action);
                    }
                }
                r
            }
            None => ["CapAction::None"; MAX_RESOURCES],
        };
        let label = tid_map
            .get(&slot)
            .map(|t| t.name.as_str())
            .unwrap_or("<empty slot>");
        writeln!(file, "    /* task {} ({}) */ [{}, {}, {}, {}],",
            slot, label, row[0], row[1], row[2], row[3])?;
    }
    writeln!(file, "];")?;
    writeln!(file)?;

    // BOOT_CHANNELS — producer/consumer task NAMES → task_id lookup.
    let name_to_id: HashMap<&str, u8> =
        m.tasks.iter().map(|t| (t.name.as_str(), t.task_id)).collect();
    writeln!(file, "/// SAFE-2 (CR-5): boot.rs iterates this table to call")?;
    writeln!(file, "/// `ipc::assign_channel(id, producer, consumer)` for each entry.")?;
    writeln!(file, "/// Drift guard: manifest [[channel]] + this table = single source.")?;
    writeln!(file, "pub static BOOT_CHANNELS: &[(u8, u8, u8)] = &[")?;
    for c in &m.channels {
        let producer = name_to_id.get(c.producer.as_str()).copied().unwrap_or(0xFF);
        let consumer = name_to_id.get(c.consumer.as_str()).copied().unwrap_or(0xFF);
        writeln!(
            file,
            "    ({}, {}, {}),  // channel {} \"{}\" {} → {}",
            c.id, producer, consumer,
            c.id, c.message, c.producer, c.consumer
        )?;
    }
    writeln!(file, "];")?;
    Ok(())
}

fn action_to_variant(a: &str) -> &'static str {
    match a {
        "None"      => "CapAction::None",
        "Read"      => "CapAction::Read",
        "Write"     => "CapAction::Write",
        "ReadWrite" => "CapAction::ReadWrite",
        "Execute"   => "CapAction::Execute",
        "All"       => "CapAction::All",
        _ => "CapAction::None",  // defensive — validator already rejects
    }
}

const CHANNELS_HEADER: &str = r#"//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml [[channel]] entries.
//! Run `bash scripts/regen_safe_codegen.sh` to regenerate.
//! SAFE-2 (sprint-u31): typed IPC API per CR-4 safety gate template.
//!
//! Drift detection: CI runs sntm-validate --output-channels + git diff.
//!
//! Per CR-4 safety gates: each struct enforces:
//!   - size_of::<T>() == manifest size_field    (compile-time assert)
//!   - size_of::<T>() <= IPC_MSG_SIZE           (compile-time assert)
//!   - align_of::<T>() <= 8                     (compile-time assert)
//!   - repr(C, align(8)) for stable ABI
//!   - send: Message::empty() before copy → no padding leak
//!   - recv: copy_nonoverlapping(_, _, size_of::<T>()) → only first N bytes

// Without per-task features, the cfg-gated wrappers vanish and the imports
// look unused — but they are referenced by the generated send/recv bodies
// once any `task_*` feature is enabled. Suppress the false-positive warning
// (also dodges a rustc 1.96 annotate_snippets ICE on the warning render).
#![allow(unused_imports)]

use crate::ipc::Message;
use crate::syscall;
use crate::Error;

"#;

/// SAFE-2: emit `sipahi_api/src/channels.rs`.
///
/// Generated per CR-4 template: typed wrappers cfg-gated by task feature
/// `task_<name>`. Producer compiles only `send_*`, consumer compiles only
/// `recv_*` — wrong direction = compile fail.
pub fn generate_channels_rs(m: &Manifest, out_path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(out_path)?;
    file.write_all(CHANNELS_HEADER.as_bytes())?;

    if m.channels.is_empty() {
        writeln!(file, "// (no [[channel]] entries in manifest)")?;
        return Ok(());
    }

    for c in &m.channels {
        emit_channel_struct_and_wrappers(&mut file, c)?;
    }
    Ok(())
}

fn emit_channel_struct_and_wrappers(
    file: &mut std::fs::File,
    c: &ChannelEntry,
) -> std::io::Result<()> {
    let snake = pascal_to_snake(&c.message);
    writeln!(file, "// ─── channel {} ({} → {}) ───────────────────────────",
        c.id, c.producer, c.consumer)?;
    writeln!(file)?;
    writeln!(file, "/// Typed IPC message body for channel {} ({}).",
        c.id, c.message)?;
    writeln!(file, "/// SAFE-2 invariant: size = {} bytes, repr(C, align(8)).",
        c.size)?;
    writeln!(file, "#[repr(C, align(8))]")?;
    writeln!(file, "#[derive(Clone, Copy, Debug)]")?;
    writeln!(file, "pub struct {} {{", c.message)?;
    writeln!(file, "    /// Opaque payload (manifest size = {} bytes).", c.size)?;
    writeln!(file, "    pub bytes: [u8; {}],", c.size)?;
    writeln!(file, "}}")?;
    writeln!(file)?;
    writeln!(file, "// CR-4 compile-time safety gates")?;
    writeln!(file, "const _: () = assert!(core::mem::size_of::<{}>() == {});",
        c.message, c.size)?;
    writeln!(file, "const _: () = assert!(core::mem::size_of::<{}>() <= Message::SIZE);",
        c.message)?;
    writeln!(file, "const _: () = assert!(core::mem::align_of::<{}>() <= 8);",
        c.message)?;
    writeln!(file)?;

    // send wrapper — only producer task compiles.
    writeln!(file, "/// Send a typed `{}` message on channel {} ({} → {}).",
        c.message, c.id, c.producer, c.consumer)?;
    writeln!(file, "/// Cfg-gated: only compiles when `task_{}` feature is enabled.",
        c.producer)?;
    writeln!(file, "#[cfg(feature = \"task_{}\")]", c.producer)?;
    writeln!(file, "#[inline]")?;
    writeln!(file, "pub fn send_{}(msg: &{}) -> Result<(), Error> {{", snake, c.message)?;
    writeln!(file, "    const CHANNEL_ID: u8 = {};", c.id)?;
    writeln!(file, "    // CR-4: zero-init buf prevents padding-byte leakage; only size_of::<T>() bytes copied.")?;
    writeln!(file, "    let mut buf = Message::empty();")?;
    writeln!(file, "    // SAFETY: T is repr(C, align(8)); size asserted at compile time; buf has IPC_MSG_SIZE bytes;")?;
    writeln!(file, "    // source and dest non-overlapping (distinct stack allocations).")?;
    writeln!(file, "    unsafe {{")?;
    writeln!(file, "        core::ptr::copy_nonoverlapping(")?;
    writeln!(file, "            msg as *const _ as *const u8,")?;
    writeln!(file, "            buf.data.as_mut_ptr(),")?;
    writeln!(file, "            core::mem::size_of::<{}>(),", c.message)?;
    writeln!(file, "        );")?;
    writeln!(file, "    }}")?;
    writeln!(file, "    syscall::ipc_send(CHANNEL_ID, &buf)")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    // recv wrapper — only consumer task compiles.
    writeln!(file, "/// Receive a typed `{}` message on channel {} ({} → {}).",
        c.message, c.id, c.producer, c.consumer)?;
    writeln!(file, "/// `Ok(None)` = empty channel (non-blocking).")?;
    writeln!(file, "/// Cfg-gated: only compiles when `task_{}` feature is enabled.",
        c.consumer)?;
    writeln!(file, "#[cfg(feature = \"task_{}\")]", c.consumer)?;
    writeln!(file, "#[inline]")?;
    writeln!(file, "pub fn recv_{}() -> Result<Option<{}>, Error> {{", snake, c.message)?;
    writeln!(file, "    const CHANNEL_ID: u8 = {};", c.id)?;
    writeln!(file, "    let mut buf = Message::empty();")?;
    writeln!(file, "    match syscall::ipc_recv(CHANNEL_ID, &mut buf) {{")?;
    writeln!(file, "        Ok(true) => {{")?;
    writeln!(file, "            let mut out = core::mem::MaybeUninit::<{}>::uninit();",
        c.message)?;
    writeln!(file, "            // SAFETY: kernel returned IPC_MSG_SIZE bytes; we read only size_of::<T>().")?;
    writeln!(file, "            unsafe {{")?;
    writeln!(file, "                core::ptr::copy_nonoverlapping(")?;
    writeln!(file, "                    buf.data.as_ptr(),")?;
    writeln!(file, "                    out.as_mut_ptr() as *mut u8,")?;
    writeln!(file, "                    core::mem::size_of::<{}>(),", c.message)?;
    writeln!(file, "                );")?;
    writeln!(file, "                Ok(Some(out.assume_init()))")?;
    writeln!(file, "            }}")?;
    writeln!(file, "        }}")?;
    writeln!(file, "        Ok(false) => Ok(None),")?;
    writeln!(file, "        Err(e) => Err(e),")?;
    writeln!(file, "    }}")?;
    writeln!(file, "}}")?;
    writeln!(file)?;
    Ok(())
}

/// Convert PascalCase → snake_case (for function names).
fn pascal_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for low in ch.to_lowercase() {
                out.push(low);
            }
        } else {
            out.push(ch);
        }
    }
    out
}
