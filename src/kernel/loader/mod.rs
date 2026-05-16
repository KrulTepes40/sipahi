//! Native task loader — boot-time PMP region image setup.
//!
//! SNTM design v0.8 §4.6 + §4.7: kernel ELF parse ETMEZ, kernel relocation
//! çözmez. sntm-pack host tool ELF → per-section .bin üretir, kernel image
//! bunları `include_bytes!()` ile embed eder, boot'ta bounded_copy +
//! zero_fill ile PMP region'larına yerleştirir.
//!
//! U-26 Phase 4: task_hello (task_id=2) ilk native task.
//! FIX-A: NATIVE_TASK_BASE = 0x80600000 (kernel/wasm_arena sonrası).
//! FIX-D: load_region önce zero_fill, sonra bounded_copy (info-leak guard).

#![allow(dead_code)] // U-26 inkremental: G7 (embed) + G8 (load_task_hello) sonu aktif

pub mod bounds;
pub mod copy;
pub mod embed;
pub mod zero;

pub use bounds::is_safe_load_dst;
pub use copy::bounded_copy;
#[allow(unused_imports)] // U-26 G8 sonu kullanılır
pub use copy::BoundedCopyError;
pub use zero::zero_fill;

#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    ProfileNotFound,
    ProfileEmpty,
    TextOverflow,
    RodataOverflow,
    DataOverflow,
    TextNotSafe,
    RodataNotSafe,
    DataNotSafe,
}

/// U-26 G8 hedef — task_hello'yu boot'ta region'larına yükle.
/// SNTM design v0.8 §4.7 step 4: bounded copy + bss zero + FIX-D zero-fill first.
///
/// SAFETY: Boot context — MIE=0, single hart, scheduler henüz başlamadı.
/// Kernel M-mode + PMP unmatched access → tüm RAM erişebilir (RISC-V spec).
/// PMP_PROFILES[2] region adresleri build-time const (sntm-validate output).
#[cfg(not(kani))]
pub unsafe fn load_task_hello() -> Result<(), LoadError> {
    const TASK_HELLO_ID: u8 = 2;
    let profile = crate::kernel::pmp::profile::get_pmp_profile(TASK_HELLO_ID)
        .ok_or(LoadError::ProfileNotFound)?;
    if profile.region_count == 0 {
        return Err(LoadError::ProfileEmpty);
    }
    // task_hello layout (sipahi.toml task_id=2): region 0=text, 1=rodata,
    // 2=data, 3=stack. Sırası manifest'le BIRLEŞIK invariant.
    let regions = profile.active_regions();

    // Region 0: text
    // SAFETY: regions[0] kernel range dışı (manifest validator + FIX-A).
    unsafe {
        load_region(&regions[0], embed::TASK_HELLO_TEXT,
                    LoadError::TextOverflow, LoadError::TextNotSafe)?;
    }
    // Region 1: rodata
    if regions.len() > 1 {
        // SAFETY: regions[1] kernel range dışı.
        unsafe {
            load_region(&regions[1], embed::TASK_HELLO_RODATA,
                        LoadError::RodataOverflow, LoadError::RodataNotSafe)?;
        }
    }
    // Region 2: data (data + bss; FIX-D zero_fill ÖNCE region tail = bss)
    if regions.len() > 2 {
        // SAFETY: regions[2] kernel range dışı.
        unsafe {
            load_region(&regions[2], embed::TASK_HELLO_DATA,
                        LoadError::DataOverflow, LoadError::DataNotSafe)?;
        }
    }
    // Region 3 (stack): FIX-D explicit zero-fill (info-leak + deterministik
    // initial state). _start sp'yi resetler ama stack content garbage'tan
    // başlamasın (CWE-457 uninitialized read defense).
    if regions.len() > 3 {
        let stack_region = &regions[3];
        if is_safe_load_dst(stack_region.base, stack_region.size) {
            // SAFETY: stack region kernel range dışı (is_safe_load_dst), M-mode access.
            unsafe {
                let stack_ptr = stack_region.base as *mut u8;
                let stack_slice = core::slice::from_raw_parts_mut(
                    stack_ptr, stack_region.size,
                );
                zero_fill(stack_slice);
            }
        }
    }
    Ok(())
}

#[cfg(not(kani))]
unsafe fn load_region(
    region: &crate::kernel::pmp::profile::Region,
    src: &'static [u8],
    overflow_err: LoadError,
    unsafe_err: LoadError,
) -> Result<(), LoadError> {
    if !is_safe_load_dst(region.base, region.size) {
        return Err(unsafe_err);
    }
    if src.len() > region.size {
        return Err(overflow_err);
    }
    // FIX-D: Stage 1 — TÜM region'ı zero-fill (info-leak + determinism).
    // Eski RAM içeriği silinir; tail kısım (src.len()..region.size) zero kalır.
    // SAFETY: dst region kernel range dışı (is_safe_load_dst doğruladı),
    // M-mode kernel write (PMP unmatched = full M-mode access).
    unsafe {
        let dst_ptr = region.base as *mut u8;
        let dst_slice = core::slice::from_raw_parts_mut(dst_ptr, region.size);
        zero_fill(dst_slice);
    }
    // Stage 2: src bytes copy (zero tail kalır).
    // SAFETY: see above.
    unsafe {
        let dst_ptr = region.base as *mut u8;
        let dst_slice = core::slice::from_raw_parts_mut(dst_ptr, region.size);
        bounded_copy(src, dst_slice).map_err(|_| overflow_err)?;
    }
    Ok(())
}
