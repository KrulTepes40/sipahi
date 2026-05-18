//! GENERATED FILE — DO NOT EDIT.
//!
//! Source: sipahi.toml [[channel]] entries.
//! Run `bash scripts/regen_safe_codegen.sh` to regenerate.
//! SAFE-2 (sprint-u31): typed IPC API per CR-4 safety gate template.
//!
//! Drift detection: CI runs sntm-validate --output-channels + git diff.
//!
//! Per CR-4 safety gates: each struct enforces:
//!   - size_of::<T>() == manifest size_field    (compile-time assert)
//!   - size_of::<T>() <= IPC_MSG_SIZE           (compile-time assert)
//!   - align_of::<T>() <= 8                     (compile-time assert)
//!   - repr(C, align(8)) for stable ABI
//!   - send: Message::empty() before copy → no padding leak
//!   - recv: copy_nonoverlapping(_, _, size_of::<T>()) → only first N bytes

// Without per-task features, the cfg-gated wrappers vanish and the imports
// look unused — but they are referenced by the generated send/recv bodies
// once any `task_*` feature is enabled. Suppress the false-positive warning
// (also dodges a rustc 1.96 annotate_snippets ICE on the warning render).
#![allow(unused_imports)]

use crate::ipc::Message;
use crate::syscall;
use crate::Error;

// ─── channel 2 (task_hello → task_world) ───────────────────────────

/// Typed IPC message body for channel 2 (GreetingPing).
/// SAFE-2 invariant: size = 16 bytes, repr(C, align(8)).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct GreetingPing {
    /// Opaque payload (manifest size = 16 bytes).
    pub bytes: [u8; 16],
}

// CR-4 compile-time safety gates
const _: () = assert!(core::mem::size_of::<GreetingPing>() == 16);
const _: () = assert!(core::mem::size_of::<GreetingPing>() <= Message::SIZE);
const _: () = assert!(core::mem::align_of::<GreetingPing>() <= 8);

/// Send a typed `GreetingPing` message on channel 2 (task_hello → task_world).
/// Cfg-gated: only compiles when `task_task_hello` feature is enabled.
#[cfg(feature = "task_task_hello")]
#[inline]
pub fn send_greeting_ping(msg: &GreetingPing) -> Result<(), Error> {
    const CHANNEL_ID: u8 = 2;
    // CR-4: zero-init buf prevents padding-byte leakage; only size_of::<T>() bytes copied.
    let mut buf = Message::empty();
    // SAFETY: T is repr(C, align(8)); size asserted at compile time; buf has IPC_MSG_SIZE bytes;
    // source and dest non-overlapping (distinct stack allocations).
    unsafe {
        core::ptr::copy_nonoverlapping(
            msg as *const _ as *const u8,
            buf.data.as_mut_ptr(),
            core::mem::size_of::<GreetingPing>(),
        );
    }
    syscall::ipc_send(CHANNEL_ID, &buf)
}

/// Receive a typed `GreetingPing` message on channel 2 (task_hello → task_world).
/// `Ok(None)` = empty channel (non-blocking).
/// Cfg-gated: only compiles when `task_task_world` feature is enabled.
#[cfg(feature = "task_task_world")]
#[inline]
pub fn recv_greeting_ping() -> Result<Option<GreetingPing>, Error> {
    const CHANNEL_ID: u8 = 2;
    let mut buf = Message::empty();
    match syscall::ipc_recv(CHANNEL_ID, &mut buf) {
        Ok(true) => {
            let mut out = core::mem::MaybeUninit::<GreetingPing>::uninit();
            // SAFETY: kernel returned IPC_MSG_SIZE bytes; we read only size_of::<T>().
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf.data.as_ptr(),
                    out.as_mut_ptr() as *mut u8,
                    core::mem::size_of::<GreetingPing>(),
                );
                Ok(Some(out.assume_init()))
            }
        }
        Ok(false) => Ok(None),
        Err(e) => Err(e),
    }
}

