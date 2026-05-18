//! TaskCertificate generator — build-time forensics metadata bundle.
//!
//! SAFE-3 (sprint-u32) §17.4. Cert opaque blob — kernel parse YOK (Section 8
//! CR-9 doctrine: forensics metadata only, runtime enforcement riscv-bin-verify
//! build-time'a yansır).
//!
//! ABI: repr(C) fixed-size layout, padding manuel. Drift guard: ABI_VERSION
//! + CERT_SIZE compile-time invariant (Section 8 CR-8 cross-crate K8 doctrine).
//!
//! Section 8 CR-6: cert artifact'lar `target/native/*.cert.{bin,sig}` —
//! gitignored (source_commit içerir, commit edersek circular dep).

pub mod cert;
pub mod chain;
pub mod stackreport;

pub use cert::{Range64, TaskCertificate, ABI_VERSION, CERT_SIZE};
pub use stackreport::UNKNOWN_SENTINEL as STACK_UNKNOWN_SENTINEL;
