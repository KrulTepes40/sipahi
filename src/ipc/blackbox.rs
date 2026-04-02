// Sipahi — Blackbox Flight Recorder (Sprint 11)
// Circular buffer · PMP R4 · 8KB · 128 kayıt
//
// Kayıt formatı (64B, doküman §BLACKBOX):
//   [MAGIC:4][VER:2][SEQ:2][TS:4][TASK:1][EVENT:1][DATA:46][CRC32:4]
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

use crate::common::config::{BLACKBOX_RECORD_SIZE, BLACKBOX_MAX_RECORDS};

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
}

// ═══════════════════════════════════════════════════════
// Kayıt yapısı — 64B sabit
// ═══════════════════════════════════════════════════════

/// Blackbox kaydı — 64B, padding yok (repr(C) + hizalama kanıtlandı)
///
/// Byte layout (Proof 52 ile doğrulandı):
///   [0..4]   magic:     [u8;4]   → "SPHI"
///   [4..6]   version:   u16      → 1
///   [6..8]   seq:       u16      → monoton, u16 wrap
///   [8..12]  timestamp: u32      → boot'tan tick sayısı
///   [12]     task_id:   u8       → tetikleyen task (0xFF=kernel)
///   [13]     event:     u8       → BlackboxEvent as u8
///   [14..60] data:      [u8;46]  → olay verisi (KernelBoot: data[0..2]=epoch)
///   [60..64] crc:       u32      → CRC32 byte 0..60 üzerinde
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlackboxRecord {
    pub magic:     [u8; 4],
    pub version:   u16,
    pub seq:       u16,
    pub timestamp: u32,
    pub task_id:   u8,
    pub event:     u8,
    pub data:      [u8; 46],
    pub crc:       u32,
}

impl BlackboxRecord {
    pub const fn zeroed() -> Self {
        BlackboxRecord {
            magic:     [0u8; 4],
            version:   0,
            seq:       0,
            timestamp: 0,
            task_id:   0,
            event:     0,
            data:      [0u8; 46],
            crc:       0,
        }
    }

    /// CRC32 hesapla ve kayıt sonuna yaz (byte 60..64)
    /// SAFETY: repr(C), boyut=64, padding yok — Proof 52 ile doğrulandı
    pub fn set_crc(&mut self) {
        let bytes: &[u8; BLACKBOX_RECORD_SIZE] =
            unsafe { &*(self as *const Self as *const [u8; BLACKBOX_RECORD_SIZE]) };
        self.crc = super::crc32(&bytes[..60]);
    }

    /// CRC32 doğrula — yanlış ise kayıt bozuk (power-loss)
    pub fn verify_crc(&self) -> bool {
        let bytes: &[u8; BLACKBOX_RECORD_SIZE] =
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
static mut BB_BUFFER: [BlackboxRecord; BLACKBOX_MAX_RECORDS] =
    [BlackboxRecord::zeroed(); BLACKBOX_MAX_RECORDS];

/// Sonraki yazma konumu [0, BLACKBOX_MAX_RECORDS)
static mut BB_WRITE_POS: u8 = 0;

/// Sonraki sıra numarası (u16, saturating add değil wrapping — ring buffer)
static mut BB_NEXT_SEQ: u16 = 0;

/// Boot'tan bu yana geçen tick — schedule() her çağrısında advance_tick() artırır
static mut BB_TICK: u32 = 0;

/// Boot dönemi — timestamp u32 wrap-around çözümü için (v1.0: daima 0)
static mut BB_BOOT_EPOCH: u16 = 0;

/// Tampondaki kayıt sayısı (max BLACKBOX_MAX_RECORDS)
static mut BB_COUNT: u8 = 0;

// ═══════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════

/// Boot başlangıcı — rust_main'de, zamanlayıcıdan önce çağrılır
/// v1.0 (QEMU): tamponu sıfırdan başlatır, KernelBoot kaydı yazar
/// v1.5+ (SRAM/FRAM): son geçerli seq'i tarar, kaldığı yerden devam eder
pub fn init() {
    unsafe {
        BB_WRITE_POS  = 0;
        BB_NEXT_SEQ   = 0;
        BB_TICK       = 0;
        BB_BOOT_EPOCH = 0; // v1.0: QEMU'da kalıcı bellek yok, epoch her boot=0
        BB_COUNT      = 0;
    }
    // KernelBoot kaydı: data[0..2] = boot_epoch (little-endian)
    let mut boot_data = [0u8; 46];
    boot_data[0] = 0u8; // BB_BOOT_EPOCH low byte
    boot_data[1] = 0u8; // BB_BOOT_EPOCH high byte
    log(BlackboxEvent::KernelBoot, 0xFF, &boot_data);
}

/// Tick sayacını ilerlet — schedule() her çağrısının başında çağrılır
#[inline]
pub fn advance_tick() {
    unsafe { BB_TICK = BB_TICK.wrapping_add(1); }
}

/// Olay kaydet — SADECE KERNEL çağırır (tek yazar garantisi)
///
/// event:   Olay türü
/// task_id: Tetikleyen task ID (0xFF = kernel dahili olay)
/// data:    En fazla 46 byte olay verisi (kısa girişler sıfır doldurulur)
pub fn log(event: BlackboxEvent, task_id: u8, data: &[u8]) {
    unsafe {
        let pos = BB_WRITE_POS as usize;

        let mut rec = BlackboxRecord::zeroed();
        rec.magic     = MAGIC;
        rec.version   = RECORD_VERSION;
        rec.seq       = BB_NEXT_SEQ;
        rec.timestamp = BB_TICK;
        rec.task_id   = task_id;
        rec.event     = event as u8;

        // Veri kopyala — en fazla 46 byte, geri kalanı sıfır
        let n = if data.len() < 46 { data.len() } else { 46 };
        let mut i = 0;
        while i < n {
            rec.data[i] = data[i];
            i += 1;
        }

        // CRC hesapla ve yaz — power-loss koruması için son adım
        rec.set_crc();
        BB_BUFFER[pos] = rec;

        // Konumu ilerlet — döngüsel sarma
        BB_WRITE_POS = if pos + 1 >= BLACKBOX_MAX_RECORDS {
            0
        } else {
            (pos + 1) as u8
        };

        BB_NEXT_SEQ = BB_NEXT_SEQ.wrapping_add(1);

        if (BB_COUNT as usize) < BLACKBOX_MAX_RECORDS {
            BB_COUNT += 1;
        }
    }
}

/// Kayıt oku — 0 = en eski, count()-1 = en yeni
/// Dönüş: Some(record) — CRC geçerli; None — index aşımı veya bozuk kayıt
pub fn read(index: usize) -> Option<BlackboxRecord> {
    unsafe {
        if index >= BB_COUNT as usize {
            return None;
        }
        let start = if (BB_COUNT as usize) < BLACKBOX_MAX_RECORDS {
            0usize
        } else {
            BB_WRITE_POS as usize // Tampon dolu: en eski = write_pos
        };
        let actual = (start + index) % BLACKBOX_MAX_RECORDS;
        let rec = BB_BUFFER[actual];
        if rec.is_valid() { Some(rec) } else { None }
    }
}

/// Tampondaki geçerli kayıt sayısı
pub fn count() -> usize {
    unsafe { BB_COUNT as usize }
}

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
}
