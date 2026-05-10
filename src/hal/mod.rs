//! Hardware Abstraction Layer: device trait, IOPMP, key store, secure boot.
// Sipahi — HAL (Hardware Abstraction Layer)
// Sprint 6:  Device trait + IOPMP stub
// Sprint 13: Secure boot Ed25519 + Key provisioning
//
// Doktrin: static dispatch (dyn Trait YASAK — vtable overhead)
// Her device statik, compile-time bilinen tip.

// U-22 GÖREV 5 [M9]: device.rs v2.0 HAL abstraction — feature-gated.
// v1.0'da hiç callsite yok (UART direkt arch::uart kullanır). v2.0 SPI/I2C/GPIO
// eklendiğinde aktif olur. Gate açık değilken hiç compile edilmez -> dead-code yok.
#[cfg(feature = "v2-hal")]
pub mod device;
pub mod iopmp;
pub mod key;
pub mod secure_boot;
