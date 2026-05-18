//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml [[resource]] + [[task.local_cap]] + [[channel]] entries.
//! Run `bash scripts/regen_safe_codegen.sh` to regenerate.
//! SAFE-2 (sprint-u31): static local capability table + boot channel ownership.
//!
//! Drift detection: CI runs sntm-validate --output-cap-table + git diff.

use crate::kernel::capability::cap_action::CapAction;

pub static LOCAL_CAP_TABLE: [[CapAction; 4]; 8] = [
    /* task 0 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
    /* task 1 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
    /* task 2 (task_hello) */ [CapAction::Write, CapAction::None, CapAction::Write, CapAction::None],
    /* task 3 (task_world) */ [CapAction::None, CapAction::Write, CapAction::Read, CapAction::None],
    /* task 4 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
    /* task 5 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
    /* task 6 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
    /* task 7 (<empty slot>) */ [CapAction::None, CapAction::None, CapAction::None, CapAction::None],
];

/// SAFE-2 (CR-5): boot.rs iterates this table to call
/// `ipc::assign_channel(id, producer, consumer)` for each entry.
/// Drift guard: manifest [[channel]] + this table = single source.
pub static BOOT_CHANNELS: &[(u8, u8, u8)] = &[
    (2, 2, 3),  // channel 2 "GreetingPing" task_hello → task_world
];
