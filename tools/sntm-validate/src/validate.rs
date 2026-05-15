//! Manifest invariant validators.
//!
//! Pure logic — kernel/pmp/overlap.rs helper'larını çağırmak istesek
//! sipahi crate dependency gerekir; bu HOST tool için sürdürülebilir
//! değil. Duplicate logic kabul: kernel + tool aynı pure fn'i implement
//! eder. SNTM-R3/R5 Kani proof'ları kernel tarafını kanıtlar; sntm-validate
//! integration test'i tool tarafını kanıtlar (tests/integration.rs).

use crate::manifest::{Manifest, TaskEntry};

const KERNEL_PMP_ENTRIES: u8 = 6;  // SNTM design v0.5 §4.5.1 static budget
const MAX_REGIONS_PER_TASK: usize = 6;

// Kernel address range (Sipahi v1.5 sabit layout — sipahi.ld'den).
// Kernel image 0x80000000..0x80100000 (1MB), task'lar 0x80100000+.
// U-25'te dinamik kernel.size manifest'ten okunacak.
const KERNEL_BASE: usize = 0x8000_0000;
const KERNEL_SIZE: usize = 0x10_0000;  // 1MB kernel image (rough upper bound)

pub fn validate_all(m: &Manifest) -> Result<(), Vec<String>> {
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

    if errors.is_empty() { Ok(()) } else { Err(errors) }
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

    let required = KERNEL_PMP_ENTRIES as usize + max_per_task;
    let available = m.platform.pmp_entries as usize;

    if required > available {
        return Err(vec![format!(
            "PMP budget exceeded: kernel({}) + max_per_task({}) = {} > platform.pmp_entries({})",
            KERNEL_PMP_ENTRIES, max_per_task, required, available
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
