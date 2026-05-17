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
#[cfg(not(kani))]
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

/// U-27 SNTM Phase 5: native task segment image descriptor.
/// Region 0 = text (required), 1 = rodata (optional), 2 = data (optional);
/// region 3+ (stack) loader tarafından zero-fill edilir (segment yok).
#[cfg(not(kani))]
pub struct NativeTaskSegments {
    pub text:   &'static [u8],
    pub rodata: Option<&'static [u8]>,
    pub data:   Option<&'static [u8]>,
}

/// U-27 generic native task loader. task_hello + task_world ortak path.
///
/// VERIFIES: SNTM-R9 (bounded_copy + zero_fill atomicity + kernel-disjoint dst),
///           SNTM-R10 (region order text→rodata→data→stack invariant),
///           SNTM-R14 prereq (multi-task ardışık yükleme PMP_PROFILES'i bozmaz).
/// CALLS:    load_region (FIX-D zero-fill öncülüklü stage), is_safe_load_dst.
/// FAILS-IF: PMP_PROFILES[task_id] EMPTY, region 0 kernel range içinde,
///           src.len() > region.size partial copy, stack region eksik
///           zero-fill (CWE-457 leak), ya da region order manifest'ten sapma.
///
/// SAFETY: Boot context — MIE=0, single hart, scheduler henüz başlamadı.
/// Kernel M-mode + PMP unmatched access → tüm RAM erişebilir (RISC-V spec).
/// PMP_PROFILES[task_id] region adresleri build-time const (sntm-validate output).
#[cfg(not(kani))]
pub unsafe fn load_native_task(
    task_id: u8,
    segments: &NativeTaskSegments,
) -> Result<(), LoadError> {
    let profile = crate::kernel::pmp::profile::get_pmp_profile(task_id)
        .ok_or(LoadError::ProfileNotFound)?;
    if profile.region_count == 0 {
        return Err(LoadError::ProfileEmpty);
    }
    // Region sırası manifest invariant: 0=text, 1=rodata, 2=data, 3=stack.
    let regions = profile.active_regions();

    // Region 0: text (zorunlu)
    // SAFETY: regions[0] kernel range dışı (manifest validator + FIX-A).
    unsafe {
        load_region(&regions[0], segments.text,
                    LoadError::TextOverflow, LoadError::TextNotSafe)?;
    }
    // Region 1: rodata (opsiyonel)
    if regions.len() > 1 {
        if let Some(rodata) = segments.rodata {
            // SAFETY: regions[1] kernel range dışı.
            unsafe {
                load_region(&regions[1], rodata,
                            LoadError::RodataOverflow, LoadError::RodataNotSafe)?;
            }
        } else {
            // rodata segment YOK ama region var → FIX-D zero-fill only
            // (deterministik initial state, info-leak guard).
            unsafe {
                load_region_zero_only(&regions[1], LoadError::RodataNotSafe)?;
            }
        }
    }
    // Region 2: data (opsiyonel — data + bss; FIX-D zero_fill ÖNCE)
    if regions.len() > 2 {
        if let Some(data) = segments.data {
            // SAFETY: regions[2] kernel range dışı.
            unsafe {
                load_region(&regions[2], data,
                            LoadError::DataOverflow, LoadError::DataNotSafe)?;
            }
        } else {
            unsafe {
                load_region_zero_only(&regions[2], LoadError::DataNotSafe)?;
            }
        }
    }
    // Region 3 (stack): FIX-D explicit zero-fill — segment YOK, sadece zero
    // (CWE-457 uninitialized read defense; _start sp'yi resetler ama
    // stack content garbage'tan başlamasın).
    if regions.len() > 3 {
        let stack_region = &regions[3];
        if is_safe_load_dst(stack_region.base, stack_region.size) {
            // SAFETY: stack region kernel range dışı, M-mode access.
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

/// U-26 backward-compat wrapper. load_native_task(2, TASK_HELLO_SEGMENTS).
#[cfg(not(kani))]
pub unsafe fn load_task_hello() -> Result<(), LoadError> {
    let segments = NativeTaskSegments {
        text:   embed::TASK_HELLO_TEXT,
        rodata: Some(embed::TASK_HELLO_RODATA),
        data:   Some(embed::TASK_HELLO_DATA),
    };
    // SAFETY: see load_native_task.
    unsafe { load_native_task(2, &segments) }
}

/// U-27 task_world loader. load_native_task(3, TASK_WORLD_SEGMENTS).
#[cfg(not(kani))]
pub unsafe fn load_task_world() -> Result<(), LoadError> {
    let segments = NativeTaskSegments {
        text:   embed::TASK_WORLD_TEXT,
        rodata: Some(embed::TASK_WORLD_RODATA),
        data:   Some(embed::TASK_WORLD_DATA),
    };
    // SAFETY: see load_native_task.
    unsafe { load_native_task(3, &segments) }
}

/// U-27: segment-less region (rodata/data placeholder ama embed YOK) için
/// FIX-D zero-fill only. info-leak guard, CWE-457 uninitialized read defense.
#[cfg(not(kani))]
unsafe fn load_region_zero_only(
    region: &crate::kernel::pmp::profile::Region,
    unsafe_err: LoadError,
) -> Result<(), LoadError> {
    if !is_safe_load_dst(region.base, region.size) {
        return Err(unsafe_err);
    }
    // SAFETY: dst region kernel range dışı (is_safe_load_dst), M-mode write.
    unsafe {
        let dst_ptr = region.base as *mut u8;
        let dst_slice = core::slice::from_raw_parts_mut(dst_ptr, region.size);
        zero_fill(dst_slice);
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
