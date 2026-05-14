//! Sipahi SNTM task-side API library.
//!
//! v0.1: SNTM Phase 1 (Sprint U-23) iskelet. Syscall wrapper'lar v1.5'te
//! implement edilecek. Şu an sadece module structure hazır.
//!
//! Referans: `SIPAHI_SNTM_DESIGN.md` v0.7 §8 Sipahi API.
//!
//! Kullanım (v1.5+):
//! ```ignore
//! use sipahi_api::syscall;
//!
//! #[no_mangle]
//! pub extern "C" fn _start() -> ! {
//!     loop { syscall::yield_cpu(); }
//! }
//! ```

#![no_std]

/// Syscall wrapper'ları — v1.5'te implement edilecek.
///
/// v1.5 hedef API:
/// - `cap_invoke(cap: u8, resource: u16, action: u8) -> Result<(), Error>`
/// - `ipc_send(channel: u8, msg: &Message) -> Result<(), Error>`
/// - `ipc_recv(channel: u8) -> Result<Message, Error>`
/// - `yield_cpu()`
/// - `task_info(task_id: u8) -> TaskInfo`
/// - `exit(code: u8) -> !`  (SNTM design v0.5'te eksik gap olarak işaretlendi)
pub mod syscall {
    // v1.5 sprint U-23'te doldurulacak.
}

/// Kernel'den re-export edilen primitive'ler (CRC32 vb).
///
/// v1.5'te `pub use sipahi_kernel::ipc::crc32;` benzeri re-export.
pub mod crc {
    // v1.5'te eklenecek.
}

/// IPC mesaj tipleri — `Message`, `MessageHeader`.
///
/// SNTM SAFE-2 (v1.7) typed IPC ile manifest'ten generate edilecek.
pub mod ipc {
    // v1.5+'da eklenecek (typed IPC generator).
}

/// Task-side hata tipleri.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidArg,
    NoCapability,
    IpcFull,
    IpcEmpty,
    RateLimited,
}
