//! SAFE-2 (sprint-u31): static local capability check — fast path (~5c).
//!
//! Section 17.5 of SIPAHI_SNTM_DESIGN.md (Static Local Capability Table).
//! Replaces MAC token validation for **same-image task → kernel resource**
//! access (cache miss path 400c → 5c, EN BÜYÜK NET WIN of SNTM-SAFE).
//!
//! Cross-hart (AMCI), HSM-provisioned, and external attestation paths MAC
//! YOLU AYNEN KORUR — see `broker::validate_full`. This module ONLY covers
//! local resources whose access matrix is build-time decidable from the
//! manifest `[[task.local_cap]]` grants.
//!
//! ABI: `sys_cap_invoke` discriminates via cap argument bit 7 (Section 8
//! CR-1). bit 7 = 1 → this module; bit 7 = 0 → `broker::validate_cached`.

use crate::common::config::{MAX_TASKS, MAX_RESOURCES};
use crate::kernel::capability::cap_action::CapAction;
use crate::kernel::capability::cap_generated::LOCAL_CAP_TABLE;

/// Static local capability check (~5c fast path).
///
/// SAFE invariants (CR-1, CR-3):
///   - `caller_task_id >= MAX_TASKS` → DENY (OOB row).
///   - `resource_id   >= MAX_RESOURCES` → DENY (OOB column).
///   - `action_bits` not in {0,1,2,3,4,7} → DENY (invalid bit pattern,
///     CapAction::from_u8 returns None).
///   - Granted cell `allows(requested)` → bit-wise subset check.
///
/// Returns `true` iff the static manifest grants the requested action on the
/// resource to the caller task.
#[inline]
pub fn local_cap_check(caller_task_id: u8, resource_id: u8, action_bits: u8) -> bool {
    if caller_task_id as usize >= MAX_TASKS {
        return false;
    }
    if resource_id as usize >= MAX_RESOURCES {
        return false;
    }
    let requested = match CapAction::from_u8(action_bits) {
        Some(a) => a,
        None    => return false,   // CR-3: invalid action MUST DENY (never permissive)
    };
    LOCAL_CAP_TABLE[caller_task_id as usize][resource_id as usize].allows(requested)
}
