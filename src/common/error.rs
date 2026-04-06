// Sipahi — Hata Tipleri
// Safety-critical: her hata açık, sessiz başarısızlık YOK

/// Sipahi kernel hata tipleri
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SipahiError {
    /// Capability token doğrulaması başarısız
    CapabilityDenied,
    /// IPC buffer dolu — mesaj YAZILMADI (kayıp yok)
    BufferFull,
    /// Geçersiz syscall numarası
    InvalidSyscall,
    /// Task budget tükendi
    BudgetExhausted,
    /// CRC32 doğrulaması başarısız (IPC veya blackbox)
    IntegrityError,
    /// WASM fuel tükendi
    FuelExhausted,
    /// WASM modül imza doğrulaması başarısız
    ModuleRejected,
    /// PMP integrity check başarısız — KRİTİK
    PmpViolation,
    /// Deadline miss
    DeadlineMiss,
    /// Watchdog timeout — 3 kademeli escalation
    WatchdogTimeout,
    /// Aygıt hazır değil veya başlatılmamış
    DeviceNotReady,
    /// Geçersiz indeks veya parametre
    InvalidParameter,
    /// Token nonce tekrarı (replay attack)
    ReplayDetected,
}
