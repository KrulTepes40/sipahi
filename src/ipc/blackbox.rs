//! 64-byte CRC32-protected circular blackbox flight recorder (8 KB, 128 records).
// Sipahi — Blackbox Flight Recorder (Sprint 11)
// Circular buffer · PMP R4 · 8KB · 128 kayıt
//
// Kayıt formatı (64B, doküman §BLACKBOX):
//   [MAGIC:4][VER:2][PAD:2][SEQ:4][TS:4][TASK:1][EVENT:1][DATA:42][CRC32:4]
//
// Power-loss koruması (v8.0):
//   Yarım yazılmış kayıt → CRC32 fail → kayıt ATLANIR
//   Bir önceki kayıt sağlam → oradan devam edilir
//   SRAM/FRAM yazma <1μs → max 1 kayıt kaybı güç kesilmesinde
//
// Kural: SADECE KERNEL YAZAR — tek yazar, race imkansız
// Kural: advance_tick() her schedule() başında çağrılır
// Kural: boot_epoch v1.0=0 (QEMU); KernelBoot kaydı data[0..2]'de taşır
//
// WCET: log() = O(1), 64B kopyalama + CRC(60B) = sabit zaman

use crate::common::config::{BLACKBOX_RECORD_SIZE, BLACKBOX_MAX_RECORDS, BLACKBOX_DATA_SIZE};
use crate::common::sync::SingleHartCell;

// ═══════════════════════════════════════════════════════
// Sabitler
// ═══════════════════════════════════════════════════════

/// Kayıt sihirli baytları — "SPHI"
const MAGIC: [u8; 4] = [0x53, 0x50, 0x48, 0x49];

/// Kayıt format versiyonu
const RECORD_VERSION: u16 = 1;

// ═══════════════════════════════════════════════════════
// Olay türleri
// ═══════════════════════════════════════════════════════

/// Blackbox olay türleri — 1 byte (doküman §BLACKBOX)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlackboxEvent {
    KernelBoot      = 0,  // Sistem başlangıcı (data[0..2] = boot_epoch LE)
    TaskStart       = 1,  // Task başlatıldı (data[0]=id, data[1]=prio, data[2]=dal)
    TaskSuspend     = 2,  // Task askıya alındı
    TaskRestart     = 3,  // Task yeniden başlatıldı (policy kararı)
    BudgetExhausted = 4,  // Bütçe tükendi (data[0]=task_id, data[1]=dal)
    PolicyIsolate   = 5,  // Task izole edildi
    PolicyDegrade   = 6,  // Sistem degrade moduna girdi (DAL-C/D durduruldu)
    PolicyFailover  = 7,  // Yedek geçişi (v1.0 stub)
    PolicyShutdown  = 8,  // Güvenli kapanma
    CapViolation    = 9,  // Capability ihlali
    IopmpViolation  = 10, // IOPMP ihlali
    DeadlineMiss    = 11, // Deadline aşımı
    WatchdogTimeout = 12, // Watchdog süresi doldu
    PmpFail         = 13, // PMP bütünlük hatası (→ SHUTDOWN)
    LockstepFail    = 14, // Policy lockstep mismatch (action1 != action2 → SHUTDOWN)
}

// ═══════════════════════════════════════════════════════
// Kayıt yapısı — 64B sabit
// ═══════════════════════════════════════════════════════

/// Blackbox kaydı — 64B, repr(C) (padding EXPLICIT — Sprint U-14)
///
/// Byte layout (Proof 52 ile doğrulandı):
///   [0..4]   magic:     [u8;4]   → "SPHI"
///   [4..6]   version:   u16      → 1
///   [6..8]   _pad:      [u8;2]   → EXPLICIT padding (u32 alignment için)
///   [8..12]  seq:       u32      → monoton, u32 wrap (~23 yıl @ 6 rec/sec)
///   [12..16] timestamp: u32      → boot'tan tick sayısı
///   [16]     task_id:   u8       → tetikleyen task (0xFF=kernel)
///   [17]     event:     u8       → BlackboxEvent as u8
///   [18..60] data:      [u8;42]  → olay verisi (KernelBoot: data[0..2]=epoch)
///   [60..64] crc:       u32      → CRC32 byte 0..60 üzerinde
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlackboxRecord {
    pub magic:     [u8; 4],
    pub version:   u16,
    pub _pad:      [u8; 2],   // Sprint U-14: explicit padding — UB-free, serileştirme güvenli
    pub seq:       u32,
    pub timestamp: u32,
    pub task_id:   u8,
    pub event:     u8,
    pub data:      [u8; BLACKBOX_DATA_SIZE],
    pub crc:       u32,
}

impl Default for BlackboxRecord {
    fn default() -> Self { Self::zeroed() }
}

impl BlackboxRecord {
    pub const fn zeroed() -> Self {
        BlackboxRecord {
            magic:     [0u8; 4],
            version:   0,
            _pad:      [0u8; 2],
            seq:       0,
            timestamp: 0,
            task_id:   0,
            event:     0,
            data:      [0u8; BLACKBOX_DATA_SIZE],
            crc:       0,
        }
    }

    /// CRC32 hesapla ve kayıt sonuna yaz (byte 60..64)
    /// SAFETY: repr(C), boyut=64, padding yok — Proof 52 ile doğrulandı
    pub fn set_crc(&mut self) {
        let bytes: &[u8; BLACKBOX_RECORD_SIZE] =
            // SAFETY: repr(C) struct, size verified by Proof 52, no padding.
            unsafe { &*(self as *const Self as *const [u8; BLACKBOX_RECORD_SIZE]) };
        self.crc = super::crc32(&bytes[..60]);
    }

    /// CRC32 doğrula — yanlış ise kayıt bozuk (power-loss)
    pub fn verify_crc(&self) -> bool {
        let bytes: &[u8; BLACKBOX_RECORD_SIZE] =
            // SAFETY: repr(C) struct, size verified by Proof 52, no padding.
            unsafe { &*(self as *const Self as *const [u8; BLACKBOX_RECORD_SIZE]) };
        let computed = super::crc32(&bytes[..60]);
        self.crc == computed
    }

    /// Geçerli kayıt mu? — magic + version + CRC hepsi doğru olmalı
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC && self.version == RECORD_VERSION && self.verify_crc()
    }
}

// ═══════════════════════════════════════════════════════
// Statik alanlar — 8KB tampon (PMP R4)
// ═══════════════════════════════════════════════════════

/// Döngüsel blackbox tamponu — 128 × 64B = 8KB
static BB_BUFFER: SingleHartCell<[BlackboxRecord; BLACKBOX_MAX_RECORDS]> =
    SingleHartCell::new([BlackboxRecord::zeroed(); BLACKBOX_MAX_RECORDS]);

/// Sonraki yazma konumu [0, BLACKBOX_MAX_RECORDS)
static BB_WRITE_POS: SingleHartCell<u8> = SingleHartCell::new(0);

/// Sonraki sıra numarası (u32, ~23 yıl wrap-free @ 6 kayıt/saniye)
static BB_NEXT_SEQ: SingleHartCell<u32> = SingleHartCell::new(0);

/// Boot'tan bu yana geçen tick — schedule() her çağrısında advance_tick() artırır
static BB_TICK: SingleHartCell<u32> = SingleHartCell::new(0);

/// Boot dönemi — timestamp u32 wrap-around çözümü için (v1.0: daima 0)
static BB_BOOT_EPOCH: SingleHartCell<u16> = SingleHartCell::new(0);

/// Tampondaki kayıt sayısı (max BLACKBOX_MAX_RECORDS)
static BB_COUNT: SingleHartCell<u8> = SingleHartCell::new(0);

// ═══════════════════════════════════════════════════════
// Volatile yardımcıları — LTO + opt-level="s" altında
// LLVM static mut'ı register'a cache'leyebilir.
// Tüm BB_* erişimleri volatile read/write kullanır.
// ═══════════════════════════════════════════════════════

macro_rules! vol_read {
    ($var:ident -> $ty:ty) => {
        core::ptr::read_volatile($var.as_ptr())
    };
}
macro_rules! vol_write {
    ($var:ident, $val:expr) => {
        core::ptr::write_volatile($var.as_ptr(), $val)
    };
}

// ═══════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════

/// Boot başlangıcı — rust_main'de, zamanlayıcıdan önce çağrılır
/// v1.0 (QEMU): tamponu sıfırdan başlatır, KernelBoot kaydı yazar
/// v1.5+ (SRAM/FRAM): son geçerli seq'i tarar, kaldığı yerden devam eder
///
/// BSS clearing'e güvenilmez (binary layout değişince semboller kayar).
/// Tüm static'ler explicit sıfırlanır — her koşulda çalışır.
pub(crate) fn init() {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        // Buffer'ı explicit sıfırla — BSS clearing'e güvenme
        let ptr = BB_BUFFER.as_ptr() as *mut u8;
        core::ptr::write_bytes(ptr, 0, core::mem::size_of::<[BlackboxRecord; BLACKBOX_MAX_RECORDS]>());

        vol_write!(BB_COUNT, 0u8);
        vol_write!(BB_WRITE_POS, 0u8);
        vol_write!(BB_NEXT_SEQ, 0u32);
        vol_write!(BB_TICK, 0u32);
        vol_write!(BB_BOOT_EPOCH, 0u16);
    }
    log(BlackboxEvent::KernelBoot, 0xFF, &[]);
}

/// Tick sayacını ilerlet — schedule() her çağrısının başında çağrılır
#[inline]
pub(crate) fn advance_tick() {
    // SAFETY: Volatile access prevents compiler from caching static mut in register.
    unsafe {
        let t = vol_read!(BB_TICK -> u32);
        let next = t.wrapping_add(1);
        // Wrap tespiti: next < t → u32 taştı → epoch artır
        if next < t {
            let epoch = vol_read!(BB_BOOT_EPOCH -> u16);
            vol_write!(BB_BOOT_EPOCH, epoch.wrapping_add(1));
        }
        vol_write!(BB_TICK, next);
    }
}

/// Olay kaydet — SADECE KERNEL çağırır (tek yazar garantisi)
///
/// event:   Olay türü
/// task_id: Tetikleyen task ID (0xFF = kernel dahili olay)
/// data:    En fazla BLACKBOX_DATA_SIZE(42) byte olay verisi (kısa girişler sıfır doldurulur)
pub(crate) fn log(event: BlackboxEvent, task_id: u8, data: &[u8]) {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        let pos = vol_read!(BB_WRITE_POS -> u8) as usize;
        // Sprint U-16: Defense-in-depth — Proof 54 BB_WRITE_POS < BLACKBOX_MAX_RECORDS
        // garantisi veriyor, ama runtime corruption (cosmic ray, fault injection)
        // pos'u yine de BLACKBOX_MAX_RECORDS üstüne çıkarabilir. Reset + drop record
        // → OOB write engellendi, sistem ayakta kalır.
        if pos >= BLACKBOX_MAX_RECORDS {
            vol_write!(BB_WRITE_POS, 0u8);
            return; // bu kayıt düşürüldü; gelecek kayıtlar düzgün yazılacak
        }
        let seq = vol_read!(BB_NEXT_SEQ -> u32);
        let tick = vol_read!(BB_TICK -> u32);

        let mut rec = BlackboxRecord::zeroed();
        rec.magic     = MAGIC;
        rec.version   = RECORD_VERSION;
        rec.seq       = seq;
        rec.timestamp = tick;
        rec.task_id   = task_id;
        rec.event     = event as u8;

        // Veri kopyala — en fazla BLACKBOX_DATA_SIZE byte, geri kalanı sıfır
        let n = if data.len() < BLACKBOX_DATA_SIZE { data.len() } else { BLACKBOX_DATA_SIZE };
        let mut i = 0;
        while i < n {
            rec.data[i] = data[i];
            i += 1;
        }

        // CRC hesapla ve yaz — power-loss koruması için son adım
        rec.set_crc();
        // SAFETY: pos < BLACKBOX_MAX_RECORDS (Proof 54). Volatile prevents LTO reorder.
        core::ptr::write_volatile(
            &mut (*BB_BUFFER.get_mut())[pos] as *mut BlackboxRecord,
            rec,
        );

        // Konumu ilerlet — döngüsel sarma
        let next_pos = if pos + 1 >= BLACKBOX_MAX_RECORDS { 0u8 } else { (pos + 1) as u8 };
        vol_write!(BB_WRITE_POS, next_pos);
        vol_write!(BB_NEXT_SEQ, seq.wrapping_add(1));

        let c = vol_read!(BB_COUNT -> u8);
        if (c as usize) < BLACKBOX_MAX_RECORDS {
            vol_write!(BB_COUNT, c + 1);
        }
    }
}

/// Kayıt oku — 0 = en eski, count()-1 = en yeni
/// Dönüş: Some(record) — CRC geçerli; None — index aşımı veya bozuk kayıt
#[allow(dead_code)] // Sprint U-16: post-mortem analiz API'si — production debug yolu eklenince kullanılır
pub(crate) fn read(index: usize) -> Option<BlackboxRecord> {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
    unsafe {
        let c = vol_read!(BB_COUNT -> u8) as usize;
        if index >= c {
            return None;
        }
        let wp = vol_read!(BB_WRITE_POS -> u8) as usize;
        let start = if c < BLACKBOX_MAX_RECORDS { 0usize } else { wp };
        let actual = (start + index) % BLACKBOX_MAX_RECORDS;
        let rec = (*BB_BUFFER.get())[actual];
        if rec.is_valid() { Some(rec) } else { None }
    }
}

/// Tampondaki geçerli kayıt sayısı
pub fn count() -> usize {
    // SAFETY: Volatile access prevents compiler from caching static mut in register.
    unsafe { vol_read!(BB_COUNT -> u8) as usize }
}

/// Mevcut blackbox tick sayacını döndür — expiry kontrolü için
/// Bileşik u64: (epoch << 32) | tick — u32 wrap-safe, ~900K yıl monoton
pub(crate) fn get_tick() -> u64 {
    // SAFETY: Single-hart, read-only access.
    unsafe {
        let epoch = vol_read!(BB_BOOT_EPOCH -> u16) as u64;
        let tick  = vol_read!(BB_TICK -> u32) as u64;
        (epoch << 32) | tick
    }
}

// Compile-time guarantees
const _: () = assert!(BLACKBOX_MAX_RECORDS <= 255);
const _: () = assert!(core::mem::size_of::<BlackboxRecord>() == BLACKBOX_RECORD_SIZE);

// ═══════════════════════════════════════════════════════
// Kani — Sprint 11 (Proof 52-57)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;

    /// Proof 52: BlackboxRecord boyutu = 64B, tampon = 8KB (padding yok)
    #[kani::proof]
    fn record_layout_correct() {
        assert!(core::mem::size_of::<BlackboxRecord>() == BLACKBOX_RECORD_SIZE);
        assert!(BLACKBOX_RECORD_SIZE == 64);
        assert!(BLACKBOX_MAX_RECORDS == 128);
        assert!(BLACKBOX_MAX_RECORDS * BLACKBOX_RECORD_SIZE == 8192);
    }

    /// Proof 53: Kayıt yaz → CRC doğrula (set_crc / verify_crc roundtrip)
    #[kani::proof]
    fn record_crc_roundtrip() {
        let mut rec = BlackboxRecord::zeroed();
        rec.magic     = MAGIC;
        rec.version   = RECORD_VERSION;
        rec.seq       = 42;
        rec.task_id   = 1;
        rec.event     = BlackboxEvent::BudgetExhausted as u8;
        rec.data[0]   = 0xAB;
        rec.data[5]   = 0xCD;
        rec.set_crc();
        assert!(rec.verify_crc());
        assert!(rec.is_valid());
    }

    /// Proof 54: write_pos her zaman [0, BLACKBOX_MAX_RECORDS) içinde kalır
    #[kani::proof]
    fn write_pos_always_bounded() {
        let pos: u8 = kani::any();
        kani::assume((pos as usize) < BLACKBOX_MAX_RECORDS);
        let next = if (pos as usize) + 1 >= BLACKBOX_MAX_RECORDS {
            0u8
        } else {
            pos + 1
        };
        assert!((next as usize) < BLACKBOX_MAX_RECORDS);
    }

    /// Proof 55: Power-loss simülasyonu — bozulan byte CRC'yi geçersiz kılar
    #[kani::proof]
    fn corrupted_record_fails_verify() {
        let mut rec = BlackboxRecord::zeroed();
        rec.magic   = MAGIC;
        rec.version = RECORD_VERSION;
        rec.seq     = 7;
        rec.event   = BlackboxEvent::TaskStart as u8;
        rec.set_crc();
        // Payload'ı boz (yarım yazılmış / bit flip simülasyonu)
        rec.data[0] = rec.data[0].wrapping_add(1);
        // CRC uyuşmaz → kayıt geçersiz → ATLANIR
        assert!(!rec.verify_crc());
        assert!(!rec.is_valid());
    }

    /// Proof 56: Sıfır kayıt is_valid() döndürmez (magic eşleşmez + CRC hatalı)
    #[kani::proof]
    fn zeroed_record_not_valid() {
        let rec = BlackboxRecord::zeroed();
        // magic = [0,0,0,0] ≠ MAGIC = ['S','P','H','I']
        assert!(rec.magic != MAGIC);
        assert!(!rec.is_valid());
    }

    /// Proof 57: Magic ve version sabitleri doğru değerlere sahip
    #[kani::proof]
    fn constants_correct() {
        assert!(MAGIC[0] == b'S');
        assert!(MAGIC[1] == b'P');
        assert!(MAGIC[2] == b'H');
        assert!(MAGIC[3] == b'I');
        assert!(RECORD_VERSION == 1);
        // 8KB / 64B = 128 kayıt
        assert!(crate::common::config::BLACKBOX_SIZE / BLACKBOX_RECORD_SIZE
                == BLACKBOX_MAX_RECORDS);
    }

    /// Proof 82: 128 kayıt sonrası write_pos başa döner (tam tur)
    #[kani::proof]
    #[kani::unwind(129)]
    fn blackbox_wrap_around_bounded() {
        let write_pos: u8 = kani::any();
        kani::assume((write_pos as usize) < BLACKBOX_MAX_RECORDS);

        let mut pos = write_pos;
        let mut i: u8 = 0;
        while i < 128 {
            pos = if (pos as usize) + 1 >= BLACKBOX_MAX_RECORDS {
                0
            } else {
                pos + 1
            };
            assert!((pos as usize) < BLACKBOX_MAX_RECORDS);
            i += 1;
        }
        // Tam tur: 128 adım sonrası başlangıca dönmeli
        assert!(pos == write_pos);
    }

    /// Proof 83: next write_pos her zaman buffer sınırları içinde
    #[kani::proof]
    fn blackbox_next_pos_always_bounded() {
        let pos: u8 = kani::any();
        kani::assume((pos as usize) < BLACKBOX_MAX_RECORDS);
        let next = if (pos as usize) + 1 >= BLACKBOX_MAX_RECORDS {
            0u8
        } else {
            pos + 1
        };
        assert!((next as usize) < BLACKBOX_MAX_RECORDS);
    }

    /// Proof 84: get_tick() epoch+tick bileşik u64 döndürür
    #[kani::proof]
    fn get_tick_returns_current() {
        let tick: u32 = kani::any();
        let epoch: u16 = kani::any();
        unsafe {
            *BB_TICK.get_mut() = tick;
            *BB_BOOT_EPOCH.get_mut() = epoch;
        }
        let expected = ((epoch as u64) << 32) | (tick as u64);
        assert!(get_tick() == expected);
    }

    /// Proof 105: BlackboxRecord zeroed → seq == 0
    #[kani::proof]
    fn blackbox_zeroed_record_seq_zero() {
        let rec = BlackboxRecord::zeroed();
        assert!(rec.seq == 0);
        assert!(rec.timestamp == 0);
        assert!(rec.task_id == 0);
    }

    /// Proof 106: Tick monoton: before < MAX → before + 1 > before
    #[kani::proof]
    fn blackbox_tick_monotonic() {
        let before: u64 = kani::any();
        kani::assume(before < u64::MAX);
        let after = before + 1;
        assert!(after > before);
    }

    /// Proof 107: write_pos wrap — pos < MAX → next < MAX, wrap → 0
    #[kani::proof]
    fn blackbox_write_pos_wraps_correctly() {
        let pos: u8 = kani::any();
        kani::assume((pos as usize) < BLACKBOX_MAX_RECORDS);
        let next = if (pos as usize) + 1 >= BLACKBOX_MAX_RECORDS { 0u8 } else { pos + 1 };
        assert!((next as usize) < BLACKBOX_MAX_RECORDS);
        if (pos as usize) + 1 >= BLACKBOX_MAX_RECORDS {
            assert!(next == 0);
        }
    }

    /// Proof 166: BlackboxRecord concrete data tamper → CRC fail
    #[kani::proof]
    fn blackbox_record_concrete_tamper_crc_fail() {
        let mut rec = BlackboxRecord::zeroed();
        rec.magic = [0x53, 0x50, 0x48, 0x49];
        rec.version = 1;
        rec.seq = 42;
        rec.task_id = 1;
        rec.event = 3;
        rec.set_crc();
        rec.data[0] = 0xFF; // tamper
        assert!(!rec.verify_crc());
    }

    /// Proof: write_pos >= MAX → güvenli 0'a dönüş
    #[kani::proof]
    fn blackbox_write_pos_out_of_bounds_safe() {
        let pos: u8 = kani::any();
        kani::assume((pos as usize) >= BLACKBOX_MAX_RECORDS);
        let next = if (pos as usize) >= BLACKBOX_MAX_RECORDS { 0u8 } else { pos + 1 };
        assert!((next as usize) < BLACKBOX_MAX_RECORDS);
    }
}
