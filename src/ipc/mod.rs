//! SPSC lock-free IPC ring buffers with CRC32 integrity and host-call limiting.
// Sipahi — IPC: SPSC Ring Buffer + Blackbox Flight Recorder
// Sprint 8: SPSC kanallar
// Sprint 11: Blackbox kayıt
// Single-Producer Single-Consumer lock-free kanal
//
// Doktrin: AtomicU16 head/tail, O(1) send/recv
// ISR/task arası güvenli: Release/Acquire ordering
// Mesaj boyutu: 64 byte sabit (IPC_MSG_SIZE)
// Kanal sayısı: 8 statik (MAX_IPC_CHANNELS)
// Slot sayısı: 16 per kanal (IPC_CHANNEL_SLOTS)
//
// CRC32: her mesajın son 4 byte'ı CRC (payload 60 byte)

pub mod blackbox; // Sprint 11: flight recorder

use core::sync::atomic::{AtomicU16, Ordering};
use crate::common::config::{IPC_MSG_SIZE, IPC_CHANNEL_SLOTS, MAX_IPC_CHANNELS};

// ═══════════════════════════════════════════════════════
// IPC Mesaj yapısı — 64 byte sabit
// ═══════════════════════════════════════════════════════

/// 64 byte IPC mesajı
/// [0..59] = payload (60 byte)
/// [60..63] = CRC32 (4 byte, little-endian)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpcMessage {
    pub data: [u8; IPC_MSG_SIZE],
}

impl IpcMessage {
    pub const fn zeroed() -> Self {
        IpcMessage {
            data: [0u8; IPC_MSG_SIZE],
        }
    }

    /// Payload'a CRC32 hesapla ve son 4 byte'a yaz
    pub fn set_crc(&mut self) {
        let crc = crc32(&self.data[..60]);
        self.data[60] = crc as u8;
        self.data[61] = (crc >> 8) as u8;
        self.data[62] = (crc >> 16) as u8;
        self.data[63] = (crc >> 24) as u8;
    }

    /// CRC32 doğrula
    pub fn verify_crc(&self) -> bool {
        let stored = u32::from_le_bytes([
            self.data[60], self.data[61],
            self.data[62], self.data[63],
        ]);
        let computed = crc32(&self.data[..60]);
        stored == computed
    }
}

// ═══════════════════════════════════════════════════════
// SPSC Ring Buffer — lock-free, O(1)
// ═══════════════════════════════════════════════════════

/// SPSC kanal — tek producer, tek consumer
pub struct SpscChannel {
    /// Yazma pozisyonu (producer artırır)
    head: AtomicU16,
    /// Okuma pozisyonu (consumer artırır)
    tail: AtomicU16,
    /// Mesaj slot'ları — statik array
    slots: [IpcMessage; IPC_CHANNEL_SLOTS],
}
 impl Default for SpscChannel {
    fn default() -> Self {
        Self::new()
    }
}
impl SpscChannel {
    pub const fn new() -> Self {
        SpscChannel {
            head: AtomicU16::new(0),
            tail: AtomicU16::new(0),
            slots: [IpcMessage::zeroed(); IPC_CHANNEL_SLOTS],
        }
    }

    /// Mesaj gönder — O(1), lock-free
    /// Producer çağırır. Buffer doluysa Err döner.
    #[allow(clippy::result_unit_err)] 
    pub fn send(&mut self, msg: &IpcMessage) -> Result<(), ()> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        let next_head = (head + 1) % (IPC_CHANNEL_SLOTS as u16);

        // Buffer dolu mu?
        if next_head == tail {
            return Err(()); // BufferFull
        }

        // Mesajı kopyala
        self.slots[head as usize] = *msg;

        // Head'i ilerlet — Release: consumer bu yazımı görsün
        self.head.store(next_head, Ordering::Release);

        Ok(())
    }

    /// Mesaj al — O(1), lock-free
    /// Consumer çağırır. Buffer boşsa None döner.
    pub fn recv(&mut self) -> Option<IpcMessage> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        // Buffer boş mu?
        if tail == head {
            return None; // Empty
        }

        // Mesajı kopyala
        let msg = self.slots[tail as usize];

        // Tail'i ilerlet — Release: producer bu okumayı görsün
        let next_tail = (tail + 1) % (IPC_CHANNEL_SLOTS as u16);
        self.tail.store(next_tail, Ordering::Release);

        Some(msg)
    }

    /// Kanal dolu mu?
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let next_head = (head + 1) % (IPC_CHANNEL_SLOTS as u16);
        next_head == tail
    }

    /// Kanal boş mu?
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        tail == head
    }

    /// Kaçç mesaj var?
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire) as usize;
        let tail = self.tail.load(Ordering::Relaxed) as usize;
        if head >= tail {
            head - tail
        } else {
            IPC_CHANNEL_SLOTS - tail + head
        }
    }
}

// ═══════════════════════════════════════════════════════
// IPC Pool — 8 statik kanal
// ═══════════════════════════════════════════════════════

/// 8 SPSC kanal — statik, heap yok
static mut IPC_CHANNELS: [SpscChannel; MAX_IPC_CHANNELS] = [
    SpscChannel::new(), SpscChannel::new(),
    SpscChannel::new(), SpscChannel::new(),
    SpscChannel::new(), SpscChannel::new(),
    SpscChannel::new(), SpscChannel::new(),
];

/// Kanal referansı al (bounds check dahil)
pub fn get_channel(id: usize) -> Option<&'static mut SpscChannel> {
    if id >= MAX_IPC_CHANNELS {
        return None;
    }
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe { Some(&mut IPC_CHANNELS[id]) }
}

// ═══════════════════════════════════════════════════════
// CRC32 — yazılımsal, lookup table yok (deterministic)
// ═══════════════════════════════════════════════════════

/// CRC32 (IEEE 802.3) — bit-by-bit, no lookup table
/// Deterministic: WCET = O(n × 8), n = veri uzunluğu
/// 60 byte payload → 60 × 8 = 480 iterasyon (bounded)
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    let mut i = 0;
    while i < data.len() {
        crc ^= data[i] as u32;
        let mut bit = 0;
        while bit < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            bit += 1;
        }
        i += 1;
    }
    !crc
}

// ═══════════════════════════════════════════════════════
// Kani Formal Verification — Sprint 8
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;

    /// Proof 33: Boş kanaldan recv → None
    #[kani::proof]
    fn empty_channel_recv_none() {
        let mut ch = SpscChannel::new();
        assert!(ch.recv().is_none());
        assert!(ch.is_empty());
        assert!(!ch.is_full());
        assert!(ch.len() == 0);
    }

    /// Proof 34: Send sonra recv → aynı veri
    #[kani::proof]
    fn send_recv_roundtrip() {
        let mut ch = SpscChannel::new();
        let mut msg = IpcMessage::zeroed();
        msg.data[0] = 0x42;
        msg.data[1] = 0xAB;
        msg.set_crc();

        assert!(ch.send(&msg).is_ok());
        assert!(!ch.is_empty());
        assert!(ch.len() == 1);

        let received = ch.recv().unwrap();
        assert!(received.data[0] == 0x42);
        assert!(received.data[1] == 0xAB);
        assert!(received.verify_crc());
        assert!(ch.is_empty());
    }

    /// Proof 35: IpcMessage boyutu == IPC_MSG_SIZE
    #[kani::proof]
    fn message_size_correct() {
        assert!(core::mem::size_of::<IpcMessage>() == IPC_MSG_SIZE);
        assert!(IPC_MSG_SIZE == 64);
    }

    /// Proof 36: CRC32 bilinen test vektörü
    #[kani::proof]
    fn crc32_known_vector() {
        // CRC32("123456789") = 0xCBF43926 (IEEE 802.3)
        let data = [0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39];
        let result = crc32(&data);
        assert!(result == 0xCBF4_3926);
    }

    /// Proof 37: CRC set/verify roundtrip
    #[kani::proof]
    fn crc_set_verify_roundtrip() {
        let mut msg = IpcMessage::zeroed();
        msg.data[0] = 0xFF;
        msg.data[5] = 0x42;
        msg.set_crc();
        assert!(msg.verify_crc());
    }

    /// Proof 38: Bozuk CRC → verify false
    #[kani::proof]
    fn crc_tampered_fails() {
        let mut msg = IpcMessage::zeroed();
        msg.data[0] = 0xFF;
        msg.set_crc();
        // Payload'ı boz
        msg.data[0] = 0x00;
        assert!(!msg.verify_crc());
    }

    /// Proof 39: Kanal ID bounds check
    #[kani::proof]
    fn channel_id_bounds() {
        assert!(get_channel(0).is_some());
        assert!(get_channel(7).is_some());
        assert!(get_channel(8).is_none());
        assert!(get_channel(usize::MAX).is_none());
    }
}
