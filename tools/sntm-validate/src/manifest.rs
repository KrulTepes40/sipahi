//! sipahi.toml deserialization types.
//!
//! Manifest schema SNTM design v0.8 §4.4'e uygun.
//!
//! NOT: U-24'te bazı alanlar deserialize ediliyor ama henüz validate
//! edilmiyor (binary, kernel.version, platform.target/machine, task
//! metadata: priority, period_ticks, budget_cycles, dal_level, region.perm).
//! Bu alanlar Sprint U-25 (build-time const table generation) ve
//! Sprint U-26 (kernel loader) tarafından kullanılacak.

#![allow(dead_code)] // U-24 placeholder: schema complete, validation U-25/U-26'da

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub kernel:   KernelEntry,
    pub platform: PlatformEntry,
    #[serde(default, rename = "task")]
    pub tasks:    Vec<TaskEntry>,
}

#[derive(Deserialize, Debug)]
pub struct KernelEntry {
    pub name:       String,
    pub version:    String,
    pub binary:     String,
    pub stack_size: usize,
}

#[derive(Deserialize, Debug)]
pub struct PlatformEntry {
    pub target:      String,
    pub machine:     String,
    pub pmp_entries: u8,
    pub ram_base:    usize,
    pub ram_size:    usize,
}

#[derive(Deserialize, Debug)]
pub struct TaskEntry {
    pub name:          String,
    pub binary:        String,
    pub task_id:       u8,
    pub priority:      u8,
    pub period_ticks:  u32,
    pub budget_cycles: u32,
    pub dal_level:     String,
    #[serde(default, rename = "region")]
    pub regions:       Vec<RegionEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegionEntry {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub perm: String,  // "RX", "R", "RW", "NONE"
}
