//! sipahi.toml parse — riscv-bin-verify yalnız ihtiyacı kadar şema okur.
//!
//! Section 4 FIX-G doctrine: ortak `sntm-manifest` lib crate önerildi ama
//! SAFE-3 scope'unda DEFER. Şu an verifier minimal struct subset kullanır
//! (region boundary check için sadece TaskEntry.name + regions[base, size]).
//! sntm-validate struct'ı ile drift riski: SAFE-4 carry-forward.

#![allow(dead_code)] // field'ların hepsi şimdi kullanılmıyor; manifest schema mirror için.

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub kernel:   KernelEntry,
    #[serde(default, rename = "task")]
    pub tasks:    Vec<TaskEntry>,
}

#[derive(Deserialize, Debug)]
pub struct KernelEntry {
    pub name:    String,
    pub version: String,
    /// SAFE-3 CR-2: kernel image reserved range; native task region kernel ile
    /// overlap'ı engellemek için min başlangıç. Default 6MB (sntm-validate doc).
    #[serde(default = "default_reserved_size")]
    pub reserved_size: usize,
}

fn default_reserved_size() -> usize {
    0x60_0000 // 6 MiB — sipahi.ld _end ≤ 0x80600000
}

#[derive(Deserialize, Debug, Clone)]
pub struct TaskEntry {
    pub name:    String,
    pub task_id: u8,
    #[serde(default, rename = "region")]
    pub regions: Vec<RegionEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegionEntry {
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub perm: String,  // "RX", "R", "RW", "NONE"
}

impl TaskEntry {
    /// Return true iff `addr` falls in any of this task's [[region]] entries.
    pub fn contains_addr(&self, addr: usize) -> bool {
        self.regions.iter().any(|r| {
            addr >= r.base && addr < r.base.saturating_add(r.size)
        })
    }
}
