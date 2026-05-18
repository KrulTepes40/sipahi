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
    /// SAFE-2 (sprint-u31): [[resource]] entries — LOCAL_CAP_TABLE column space.
    #[serde(default, rename = "resource")]
    pub resources: Vec<ResourceEntry>,
    /// SAFE-2 (sprint-u31): [[channel]] entries — typed IPC + BOOT_CHANNELS.
    #[serde(default, rename = "channel")]
    pub channels: Vec<ChannelEntry>,
}

#[derive(Deserialize, Debug)]
pub struct KernelEntry {
    pub name:       String,
    pub version:    String,
    /// SAFE-3 (sprint-u32, Section 8 CR-2): kernel image reserved address range
    /// at `KERNEL_BASE`. Native task region MUST start at `KERNEL_BASE +
    /// reserved_size`. Default 6MB matches sipahi.ld `_end ≤ 0x80600000` +
    /// NATIVE_TASK_BASE = 0x80600000. Validator cross-checks; manifest +
    /// linker drift FAIL.
    #[serde(default = "default_reserved_size")]
    pub reserved_size: usize,
    pub binary:     String,
    pub stack_size: usize,
}

/// SAFE-3 CR-2: default 6MB matches sipahi.ld `_end ≤ 0x80600000` +
/// `NATIVE_TASK_BASE = 0x80600000` in src/common/config.rs:31.
fn default_reserved_size() -> usize {
    0x60_0000 // 6 MiB
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
    /// SAFE-2: per-task LOCAL_CAP_TABLE row — list of resource_id grants. Absent
    /// → deny-all row emitted. Codegen writes CapAction per (task, resource).
    #[serde(default, rename = "local_cap")]
    pub local_caps:    Vec<LocalCapGrant>,
    /// SAFE-4 (sprint-u33, Section 8 CR-5): per-task stack analysis safety
    /// margin override. Absent → `STACK_ANALYSIS_MARGIN_BYTES` const default
    /// (256). DAL-A için 512+ önerilir; advisory ama validator enforce eder.
    #[serde(default)]
    pub stack_margin_override: Option<u32>,
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

// ─── SAFE-2 schema (Section 8 CR-1..CR-5) ─────────────────────────

/// `[[resource]]` — static local capability table column. Each task entry in
/// `[[task.local_cap]]` references resource by `id`. LOCAL_CAP_TABLE codegen
/// emits one column per ResourceEntry, `MAX_RESOURCES` matches manifest count.
#[derive(Deserialize, Debug, Clone)]
pub struct ResourceEntry {
    pub id:   u8,
    pub name: String,
    /// "device" | "log" | "channel_endpoint" | "mailbox" | ... (free-form;
    /// validator accepts but flags unknown kinds with warning later).
    pub kind: String,
}

/// `[[channel]]` — typed IPC channel. Producer/consumer task names referenced
/// by `[[task]] name`; codegen emits `BOOT_CHANNELS` table + per-channel
/// typed `send_<name>` / `recv_<name>` wrappers.
#[derive(Deserialize, Debug, Clone)]
pub struct ChannelEntry {
    pub id:        u8,
    pub producer:  String,
    pub consumer:  String,
    pub message:   String,  // PascalCase struct name (e.g. "GreetingPing")
    pub size:      usize,   // repr(C) struct byte count — must be <= IPC_MSG_SIZE
    #[serde(default)]
    pub period_ms: Option<u32>,  // optional flow metadata (CR Section 4 FIX-D)
}

/// `[[task.local_cap]]` — sub-array on TaskEntry. Reserved for SAFE-2 manifest;
/// initial implementation reads it but produces a deny-all row if absent.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct LocalCapGrant {
    pub resource_id: u8,
    /// "Read" | "Write" | "ReadWrite" | "Execute" | "All" | "None"
    pub action:      String,
}
