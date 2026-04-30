//! Common modules: config, types, error, crypto, formatting.
// Genel: Config + Types + Error + Crypto
// 300 satır bütçe (crypto trait'ler dahil)
// Sprint 0-1'de temel yapı, Sprint 9'da crypto implemente

pub mod config;
pub mod types;
pub mod error;
pub mod crypto;
pub mod fmt;
pub mod sync;
pub mod diagnostic;

// U-19 GÖREV 4: Terminal halt helper — duplikasyon engelleme.
// 12+ yerde aynı pattern: uart::println("[FATAL] ..."); loop { wfi }.
// Bu helper tek noktada toplar. trap.S `.park_nested` istisna (Rust çağrılamaz).

/// Terminal halt — UART'a sebep yazıp wfi loop'a girer. Asla dönmez.
/// UART mesajı gate'lenmez (terminal event, her build'de görünmeli).
#[cfg(not(kani))]
#[inline(never)]
pub(crate) fn halt_system(reason: &str) -> ! {
    crate::arch::uart::println(reason);
    // SAFETY: Terminal halt — recovery beklenmiyor. WFI hart'ı interrupt'a
    // kadar durdurur; loop interrupt sonrası tekrar wfi'ya döner.
    loop { unsafe { core::arch::asm!("wfi"); } }
}
