//! Manifest invariant validators.
//!
//! Pure logic — kernel/pmp/overlap.rs helper'larını çağırmak istesek
//! sipahi crate dependency gerekir; bu HOST tool için sürdürülebilir
//! değil. Duplicate logic kabul: kernel + tool aynı pure fn'i implement
//! eder. SNTM-R3/R5 Kani proof'ları kernel tarafını kanıtlar; sntm-validate
//! integration test'i tool tarafını kanıtlar (tests/integration.rs).

use crate::manifest::{Manifest, TaskEntry};
use std::path::Path;

/// U-25 FIX-6: PMP reserved low entries (kernel + UART, lock'lu).
/// SNTM design v0.8 §4.5.1 + Sipahi PMP layout:
///   entry 0..5: kernel text/rodata/data
///   entry 6..7: UART MMIO TOR + LOCK (debug/trace/self-test)
/// → 8 entry kernel/UART reserved. Dynamic SNTM entry'leri 8..15.
///
/// U-24'te bu sabit KERNEL_PMP_ENTRIES=6 idi (UART entry 6/7 sayılmıyordu →
/// budget yanlış pozitif veriyordu). U-25'te düzeltildi.
const RESERVED_LOW_PMP_ENTRIES: u8 = 8;
const MAX_REGIONS_PER_TASK: usize = 6;

// Kernel address range — sipahi.ld'den + manifest [kernel] reserved_size.
// SAFE-3 (sprint-u32, Section 8 CR-2): KERNEL_SIZE const 1MB idi; gerçek
// _end ≤ 0x80600000 (sipahi.ld:129) → 6MB. Validator manifest field oku.
const KERNEL_BASE: usize = 0x8000_0000;

pub fn validate_all(m: &Manifest, manifest_path: Option<&Path>) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Err(es) = check_task_id_uniqueness(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_napot_alignment(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_intra_task_overlap(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_cross_task_overlap(&m.tasks) {
        errors.extend(es);
    }
    if let Err(es) = check_kernel_task_overlap(m) {
        errors.extend(es);
    }
    if let Err(es) = check_pmp_budget(m) {
        errors.extend(es);
    }
    // SAFE-1 (U-30): SNTM-SAFE schema checks (trust_tier + waiver_reason +
    // demo_feature_waivers + DAL enum + DAL × trust_tier policy matrix).
    if let Err(es) = check_safe_native_profile(m) {
        errors.extend(es);
    }
    // U-30.1: demo_feature_waivers her item task Cargo.toml [features]'de tanımlı +
    // [features.default] dışında olmalı (default-ON drift guard). task-lint ile
    // defense-in-depth. manifest_path verilmediyse skip (legacy test scenarios).
    if let Some(mp) = manifest_path {
        if let Err(es) = check_demo_waiver_cargo(m, mp) {
            errors.extend(es);
        }
    }
    // SAFE-2 (sprint-u31): [[resource]] + [[channel]] schema invariants.
    if let Err(es) = check_resources(m) {
        errors.extend(es);
    }
    if let Err(es) = check_channels(m) {
        errors.extend(es);
    }
    if let Err(es) = check_local_caps(m) {
        errors.extend(es);
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

// SAFE-2 platform constants — must mirror src/common/config.rs.
// MAX_IPC_CHANNELS=8 (config.rs:78), IPC_MSG_SIZE=64 (config.rs:72), MAX_RESOURCES=4 (Section 8 FIX-E).
const MAX_IPC_CHANNELS: u8 = 8;
const IPC_MSG_SIZE: usize = 64;
const MAX_RESOURCES: u8 = 4;

/// SAFE-2 (CR Section 8): `[[resource]]` invariants.
///   1. id < MAX_RESOURCES (manifest can't request more rows than codegen emits)
///   2. id unique
///   3. name unique + non-empty + snake_case-ish (basic — no whitespace/punct)
///   4. kind non-empty
fn check_resources(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let mut seen_ids = std::collections::HashMap::<u8, &str>::new();
    let mut seen_names = std::collections::HashMap::<&str, u8>::new();
    for r in &m.resources {
        if r.id >= MAX_RESOURCES {
            errs.push(format!(
                "resource '{}': id={} >= MAX_RESOURCES({})",
                r.name, r.id, MAX_RESOURCES
            ));
        }
        if let Some(prev) = seen_ids.insert(r.id, r.name.as_str()) {
            errs.push(format!(
                "resource id={} duplicate: '{}' and '{}'", r.id, prev, r.name
            ));
        }
        if r.name.trim().is_empty() {
            errs.push(format!("resource id={}: empty name", r.id));
        } else {
            if let Some(prev_id) = seen_names.insert(r.name.as_str(), r.id) {
                errs.push(format!(
                    "resource name '{}' duplicate: id={} and id={}",
                    r.name, prev_id, r.id
                ));
            }
            if r.name.contains(|c: char| c.is_whitespace() || c == '.') {
                errs.push(format!(
                    "resource '{}': name must be snake_case (no whitespace/punct)",
                    r.name
                ));
            }
        }
        if r.kind.trim().is_empty() {
            errs.push(format!("resource '{}': empty kind", r.name));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// SAFE-2 (CR Section 8): `[[channel]]` invariants.
///   1. id < MAX_IPC_CHANNELS
///   2. id unique
///   3. producer + consumer reference existing [[task]] name (orphan check)
///   4. producer != consumer (self-loop forbidden)
///   5. size <= IPC_MSG_SIZE
///   6. message PascalCase + unique (struct codegen safety)
fn check_channels(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let task_names: std::collections::HashSet<&str> =
        m.tasks.iter().map(|t| t.name.as_str()).collect();
    let mut seen_ids = std::collections::HashMap::<u8, &str>::new();
    let mut seen_messages = std::collections::HashMap::<&str, u8>::new();
    for c in &m.channels {
        let tag = format!("channel id={}", c.id);
        if c.id >= MAX_IPC_CHANNELS {
            errs.push(format!(
                "{}: id >= MAX_IPC_CHANNELS({})", tag, MAX_IPC_CHANNELS
            ));
        }
        if let Some(prev) = seen_ids.insert(c.id, c.message.as_str()) {
            errs.push(format!(
                "channel id={} duplicate: message '{}' and '{}'",
                c.id, prev, c.message
            ));
        }
        if !task_names.contains(c.producer.as_str()) {
            errs.push(format!(
                "{}: producer '{}' orphan (no matching [[task]] name)",
                tag, c.producer
            ));
        }
        if !task_names.contains(c.consumer.as_str()) {
            errs.push(format!(
                "{}: consumer '{}' orphan (no matching [[task]] name)",
                tag, c.consumer
            ));
        }
        if c.producer == c.consumer {
            errs.push(format!(
                "{}: producer == consumer ('{}') — self-loop forbidden",
                tag, c.producer
            ));
        }
        if c.size == 0 {
            errs.push(format!("{}: size=0 — message struct must have at least 1 byte", tag));
        }
        if c.size > IPC_MSG_SIZE {
            errs.push(format!(
                "{}: size={} > IPC_MSG_SIZE({}) — message struct exceeds IPC slot",
                tag, c.size, IPC_MSG_SIZE
            ));
        }
        // Message name: PascalCase = first char upper alpha, rest alphanumeric.
        if c.message.trim().is_empty() {
            errs.push(format!("{}: empty message struct name", tag));
        } else {
            let first = c.message.chars().next().unwrap();
            if !first.is_ascii_uppercase() {
                errs.push(format!(
                    "{}: message '{}' not PascalCase (must start uppercase alpha)",
                    tag, c.message
                ));
            }
            if c.message.chars().any(|ch| !ch.is_ascii_alphanumeric()) {
                errs.push(format!(
                    "{}: message '{}' has non-alphanumeric chars (snake_case forbidden)",
                    tag, c.message
                ));
            }
            if let Some(prev_id) = seen_messages.insert(c.message.as_str(), c.id) {
                errs.push(format!(
                    "message '{}' duplicate: channel id={} and id={}",
                    c.message, prev_id, c.id
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// SAFE-2 (CR-3): `[[task.local_cap]]` invariants.
///   1. resource_id < MAX_RESOURCES + references existing [[resource]]
///   2. action ∈ {None, Read, Write, ReadWrite, Execute, All}
///   3. (task, resource_id) pair unique within task (no duplicate grants)
fn check_local_caps(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let resource_ids: std::collections::HashSet<u8> =
        m.resources.iter().map(|r| r.id).collect();
    for t in &m.tasks {
        let mut seen = std::collections::HashSet::<u8>::new();
        for g in &t.local_caps {
            if g.resource_id >= MAX_RESOURCES {
                errs.push(format!(
                    "task '{}': local_cap resource_id={} >= MAX_RESOURCES({})",
                    t.name, g.resource_id, MAX_RESOURCES
                ));
                continue;
            }
            if !resource_ids.contains(&g.resource_id) {
                errs.push(format!(
                    "task '{}': local_cap resource_id={} orphan (no [[resource]] declared)",
                    t.name, g.resource_id
                ));
            }
            if !seen.insert(g.resource_id) {
                errs.push(format!(
                    "task '{}': duplicate local_cap grant for resource_id={}",
                    t.name, g.resource_id
                ));
            }
            match g.action.as_str() {
                "None" | "Read" | "Write" | "ReadWrite" | "Execute" | "All" => {}
                other => errs.push(format!(
                    "task '{}': invalid local_cap action '{}' for resource_id={} \
                     (must be None|Read|Write|ReadWrite|Execute|All)",
                    t.name, other, g.resource_id
                )),
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// SAFE-1 (U-30): SNTM-SAFE Safe Native Profile manifest invariants.
///
/// Kontroller:
///   1. dal_level enum parse (A/B/C/D dışı FAIL)
///   2. trust_tier value "safe" | "trusted_unsafe" dışı FAIL
///   3. trust_tier="trusted_unsafe" + waiver_reason boş → FAIL
///   4. DAL-A/B + trust_tier="trusted_unsafe" → HARD-FAIL (DO-178C cert doctrine)
///   5. demo_feature_waivers + trust_tier="trusted_unsafe" combine YASAK
///      (tek tier ya cfg waiver — audit karmaşası önlenir)
///   6. demo_feature_waivers orphan feature ismi FAIL (Cargo.toml [features]'de yok)
///      → NOT: bu check sntm-validate'in Cargo.toml okuma kapasitesi gerektirir;
///        SAFE-1 scope'unda basit "non-empty string" validate, tam Cargo.toml
///        cross-check task-lint tool tarafında yapılır (G3'te).
fn check_safe_native_profile(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in &m.tasks {
        // (1) DAL enum parse
        let dal = match crate::manifest::DalLevel::parse(&t.dal_level) {
            Ok(d) => d,
            Err(e) => {
                errs.push(format!("task '{}': {}", t.name, e));
                continue;
            }
        };
        // (2) trust_tier value
        let tier = t.trust_tier.as_str();
        if tier != "safe" && tier != "trusted_unsafe" {
            errs.push(format!(
                "task '{}': invalid trust_tier '{}' (must be 'safe' or 'trusted_unsafe')",
                t.name, tier
            ));
            continue;
        }
        // (3) trusted_unsafe + boş waiver_reason FAIL
        if tier == "trusted_unsafe" && t.waiver_reason.trim().is_empty() {
            errs.push(format!(
                "task '{}': trust_tier='trusted_unsafe' requires waiver_reason (non-empty)",
                t.name
            ));
        }
        // (4) DAL-A/B + trusted_unsafe HARD-FAIL
        if tier == "trusted_unsafe" && (dal == crate::manifest::DalLevel::A
                                    || dal == crate::manifest::DalLevel::B) {
            errs.push(format!(
                "task '{}': DAL-{:?} forbids trust_tier='trusted_unsafe' (DO-178C cert doctrine)",
                t.name, dal
            ));
        }
        // (5) demo_feature_waivers + trusted_unsafe combine YASAK
        if tier == "trusted_unsafe" && !t.demo_feature_waivers.is_empty() {
            errs.push(format!(
                "task '{}': demo_feature_waivers + trust_tier='trusted_unsafe' \
                 combine YASAK (audit karmaşası — tek tier ya cfg waiver)",
                t.name
            ));
        }
        // (6) demo_feature_waivers non-empty string ve non-default features.
        // Tam Cargo.toml cross-check task-lint tarafında (G3).
        for waiver in &t.demo_feature_waivers {
            if waiver.trim().is_empty() {
                errs.push(format!(
                    "task '{}': demo_feature_waivers boş string içeriyor",
                    t.name
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// U-30.1: demo_feature_waivers Cargo.toml [features] cross-check.
///
/// Her task'in tasks/<name>/Cargo.toml içinde her waiver item:
///   - [features] tablosunda tanımlı (orphan FAIL)
///   - [features.default] dizisinde DEĞİL (default-ON drift FAIL)
fn check_demo_waiver_cargo(m: &Manifest, manifest_path: &Path) -> Result<(), Vec<String>> {
    let workspace_root = manifest_path.parent().unwrap_or(Path::new("."));
    let mut errs = Vec::new();
    for t in &m.tasks {
        if t.demo_feature_waivers.is_empty() {
            continue;
        }
        let task_cargo = workspace_root.join("tasks").join(&t.name).join("Cargo.toml");
        if !task_cargo.exists() {
            errs.push(format!(
                "task '{}': demo_feature_waivers={:?} but tasks/{}/Cargo.toml missing",
                t.name, t.demo_feature_waivers, t.name
            ));
            continue;
        }
        let content = match std::fs::read_to_string(&task_cargo) {
            Ok(s) => s,
            Err(e) => {
                errs.push(format!("task '{}': cannot read Cargo.toml: {}", t.name, e));
                continue;
            }
        };
        let parsed: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!("task '{}': Cargo.toml parse error: {}", t.name, e));
                continue;
            }
        };
        let features = match parsed.get("features") {
            Some(toml::Value::Table(t)) => t.clone(),
            Some(_) => {
                errs.push(format!("task '{}': [features] must be a table", t.name));
                continue;
            }
            None => {
                errs.push(format!(
                    "task '{}': demo_feature_waivers={:?} but [features] table missing",
                    t.name, t.demo_feature_waivers
                ));
                continue;
            }
        };
        let default_list: Vec<String> = features.get("default")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect())
            .unwrap_or_default();
        for w in &t.demo_feature_waivers {
            if !features.contains_key(w) {
                errs.push(format!(
                    "task '{}': waiver '{}' orphan (not in [features])", t.name, w
                ));
                continue;
            }
            if default_list.iter().any(|d| d == w) {
                errs.push(format!(
                    "task '{}': waiver '{}' in [features.default] (must be default-OFF; drift)",
                    t.name, w
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_task_id_uniqueness(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut seen: std::collections::HashMap<u8, &str> = std::collections::HashMap::new();
    let mut errs = Vec::new();
    for t in tasks {
        if let Some(prev) = seen.insert(t.task_id, t.name.as_str()) {
            errs.push(format!(
                "task_id={} duplicate: '{}' and '{}'",
                t.task_id, prev, t.name
            ));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_napot_alignment(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        if t.regions.len() > MAX_REGIONS_PER_TASK {
            errs.push(format!(
                "task '{}': {} regions > MAX_REGIONS_PER_TASK ({})",
                t.name, t.regions.len(), MAX_REGIONS_PER_TASK
            ));
        }
        for r in &t.regions {
            if !valid_napot_alignment(r.base, r.size) {
                errs.push(format!(
                    "task '{}' region '{}': base=0x{:x} size=0x{:x} \
                     not NAPOT-aligned (size power-of-2 + base aligned)",
                    t.name, r.name, r.base, r.size
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_intra_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        for i in 0..t.regions.len() {
            for j in (i + 1)..t.regions.len() {
                let a = &t.regions[i];
                let b = &t.regions[j];
                if regions_overlap(a.base, a.size, b.base, b.size) {
                    errs.push(format!(
                        "task '{}': region '{}' overlaps '{}' \
                         (a=0x{:x}+0x{:x}, b=0x{:x}+0x{:x})",
                        t.name, a.name, b.name,
                        a.base, a.size, b.base, b.size
                    ));
                }
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_cross_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for i in 0..tasks.len() {
        for j in (i + 1)..tasks.len() {
            let ta = &tasks[i];
            let tb = &tasks[j];
            for ra in &ta.regions {
                for rb in &tb.regions {
                    if regions_overlap(ra.base, ra.size, rb.base, rb.size) {
                        errs.push(format!(
                            "task '{}' region '{}' overlaps task '{}' region '{}' \
                             (a=0x{:x}+0x{:x}, b=0x{:x}+0x{:x})",
                            ta.name, ra.name, tb.name, rb.name,
                            ra.base, ra.size, rb.base, rb.size
                        ));
                    }
                }
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// SNTM-R3 (kernel half): task region kernel address range ile çakışmamalı.
/// Critical: PMP priority + kernel-task overlap = izolasyon ihlali
/// (SNTM design v0.8 §4.5.2 shadow attack scenario).
///
/// SAFE-3 (sprint-u32, Section 8 CR-2): kernel size manifest
/// `[kernel] reserved_size` field'dan okunur (default 6MB). Eski 1MB
/// hardcoded const → silent overlap riski (0x80100000..0x80600000 arası
/// kernel `.task_stacks` + `.wasm_arena` + `.bss` ile çakışan region
/// silent kabul ediliyordu).
fn check_kernel_task_overlap(m: &Manifest) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let kernel_size = m.kernel.reserved_size;
    for t in &m.tasks {
        for r in &t.regions {
            if regions_overlap(r.base, r.size, KERNEL_BASE, kernel_size) {
                errs.push(format!(
                    "task '{}' region '{}' (base=0x{:x} size=0x{:x}) \
                     overlaps kernel range [0x{:x}..0x{:x})",
                    t.name, r.name, r.base, r.size,
                    KERNEL_BASE, KERNEL_BASE + kernel_size
                ));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_pmp_budget(m: &Manifest) -> Result<(), Vec<String>> {
    // Worst case: tüm region NAPOT (1 entry per region).
    // Per-task max region sayısı budget'ı belirler.
    let max_per_task = m.tasks
        .iter()
        .map(|t| t.regions.len())
        .max()
        .unwrap_or(0);

    let required = RESERVED_LOW_PMP_ENTRIES as usize + max_per_task;
    let available = m.platform.pmp_entries as usize;

    if required > available {
        return Err(vec![format!(
            "PMP budget exceeded: reserved({}) + max_per_task({}) = {} > platform.pmp_entries({})",
            RESERVED_LOW_PMP_ENTRIES, max_per_task, required, available
        )]);
    }
    Ok(())
}

// ─── SAFE-4 stack bound (sprint-u33, Section 8 CR-5) ─────────────────

/// SAFE-4 CR-5: static stack analysis imprecise + inline asm + compiler drift
/// için safety margin. 256 byte default — task_hello observed max ~128 byte
/// ile bile rahat sığar; production DAL-A için 512+ önerilir (per-task
/// manifest override `stack_margin_override`). Exact equality PASS yasak.
pub const STACK_ANALYSIS_MARGIN_BYTES: u32 = 256;

/// SAFE-4 CR-4: cert + sntm-stack ortak UNKNOWN sentinel.
pub const STACK_UNKNOWN_SENTINEL: u32 = 0xFFFF_FFFF;

/// Stack region bound invariant: manifest task `name="stack"` region size
/// ≥ observed_max + margin. Exact equality FAIL (CR-5 doctrine).
/// observed_max == UNKNOWN_SENTINEL → ASLA PASS (CR-4 doctrine — sentinel
/// "analiz yapılmadı" anlamına gelir, gate erteleyemez).
pub fn check_stack_bounds(
    task: &TaskEntry,
    observed_max: u32,
) -> Result<(), Vec<String>> {
    if observed_max == STACK_UNKNOWN_SENTINEL {
        return Err(vec![format!(
            "task '{}': stack analysis missing — max_stack_bytes UNKNOWN sentinel \
             (0xFFFF_FFFF). SAFE gate report ZORUNLU (Section 8 CR-4 doctrine).",
            task.name
        )]);
    }
    let stack_region = task.regions.iter().find(|r| r.name == "stack");
    let stack_region = match stack_region {
        Some(r) => r,
        None => return Err(vec![format!(
            "task '{}': no region named 'stack' — manifest schema error",
            task.name
        )]),
    };
    let stack_size: u32 = match u32::try_from(stack_region.size) {
        Ok(v) => v,
        Err(_) => return Err(vec![format!(
            "task '{}': stack region size {} overflow u32",
            task.name, stack_region.size
        )]),
    };
    let margin = task.stack_margin_override.unwrap_or(STACK_ANALYSIS_MARGIN_BYTES);
    let required = match observed_max.checked_add(margin) {
        Some(v) => v,
        None => return Err(vec![format!(
            "task '{}': observed_max {} + margin {} overflow u32",
            task.name, observed_max, margin
        )]),
    };
    if stack_size < required {
        return Err(vec![format!(
            "task '{}': stack_size {} < observed_max {} + margin {} (= {}) \
             (Section 8 CR-5 doctrine — exact equality PASS yasak)",
            task.name, stack_size, observed_max, margin, required
        )]);
    }
    Ok(())
}

#[cfg(test)]
mod stack_bounds_tests {
    use super::*;
    use crate::manifest::{RegionEntry, TaskEntry};

    fn make_task(stack_size: usize, margin_override: Option<u32>) -> TaskEntry {
        TaskEntry {
            name: "task_x".into(),
            binary: "x.bin".into(),
            task_id: 1,
            priority: 1,
            period_ticks: 100,
            budget_cycles: 1000,
            dal_level: "D".into(),
            trust_tier: "safe".into(),
            waiver_reason: "".into(),
            demo_feature_waivers: Vec::new(),
            regions: vec![RegionEntry {
                name: "stack".into(),
                base: 0x80700000,
                size: stack_size,
                perm: "RW".into(),
            }],
            local_caps: Vec::new(),
            stack_margin_override: margin_override,
        }
    }

    #[test]
    fn stack_bounds_pass_with_default_margin() {
        let t = make_task(8192, None);
        assert!(check_stack_bounds(&t, 128).is_ok());
    }

    #[test]
    fn stack_bounds_exact_equality_fails() {
        // 4096 stack + 4096 observed + 256 margin = 4352 > 4096 → FAIL (CR-5 doctrine).
        let t = make_task(4096, None);
        let err = check_stack_bounds(&t, 4096).unwrap_err();
        assert!(err[0].contains("CR-5"));
        assert!(err[0].contains("task_x"));
    }

    #[test]
    fn stack_bounds_unknown_sentinel_always_fails() {
        let t = make_task(65536, None); // huge stack — even so, sentinel rejected
        let err = check_stack_bounds(&t, STACK_UNKNOWN_SENTINEL).unwrap_err();
        assert!(err[0].contains("UNKNOWN sentinel"));
        assert!(err[0].contains("CR-4"));
    }

    #[test]
    fn stack_bounds_margin_override_honored() {
        // 4096 stack, observed 3000, override 1500 → 3000+1500=4500 > 4096 FAIL
        let t = make_task(4096, Some(1500));
        let err = check_stack_bounds(&t, 3000).unwrap_err();
        assert!(err[0].contains("margin 1500"));
    }

    #[test]
    fn stack_bounds_no_stack_region_fails() {
        let mut t = make_task(4096, None);
        t.regions.clear();
        let err = check_stack_bounds(&t, 128).unwrap_err();
        assert!(err[0].contains("no region named 'stack'"));
    }
}

// ─── Pure helpers (kernel/pmp/overlap.rs ile duplicate, host-side) ──

#[inline]
fn regions_overlap(a_base: usize, a_size: usize, b_base: usize, b_size: usize) -> bool {
    if a_size == 0 || b_size == 0 {
        return false;
    }
    let a_end = a_base.saturating_add(a_size);
    let b_end = b_base.saturating_add(b_size);
    !(a_end <= b_base || b_end <= a_base)
}

#[inline]
fn valid_napot_alignment(base: usize, size: usize) -> bool {
    if size < 8 {
        return false;
    }
    if size & (size - 1) != 0 {
        return false;
    }
    base & (size - 1) == 0
}
