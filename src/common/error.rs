//! Kernel error types — every failure is explicit, no silent drops.
// U-19 GÖREV 3: blanket #![allow(dead_code)] kaldırıldı — tekil işaretlenir
// Sipahi — Hata Tipleri
// Safety-critical: her hata açık, sessiz başarısızlık YOK

/// Sipahi kernel hata tipleri
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[allow(dead_code)] // Public error enum — v1.5 syscall API surface, runtime'da BufferFull aktif
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
    /// Geçersiz kullanıcı pointer'ı (kernel belleğine erişim girişimi)
    InvalidPointer,
}

impl SipahiError {
    /// Error -> human-readable string (Kani harness + production trace UART için)
    #[allow(dead_code)] // Kani Proof 85 (sipahi_error_as_str_never_empty) çağırır
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CapabilityDenied => "capability denied",
            Self::BufferFull       => "buffer full",
            Self::InvalidSyscall   => "invalid syscall",
            Self::BudgetExhausted  => "budget exhausted",
            Self::IntegrityError   => "integrity error",
            Self::FuelExhausted    => "fuel exhausted",
            Self::ModuleRejected   => "module rejected",
            Self::PmpViolation     => "PMP violation",
            Self::DeadlineMiss     => "deadline miss",
            Self::WatchdogTimeout  => "watchdog timeout",
            Self::DeviceNotReady   => "device not ready",
            Self::InvalidParameter => "invalid parameter",
            Self::ReplayDetected   => "replay detected",
            Self::InvalidPointer   => "invalid pointer",
        }
    }
}
