//! Sipahi SNTM-SAFE task-lint library.
//!
//! U-30.1: bin + lib dual crate — bin entry main.rs, integration tests
//! `tools/task-lint/tests/integration.rs` lint API'sini direkt çağırır.

use serde::Deserialize;

pub mod lint;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "task")]
    pub tasks: Vec<TaskEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TaskEntry {
    pub name: String,
    pub dal_level: String,
    #[serde(default = "default_trust_tier")]
    pub trust_tier: String,
    #[serde(default)]
    pub waiver_reason: String,
    #[serde(default)]
    pub demo_feature_waivers: Vec<String>,
}

pub fn default_trust_tier() -> String {
    "safe".to_string()
}
