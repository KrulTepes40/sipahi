// Sipahi — HAL (Hardware Abstraction Layer)
// Sprint 6:  Device trait + IOPMP stub
// Sprint 13: Secure boot Ed25519 + Key provisioning
//
// Doktrin: static dispatch (dyn Trait YASAK — vtable overhead)
// Her device statik, compile-time bilinen tip.

pub mod device;
pub mod iopmp;
pub mod key;
pub mod secure_boot;
