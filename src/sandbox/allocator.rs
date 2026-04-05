// Sipahi — WASM Bump Allocator (Sprint 12)
// Doküman §WASM: "static WASM_HEAP: [u8; 65536] = [0; 65536]; // 64KB sabit"
//
// Kural: SADECE WASM sandbox kullanır — kernel kodu asla alloc KULLANMAZ
// Kural: dealloc = no-op (bump allocator, tek tek free yok)
// Kural: epoch_reset() → offset sıfırla (modül değiştiğinde çağrılır)
// Kural: OOM → null dön → alloc_error_handler → wfi loop (panic yok)
//
// Kani Proof 58: offset asla WASM_HEAP_SIZE'ı aşmaz
// Kani Proof 59: epoch_reset() sonrası offset == 0

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::common::config::WASM_HEAP_SIZE;

// ═══════════════════════════════════════════════════════
// Statik alan — 64KB sabit arena
// ═══════════════════════════════════════════════════════

/// WASM bellek arenası — 64KB, BSS'te sıfır başlatımlı
static mut ARENA: [u8; WASM_HEAP_SIZE] = [0u8; WASM_HEAP_SIZE];

/// Bir sonraki serbest baytın ofseti
static ARENA_OFFSET: AtomicUsize = AtomicUsize::new(0);

// ═══════════════════════════════════════════════════════
// BumpAllocator — GlobalAlloc impl
// ═══════════════════════════════════════════════════════

/// Sıfır boyutlu tip — GlobalAlloc impl için yeterli
pub struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size  = layout.size();
        let align = layout.align();

        // Mevcut ofseti oku
        let old = ARENA_OFFSET.load(Ordering::Relaxed);

        // Hizalamayı yukarı yuvarlat: aligned = ceil(old / align) * align
        let aligned = old.wrapping_add(align - 1) & !(align - 1);

        // Yeni son ofset
        let new_end = match aligned.checked_add(size) {
            Some(v) => v,
            None    => return core::ptr::null_mut(), // aritmetik taşma → OOM
        };

        // Arena sınır kontrolü
        if new_end > WASM_HEAP_SIZE {
            return core::ptr::null_mut(); // OOM → alloc_error_handler devreye girer
        }

        // Atomik güncelleme (tek hart, Relaxed yeterli)
        ARENA_OFFSET.store(new_end, Ordering::Relaxed);

        // Hizalanmış blok başlangıcını dön
        // &raw mut: referans oluşturmadan ham işaretçi (static_mut_refs uyarısını önler)
        (&raw mut ARENA).cast::<u8>().add(aligned)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Kasıtlı no-op — bump allocator bireysel free yapmaz.
        // Bellek yönetimi epoch_reset() ile toplu sıfırlama yoluyla yapılır.
    }
}

// ═══════════════════════════════════════════════════════
// Epoch Reset — modül değiştiğinde çağrılır
// ═══════════════════════════════════════════════════════

/// Arena ofseti sıfırla — tüm önceki allocations geçersiz sayılır.
///
/// SORUN 6 çözümü: Wasmi Vec/Box drop edince bellek geri gelmez (bump allocator).
/// Epoch reset, hot-swap sırasında wasmi instance tamamen drop edildikten
/// SONRA çağrılmalıdır. Bu sıralama çağıran tarafın sorumluluğundadır.
#[inline]
pub fn epoch_reset() {
    ARENA_OFFSET.store(0, Ordering::Release);
}

/// Şu anki ofset — test ve izleme için
#[inline]
pub fn current_offset() -> usize {
    ARENA_OFFSET.load(Ordering::Relaxed)
}
