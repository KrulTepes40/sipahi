//! Sipahi SNTM task-side API library.
//!
//! Sprint U-23 (SNTM Phase 1): sipahi_api body — Error + Message + 5 syscall
//! wrapper. SYS_EXIT wrapper G5'te eklenir.
//!
//! Referans: `SIPAHI_SNTM_DESIGN.md` v0.8 §8 Sipahi API.
//!
//! Kullanım:
//! ```ignore
//! use sipahi_api::syscall;
//!
//! #[no_mangle]
//! pub extern "C" fn _start() -> ! {
//!     loop { syscall::yield_cpu(); }
//! }
//! ```

#![no_std]

// SAFE-2 (sprint-u31): manifest-driven typed IPC wrappers.
// Generated file — DO NOT edit; regenerate via `bash scripts/regen_safe_codegen.sh`.
pub mod channels;

/// Task-side hata tipleri — kernel `SyscallResult::to_raw()` + ek E_RATE_LIMITED
/// + E_INTERNAL sentinel'leriyle bit-eşit hizalı.
///
/// SAFE-3 (sprint-u32, Section 8 CR-1 + CR-13): bu hizalamayı U-21 GÖREV 4
/// [MP2] scaffold sentinel mapping kayıkken (InvalidArg / Permission /
/// InvalidSyscall yanlış raw value) → task yanlış error name görüyordu.
/// CR-13: RateLimited + Internal orphan değil — kernel DİSPATCH HÂLÂ EMİT
/// EDİYOR (dispatch.rs:60-61 E_RATE_LIMITED / E_INTERNAL). Geri eklendi.
///
/// | Raw value        | Kernel emit                       | sipahi_api `Error` |
/// |------------------|-----------------------------------|--------------------|
/// | 0                | SyscallResult::Ok                 | (None)             |
/// | usize::MAX       | SyscallResult::InvalidSyscall     | InvalidSyscall     |
/// | usize::MAX - 1   | SyscallResult::NoCapability       | NoCapability       |
/// | usize::MAX - 2   | SyscallResult::IpcFull            | IpcFull            |
/// | usize::MAX - 3   | SyscallResult::IpcEmpty           | IpcEmpty           |
/// | usize::MAX - 4   | SyscallResult::InvalidArg         | InvalidArg         |
/// | usize::MAX - 5   | SyscallResult::BufferFull         | BufferFull         |
/// | usize::MAX - 6   | const E_RATE_LIMITED (dispatch.rs)| RateLimited        |
/// | usize::MAX - 7   | const E_INTERNAL (dispatch.rs)    | Internal           |
///
/// Drift guard: `verify::verification::syscall_error_abi_alignment` Kani
/// harness K8 cross-crate — 8 raw değer kapsar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    InvalidSyscall = 0,
    NoCapability   = 1,
    IpcFull        = 2,
    IpcEmpty       = 3,
    InvalidArg     = 4,
    BufferFull     = 5,
    RateLimited    = 6,
    Internal       = 7,
}

impl Error {
    /// 0 (E_OK) Error değil → None. Aksi: kernel raw value → Error variant.
    /// Bilinmeyen raw değer (drift sinyali) → `Some(Error::Internal)`
    /// (defansif default; gerçek ABI drift Kani harness ile yakalanır).
    #[inline]
    pub fn from_kernel(ret: usize) -> Option<Self> {
        match ret {
            0 => None,
            v if v == usize::MAX     => Some(Error::InvalidSyscall),
            v if v == usize::MAX - 1 => Some(Error::NoCapability),
            v if v == usize::MAX - 2 => Some(Error::IpcFull),
            v if v == usize::MAX - 3 => Some(Error::IpcEmpty),
            v if v == usize::MAX - 4 => Some(Error::InvalidArg),
            v if v == usize::MAX - 5 => Some(Error::BufferFull),
            v if v == usize::MAX - 6 => Some(Error::RateLimited),
            v if v == usize::MAX - 7 => Some(Error::Internal),
            _ => Some(Error::Internal),
        }
    }
}

/// IPC mesaj tipleri.
pub mod ipc {
    /// IPC mesaj — kernel `crate::ipc::IpcMessage` ile binary uyumlu.
    /// 64 byte (config.rs::IPC_MSG_SIZE). repr(C) ABI stability.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Message {
        pub data: [u8; 64],
    }

    impl Message {
        pub const SIZE: usize = 64;

        #[inline]
        pub const fn empty() -> Self {
            Self { data: [0u8; 64] }
        }
    }
}

/// CRC primitives (v1.7 typed IPC ile genişletilecek).
pub mod crc {
    // v1.7'de typed IPC ile birlikte kernel'den re-export gelecek.
}

/// Syscall wrappers — ABI: a7 = id, a0-a3 = args, return = a0.
pub mod syscall {
    use crate::{Error, ipc::Message};

    const SYS_CAP_INVOKE: usize = 0;
    const SYS_IPC_SEND:   usize = 1;
    const SYS_IPC_RECV:   usize = 2;
    const SYS_YIELD:      usize = 3;
    const SYS_TASK_INFO:  usize = 4;
    const SYS_EXIT:       usize = 5;  // U-23 SNTM Phase 1

    /// Capability invoke — legacy MAC token + resource + action kontrol.
    ///
    /// `token` MUST have bit 7 clear (token id < 0x80). Bit 7 reserved as
    /// SAFE-2 path discriminant. SAFE-3 (Section 8 CR-5): wrapper bit-7'yi
    /// **enforce eder** — `token >= 0x80` çağrılırsa `Err(InvalidArg)`
    /// döner, kernel-side local-cap path'e silent fallback YOK. Static
    /// local capability için `local_cap_invoke` kullan.
    #[inline]
    pub fn cap_invoke(token: u8, resource: u16, action: u8) -> Result<(), Error> {
        // SAFE-3 CR-5 guard: bit 7 set = local-cap path discriminant; bu
        // wrapper'da çağrı caller intent drift'idir (MAC token path
        // beklenirken local-cap path'e düşürür). Audit clarity için fail-fast.
        if token & 0x80 != 0 {
            return Err(Error::InvalidArg);
        }
        // SAFETY: ecall trap to M-mode, kernel dispatch handles registers.
        let ret = unsafe {
            ecall3(SYS_CAP_INVOKE, token as usize, resource as usize, action as usize)
        };
        match Error::from_kernel(ret) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// SAFE-2 (sprint-u31, CR-2): static local capability invoke (~5c).
    ///
    /// Resource grants are build-time decidable from manifest
    /// `[[task.local_cap]]`. No MAC, no nonce, no cache — kernel-side
    /// `LOCAL_CAP_TABLE` array lookup.
    ///
    /// ABI: `sys_cap_invoke` cap bit 7 = 1 selects this path; bit 7 = 0
    /// routes to legacy `cap_invoke` (MAC token).
    #[inline]
    pub fn local_cap_invoke(resource: u8, action: u8) -> Result<(), Error> {
        // cap bit 7 = 1 → local path discriminant. Lower 7 bits MUST be 0
        // (reserved for future flags); kernel returns InvalidArg otherwise.
        const LOCAL_PATH_FLAG: usize = 0x80;
        // SAFETY: ecall trap; kernel-side bounds & action validation.
        let ret = unsafe {
            ecall3(SYS_CAP_INVOKE, LOCAL_PATH_FLAG, resource as usize, action as usize)
        };
        match Error::from_kernel(ret) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// IPC send — channel'a mesaj gönder.
    #[inline]
    pub fn ipc_send(channel: u8, msg: &Message) -> Result<(), Error> {
        // SAFETY: ecall trap; msg pointer is_valid_user_ptr ile kernel-side validate edilir.
        let ret = unsafe {
            ecall2(SYS_IPC_SEND, channel as usize, msg as *const _ as usize)
        };
        match Error::from_kernel(ret) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// IPC recv — channel'dan mesaj al. Ok(false) = boş kanal (non-blocking).
    #[inline]
    pub fn ipc_recv(channel: u8, msg_out: &mut Message) -> Result<bool, Error> {
        // SAFETY: ecall trap; msg_out pointer kernel-side validate edilir.
        let ret = unsafe {
            ecall2(SYS_IPC_RECV, channel as usize, msg_out as *mut _ as usize)
        };
        match Error::from_kernel(ret) {
            None => Ok(true),
            Some(Error::IpcEmpty) => Ok(false),
            Some(e) => Err(e),
        }
    }

    /// Yield CPU — scheduler'a kontrolü ver. Hata dönmez (sıfır args/return).
    #[inline]
    pub fn yield_cpu() {
        // SAFETY: ecall trap; SYS_YIELD handler tablo dispatch, no args needed.
        unsafe { ecall0(SYS_YIELD) };
    }

    /// Task bilgisi (task_id, state, priority bitfield packed).
    #[inline]
    pub fn task_info(task_id: u8) -> Result<u32, Error> {
        // SAFETY: ecall trap; arg0 = task_id, return = packed u32 info.
        let ret = unsafe { ecall1(SYS_TASK_INFO, task_id as usize) };
        match Error::from_kernel(ret) {
            None => Ok(ret as u32),
            Some(e) => Err(e),
        }
    }

    /// Task voluntary termination — divergent (-> !), task'a geri dönüş YOK.
    /// Kernel sys_exit handler: isolate_task + schedule_yield. Task'ı bir
    /// daha scheduler dispatch etmez (TaskState::Isolated).
    #[inline]
    pub fn exit(code: u8) -> ! {
        // SAFETY: ecall trap; kernel handler isolate eder, dönmez.
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") SYS_EXIT,
                in("a0") code as usize,
                options(nostack, noreturn),
            );
        }
    }

    // ─── ecall trampolines ──────────────────────────────────────────
    // SAFETY contract: caller her zaman kernel'ın beklediği ABI'ye uygun
    // argümanlar geçirir (kernel-side is_valid_user_ptr ek validation yapar).
    // a7 = syscall id, a0-a3 = args, return value = a0. Trap to M-mode.

    #[inline(always)]
    unsafe fn ecall0(id: usize) -> usize {
        let ret: usize;
        core::arch::asm!(
            "ecall",
            in("a7") id,
            lateout("a0") ret,
            options(nostack, preserves_flags),
        );
        ret
    }

    #[inline(always)]
    unsafe fn ecall1(id: usize, a0: usize) -> usize {
        let ret: usize;
        core::arch::asm!(
            "ecall",
            in("a7") id,
            inlateout("a0") a0 => ret,
            options(nostack, preserves_flags),
        );
        ret
    }

    #[inline(always)]
    unsafe fn ecall2(id: usize, a0: usize, a1: usize) -> usize {
        let ret: usize;
        core::arch::asm!(
            "ecall",
            in("a7") id,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            options(nostack, preserves_flags),
        );
        ret
    }

    #[inline(always)]
    unsafe fn ecall3(id: usize, a0: usize, a1: usize, a2: usize) -> usize {
        let ret: usize;
        core::arch::asm!(
            "ecall",
            in("a7") id,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            options(nostack, preserves_flags),
        );
        ret
    }
}
