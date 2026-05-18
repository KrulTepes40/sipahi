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
    /// SAFE-1: SNTM-SAFE trust tier — "safe" (default) | "trusted_unsafe".
    /// DAL-A/B trusted_unsafe HARD-FAIL (cert doctrine); DAL-C/D waiver_reason ile izinli.
    #[serde(default = "default_trust_tier")]
    pub trust_tier:    String,
    /// SAFE-1: trusted_unsafe için zorunlu, safe için boş.
    #[serde(default)]
    pub waiver_reason: String,
    /// SAFE-1: cfg-gated demo feature listesi (task-lint scope dışı tutulur).
    /// Her item Cargo.toml [features]'de tanımlı + default-OFF olmalı (drift guard).
    #[serde(default)]
    pub demo_feature_waivers: Vec<String>,
    #[serde(default, rename = "region")]
    pub regions:       Vec<RegionEntry>,
}

fn default_trust_tier() -> String {
    "safe".to_string()
}

/// SAFE-1: DAL level enum — string compare yerine type-safe parse.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum DalLevel { A, B, C, D }

impl DalLevel {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "A" => Ok(DalLevel::A),
            "B" => Ok(DalLevel::B),
            "C" => Ok(DalLevel::C),
            "D" => Ok(DalLevel::D),
            _   => Err(format!("invalid dal_level: {} (must be A/B/C/D)", s)),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegionEntry {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub perm: String,  // "RX", "R", "RW", "NONE"
}
