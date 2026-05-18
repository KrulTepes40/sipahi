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

// Kernel address range (Sipahi v1.5 sabit layout — sipahi.ld'den).
// Kernel image 0x80000000..0x80100000 (1MB), task'lar 0x80100000+.
// U-25'te dinamik kernel.size manifest'ten okunacak.
const KERNEL_BASE: usize = 0x8000_0000;
const KERNEL_SIZE: usize = 0x10_0000;  // 1MB kernel image (rough upper bound)

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
    if let Err(es) = check_kernel_task_overlap(&m.tasks) {
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

    if errors.is_empty() { Ok(()) } else { Err(errors) }
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
fn check_kernel_task_overlap(tasks: &[TaskEntry]) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for t in tasks {
        for r in &t.regions {
            if regions_overlap(r.base, r.size, KERNEL_BASE, KERNEL_SIZE) {
                errs.push(format!(
                    "task '{}' region '{}' (base=0x{:x} size=0x{:x}) \
                     overlaps kernel range [0x{:x}..0x{:x})",
                    t.name, r.name, r.base, r.size,
                    KERNEL_BASE, KERNEL_BASE + KERNEL_SIZE
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
