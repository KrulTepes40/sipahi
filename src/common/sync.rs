//! Single-hart exclusive access wrapper — zero-cost, no synchronization.
#![allow(dead_code)]

use core::cell::UnsafeCell;

#[cfg(feature = "multi-hart")]
compile_error!("SingleHartCell multi-hart ile uyumsuz — Mutex<T> kullanın");

/// Single-hart exclusive access wrapper — zero-cost.
///
/// SAFETY CONTRACT:
///   Bu tip SADECE single-hart sistemlerde güvenlidir.
///   Multi-hart desteği eklenirken Mutex<T> ile değiştir.
pub struct SingleHartCell<T> {
    inner: UnsafeCell<T>,
}

// SAFETY: Single-hart — no concurrent access possible.
unsafe impl<T> Sync for SingleHartCell<T> {}

impl<T> SingleHartCell<T> {
    pub const fn new(val: T) -> Self {
        Self { inner: UnsafeCell::new(val) }
    }

    /// Mutable reference al
    /// SAFETY: Caller single-hart + no reentrant interrupt guarantee etmeli.
    #[inline(always)]
    #[allow(clippy::mut_from_ref)] // UnsafeCell interior mutability — by design.
    pub unsafe fn get_mut(&self) -> &mut T {
        &mut *self.inner.get()
    }

    /// Shared reference al
    /// SAFETY: Caller single-hart guarantee etmeli.
    #[inline(always)]
    pub unsafe fn get(&self) -> &T {
        &*self.inner.get()
    }

    /// Raw pointer al — volatile erişim için
    #[inline(always)]
    pub fn as_ptr(&self) -> *mut T {
        self.inner.get()
    }
}
