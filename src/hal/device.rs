//! HAL device trait — static-dispatch hardware abstraction (no vtable).
#![allow(dead_code)] // HAL API — v2.0 SPI/I2C/GPIO will use this trait.
// Sipahi — Device Access Trait (Sprint 6)
//
// Tüm donanım aygıtları bu trait'i implemente eder.
// Static dispatch: dyn Trait YASAK — vtable overhead.
// Generic veya concrete tip kullan.
//
// v1.0: Sadece UART implemente eder.
// v2.0: SPI, I2C, GPIO eklenebilir.

use crate::common::error::SipahiError;

/// Donanım aygıt erişim trait'i
///
/// WCET garantisi: her operasyon bounded, blocking YOK.
/// Hata durumunda SipahiError döner, panic OLMAZ.
pub trait DeviceAccess {
    /// Aygıtı başlat (bir kez, boot sırasında)
    fn init(&mut self) -> Result<(), SipahiError>;

    /// Tek byte oku (non-blocking)
    /// Veri yoksa → Err(DeviceNotReady)
    fn read_byte(&self) -> Result<u8, SipahiError>;

    /// Tek byte yaz (non-blocking)
    /// Buffer doluysa → Err(DeviceNotReady)
    fn write_byte(&self, byte: u8) -> Result<(), SipahiError>;

    /// Aygıt hazır mı? (polling, blocking DEĞİL)
    fn is_ready(&self) -> bool;
}

/// UART aygıtı — DeviceAccess implementasyonu
///
/// QEMU virt: ns16550a @ 0x10000000
/// Sprint 1'deki uart.rs fonksiyonlarını trait üzerinden sunar.
/// Mevcut uart.rs dokunulmadı — bu wrapper, eski API çalışmaya devam eder.
pub struct UartDevice {
    base_addr: usize,
    initialized: bool,
}

impl UartDevice {
    /// Yeni UART device oluştur (const fn — statik init için)
    pub const fn new(base_addr: usize) -> Self {
        UartDevice {
            base_addr,
            initialized: false,
        }
    }
}

#[cfg(not(kani))]
impl DeviceAccess for UartDevice {
    fn init(&mut self) -> Result<(), SipahiError> {
        // ns16550a basit — init gerekmez, donanım hazır
        self.initialized = true;
        Ok(())
    }

    fn read_byte(&self) -> Result<u8, SipahiError> {
        if !self.initialized {
            return Err(SipahiError::DeviceNotReady);
        }
        // LSR (Line Status Register) bit 0: Data Ready
        // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
        let lsr = unsafe {
            core::ptr::read_volatile((self.base_addr + 5) as *const u8)
        };
        if lsr & 1 == 0 {
            return Err(SipahiError::DeviceNotReady);
        }
        // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
        let byte = unsafe {
            core::ptr::read_volatile(self.base_addr as *const u8)
        };
        Ok(byte)
    }

    fn write_byte(&self, byte: u8) -> Result<(), SipahiError> {
        if !self.initialized {
            return Err(SipahiError::DeviceNotReady);
        }
        // LSR bit 5: Transmit Holding Register Empty
        // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
        let lsr = unsafe {
            core::ptr::read_volatile((self.base_addr + 5) as *const u8)
        };
        if lsr & 0x20 == 0 {
            return Err(SipahiError::DeviceNotReady);
        }
        // SAFETY: MMIO register access at hardware-defined address.
        unsafe {
            core::ptr::write_volatile(self.base_addr as *mut u8, byte);
        }
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.initialized
    }
}
