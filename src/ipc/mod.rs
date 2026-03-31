// Katman 2: Sinir Sistemi — IPC & Transport
// 1,100 satır bütçe
// Sprint 8, 11'de implemente edilecek
//
// IPC CONTRACT (v10.0):
// 1. Size:        64B sabit
// 2. Ordering:    FIFO (SPSC)
// 3. Backpressure: Err(Full), bloklanma YOK
// 4. Integrity:   CRC32
// 5. Auth:        Capability token gerekli
// 6. Determinism: O(1) send/recv
// 7. Failure:     Explicit, retry YOK
//
// İMPLEMENTASYON NOTU (Sprint 8):
// head/tail → AtomicU16 olmalı (u16 DEĞİL)
// ISR task'ı kesebilir → atomic olmadan data race
// use core::sync::atomic::{AtomicU16, Ordering};
// Producer: store(Release), Consumer: load(Acquire)

// pub mod channel;   // Sprint 8
// pub mod message;   // Sprint 8
// pub mod blackbox;  // Sprint 11
