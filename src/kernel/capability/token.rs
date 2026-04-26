//! 32-byte capability token: id, resource, action, nonce, expiry, MAC.
// Sipahi — Capability Token (Sprint 9)
// 32B sabit boyut, #[repr(C)], stack-only, heap yok
//
// Layout (DO-178C traceability — offset'ler sabit):
//   [0]      id       u8  — token identifier
//   [1]      task_id  u8  — owning task (0-7)
//   [2..3]   resource u16 — kaynak ID (IPC kanal, compute service, vb.)
//   [4]      action   u8  — izin bitfield (ACTION_*)
//   [5]      dal      u8  — DAL seviyesi (0=A 1=B 2=C 3=D)
//   [6..7]   _pad     [u8;2] — alignment padding, MAC hesabında sıfır olmalı
//   [8..11]  expires  u32 — son geçerlilik tick (0 = sonsuz)
//   [12..15] nonce    u32 — replay saldırısı önleme sayacı
//   [16..31] mac      [u8;16] — SipahiMAC (Sprint 9 stub, Sprint 13 BLAKE3)

use crate::common::types::{TaskId, ResourceId};

/// Capability action bitleri
#[allow(dead_code)]
pub const ACTION_READ:    u8 = 0x01;
#[allow(dead_code)]
pub const ACTION_WRITE:   u8 = 0x02;
#[allow(dead_code)]
pub const ACTION_EXECUTE: u8 = 0x04;
#[allow(dead_code)]
pub const ACTION_ALL:     u8 = 0x07;

/// 32-byte capability token
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Token {
    pub id:       u8,
    pub task_id:  TaskId,
    pub resource: ResourceId,
    pub action:   u8,
    pub dal:      u8,
    pub _pad:     [u8; 2],
    pub expires:  u32,
    pub nonce:    u32,
    pub mac:      [u8; 16],
}

impl Default for Token {
    fn default() -> Self { Self::zeroed() }
}

impl Token {
    pub const fn zeroed() -> Self {
        Token {
            id: 0, task_id: 0, resource: 0, action: 0,
            dal: 0, _pad: [0; 2], expires: 0, nonce: 0,
            mac: [0; 16],
        }
    }

    /// MAC hesabı için header bytes — ilk 16 byte, explicit LE encoding
    /// Güvenli: unsafe yok, endian-agnostik, DO-178C traceable
    pub const fn header_bytes(&self) -> [u8; 16] {
        let mut h = [0u8; 16];
        h[0]  = self.id;
        h[1]  = self.task_id;
        h[2]  = self.resource as u8;
        h[3]  = (self.resource >> 8) as u8;
        h[4]  = self.action;
        h[5]  = self.dal;
        h[6]  = 0; // _pad: MAC hesabında sıfır (deterministik)
        h[7]  = 0;
        h[8]  = self.expires as u8;
        h[9]  = (self.expires >> 8) as u8;
        h[10] = (self.expires >> 16) as u8;
        h[11] = (self.expires >> 24) as u8;
        h[12] = self.nonce as u8;
        h[13] = (self.nonce >> 8) as u8;
        h[14] = (self.nonce >> 16) as u8;
        h[15] = (self.nonce >> 24) as u8;
        h
    }
}

// Compile-time layout guarantee
const _: () = assert!(core::mem::size_of::<Token>() == 32);
