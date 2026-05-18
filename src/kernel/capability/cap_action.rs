//! SAFE-2 (sprint-u31): static local capability action enum.
//!
//! Bit-wise compatible with `ACTION_READ=0x01`, `ACTION_WRITE=0x02`,
//! `ACTION_EXECUTE=0x04`, `ACTION_ALL=0x07` (see `token.rs`). The enum is the
//! **source-of-truth** for codegen (`cap_generated.rs`); the bit constants
//! remain for legacy MAC token path. Section 8 CR-3 doctrine:
//!
//!   - Invalid bit combinations → `from_u8` returns `None`.
//!   - `local_cap_check` MUST treat `None` as DENY (never permissive default).
//!   - `allows()` is bit-wise subset: `ReadWrite.allows(Read) == true`.
//!
//! Kernel uses this enum directly; `sipahi_api` task-side passes `u8` over the
//! ecall ABI. Cross-crate drift is impossible because the enum is kernel-only.

#![allow(dead_code)] // Slot reserved for SAFE-2 feature; production wiring G4.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CapAction {
    None      = 0x00,
    Read      = 0x01,   // ACTION_READ
    Write     = 0x02,   // ACTION_WRITE
    ReadWrite = 0x03,   // R | W
    Execute   = 0x04,   // ACTION_EXECUTE
    All       = 0x07,   // ACTION_ALL (R | W | X)
}

impl CapAction {
    /// SAFE invariant (Section 8 CR-3): invalid u8 → None (DENY).
    /// Never permissive default; security regression guard.
    ///
    /// Valid encodings: 0x00, 0x01, 0x02, 0x03, 0x04, 0x07.
    /// All other bit patterns (0x05, 0x06, 0x08..=0xFF) are rejected.
    #[inline]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(CapAction::None),
            0x01 => Some(CapAction::Read),
            0x02 => Some(CapAction::Write),
            0x03 => Some(CapAction::ReadWrite),
            0x04 => Some(CapAction::Execute),
            0x07 => Some(CapAction::All),
            _    => None,
        }
    }

    /// Bit-wise subset: does `self` grant include the requested action?
    ///
    /// Examples:
    ///   `Read.allows(Read)       == true`
    ///   `ReadWrite.allows(Read)  == true`
    ///   `ReadWrite.allows(Write) == true`
    ///   `Read.allows(Write)      == false`
    ///   `All.allows(*)           == true` for any non-None action
    ///   `None.allows(*)          == false` (None = deny-all column)
    ///
    /// Invariant: requested == None → false (asking "permission to do nothing"
    /// is meaningless and should not consume a capability grant).
    #[inline]
    pub const fn allows(self, requested: CapAction) -> bool {
        let granted = self as u8;
        let asked   = requested as u8;
        (granted & asked == asked) && (asked != 0)
    }
}
