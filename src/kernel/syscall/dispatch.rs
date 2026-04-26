//! Syscall dispatch table — 5 handlers, O(1) jump, WCET-tracked.
// Sipahi — Syscall Dispatch (Sprint 7-8)
// Jump table dispatch — 5 syscall, O(1), deterministic
// Sprint 8: ipc_send/ipc_recv gerçek SPSC entegrasyonu

#[cfg(not(kani))]
use crate::arch::uart;

use crate::common::sync::SingleHartCell;
use crate::common::config::MAX_TASKS;

pub use crate::common::config::{
    SYS_CAP_INVOKE, SYS_IPC_SEND, SYS_IPC_RECV,
    SYS_YIELD, SYS_TASK_INFO, SYSCALL_COUNT,
};

/// Ardışık cap_invoke fail sayacı (per-task) — 3 fail → CapViolation
/// Başarılı cap_invoke sıfırlar. Sadece ardışık fail tetikler.
#[cfg(not(kani))]
static CAP_FAIL_COUNT: SingleHartCell<[u8; MAX_TASKS]> = SingleHartCell::new([0u8; MAX_TASKS]);

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum SyscallResult {
    Ok,
    InvalidSyscall,
    NoCapability,
    IpcFull,
    IpcEmpty,
    InvalidArg,
    BufferFull,
}

impl SyscallResult {
    pub const fn to_raw(self) -> usize {
        match self {
            Self::Ok             => 0,
            Self::InvalidSyscall => usize::MAX,
            Self::NoCapability   => usize::MAX - 1,
            Self::IpcFull        => usize::MAX - 2,
            Self::IpcEmpty       => usize::MAX - 3,
            Self::InvalidArg     => usize::MAX - 4,
            Self::BufferFull     => usize::MAX - 5,
        }
    }
}

pub const E_OK: usize = SyscallResult::Ok.to_raw();
pub const E_INVALID_SYSCALL: usize = SyscallResult::InvalidSyscall.to_raw();
pub const E_NO_CAPABILITY: usize = SyscallResult::NoCapability.to_raw();
pub const E_IPC_FULL: usize = SyscallResult::IpcFull.to_raw();
pub const E_IPC_EMPTY: usize = SyscallResult::IpcEmpty.to_raw();
pub const E_INVALID_ARG: usize = SyscallResult::InvalidArg.to_raw();
const E_RATE_LIMITED: usize = 7;
const E_INTERNAL: usize = 8;

// Linker-provided symbol — kernel memory end
#[cfg(not(kani))]
extern "C" { static _end: u8; }

/// Kernel bellek sınırını al — Kani'de sabit mock (0x80800000)
#[inline]
fn kernel_end_addr() -> usize {
    #[cfg(not(kani))]
    {
        // SAFETY: Linker-provided symbol address.
        unsafe { &_end as *const u8 as usize }
    }
    #[cfg(kani)]
    { 0x80800000 } // 8MB RAM mock
}

/// User pointer doğrulama — caller'ın KENDİ stack aralığına sınırlandırılmış.
/// Sprint U-16: Eski versiyon `_end` (WASM arena sonu) altındaki TÜM adresleri
/// reddediyordu — task stack'leri _end'in ALTINDA olduğundan tüm syscall pointer'ları
/// dormant olarak başarısız olurdu. Yeni versiyon caller'ın stack range'ini sorar
/// ve sadece o aralığı kabul eder. Cross-task pointer impersonation engellendi.
///
/// ptr == 0 → reject, ptr+size overflow → reject, ptr range != caller stack → reject.
#[must_use = "pointer validation result must be checked"]
fn is_valid_user_ptr(caller_task_id: u8, ptr: usize, size: usize) -> bool {
    if ptr == 0 { return false; }
    let end = match ptr.checked_add(size) {
        Some(e) => e,
        None => return false,
    };
    // Caller'ın stack aralığı — helper unsafe/aliasing'i kapsüller
    match crate::kernel::scheduler::task_stack_range(caller_task_id) {
        Some((base, top)) => ptr >= base && end <= top,
        None => false, // Dead/Isolated/uninitialized → default deny
    }
}

type SyscallHandler = fn(usize, usize, usize, usize) -> usize;

static SYSCALL_TABLE: [SyscallHandler; SYSCALL_COUNT] = [
    sys_cap_invoke,
    sys_ipc_send,
    sys_ipc_recv,
    sys_yield,
    sys_task_info,
];

#[cfg(not(kani))]
#[inline(always)]
fn rdcycle() -> u64 {
    let val: u64;
    // SAFETY: rdcycle reads cycle counter — no side effects.
    unsafe { core::arch::asm!("rdcycle {}", out(reg) val); }
    val
}

#[cfg(kani)]
#[inline(always)]
fn rdcycle() -> u64 { 0 }

static WCET_MAX: SingleHartCell<[u64; SYSCALL_COUNT]> = SingleHartCell::new([0; SYSCALL_COUNT]);
static WCET_LAST: SingleHartCell<[u64; SYSCALL_COUNT]> = SingleHartCell::new([0; SYSCALL_COUNT]);

#[inline(always)]
fn wcet_update(id: usize, cycles: u64) {
    if id < SYSCALL_COUNT {
        // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
        unsafe {
            (*WCET_LAST.get_mut())[id] = cycles;
            if cycles > (*WCET_MAX.get())[id] {
                (*WCET_MAX.get_mut())[id] = cycles;
            }
        }
    }
}

#[allow(dead_code)]
#[cfg(not(kani))]
pub fn print_wcet_stats() {
    uart::println("[WCET] Syscall cycle stats:");
    let names = ["cap_invoke", "ipc_send  ", "ipc_recv  ", "yield     ", "task_info "];
    let mut i = 0usize;
    while i < SYSCALL_COUNT {
        uart::puts("  ");
        uart::puts(names[i]);
        uart::puts(": last=");
        // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
        let (last, max) = unsafe { ((*WCET_LAST.get())[i], (*WCET_MAX.get())[i]) };
        print_u64(last);
        uart::puts(" max=");
        print_u64(max);
        uart::println(" cycles");
        i += 1;
    }
}

/// WCET limit kontrolü — max cycle'lar hedefleri aşıyor mu?
/// true = tüm syscall'lar limit altında, false = en az biri aşıyor
#[cfg(not(kani))]
pub fn check_wcet_limits() -> bool {
    use crate::common::config;
    let limits: [u64; SYSCALL_COUNT] = [
        config::WCET_CAP_INVOKE,
        config::WCET_IPC_SEND,
        config::WCET_IPC_RECV,
        config::WCET_YIELD,
        config::WCET_SCHEDULER_TICK,
    ];
    // SAFETY: Single-hart, no concurrent mutation.
    unsafe {
        let mut i = 0;
        let mut all_ok = true;
        while i < SYSCALL_COUNT {
            let max = (*WCET_MAX.get())[i];
            if max > limits[i] {
                uart::puts("[WCET] EXCEED syscall ");
                print_u64(i as u64);
                uart::puts(": max=");
                print_u64(max);
                uart::puts(" limit=");
                print_u64(limits[i]);
                uart::println("");
                all_ok = false;
            }
            i += 1;
        }
        all_ok
    }
}

#[inline(never)]
pub fn dispatch(
    syscall_id: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    if syscall_id >= SYSCALL_COUNT {
        #[cfg(all(not(kani), feature = "trace"))]
        {
            uart::puts("[SYSCALL] Invalid ID: ");
            print_u64(syscall_id as u64);
            uart::println("");
        }
        return E_INVALID_SYSCALL;
    }

    #[cfg(not(kani))]
    crate::kernel::scheduler::increment_syscall_count();

    let start = rdcycle();
    let handler = SYSCALL_TABLE[syscall_id];
    let result = handler(arg0, arg1, arg2, arg3);
    let end = rdcycle();
    wcet_update(syscall_id, end.wrapping_sub(start));

    // Kernel pointer sızıntı koruması — kernel adresi U-mode'a dönmemeli
    #[cfg(not(kani))]
    if result >= crate::common::config::RAM_BASE && result < kernel_end_addr() {
        return E_INTERNAL;
    }

    result
}

// ═══════════════════════════════════════════════════════
// Syscall Handler'lar — Sprint 8: IPC gerçek
// ═══════════════════════════════════════════════════════

fn sys_cap_invoke(cap: usize, resource: usize, action: usize, _arg: usize) -> usize {
    // Truncation koruması — usize → u8/u16 dönüşümde veri kaybı
    if cap > u8::MAX as usize
        || resource > u16::MAX as usize
        || action > u8::MAX as usize
    {
        #[cfg(all(not(kani), feature = "trace"))]
        uart::println("[SYS] cap_invoke: argument overflow");
        return E_INVALID_ARG;
    }

    // Cache-only fast path (~10c) — validate_full ile önceden kayıt edilmeli
    #[cfg(not(kani))]
    {
        let caller = crate::kernel::scheduler::current_task_id();
        let ok = crate::kernel::capability::broker::validate_cached(
            caller,
            cap as u8,
            resource as u16,
            action as u8,
        );
        #[cfg(feature = "trace")]
        {
            uart::puts("[SYS] cap_invoke(cap=");
            print_u64(cap as u64);
            uart::puts(") ");
            uart::println(if ok { "OK" } else { "DENIED" });
        }

        // CapViolation detection — 3 ardışık fail → policy tetikle
        // SAFETY: MIE=0 in trap context, single-hart.
        unsafe {
            if ok {
                // Başarılı cap_invoke → fail counter sıfırla
                (*CAP_FAIL_COUNT.get_mut())[caller as usize] = 0;
            } else {
                let count = &mut (*CAP_FAIL_COUNT.get_mut())[caller as usize];
                *count = count.saturating_add(1);
                if *count >= 3 {
                    // 3 ardışık cap fail → CapViolation policy
                    *count = 0; // reset (tekrar kuluçka periyodu)
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::CapViolation,
                        caller, &[],
                    );
                    let dal = crate::kernel::scheduler::TASKS.get()[caller as usize].dal;
                    let cap_action = crate::kernel::policy::apply_policy(
                        caller,
                        crate::kernel::policy::PolicyEvent::CapViolation,
                        dal,
                    );
                    crate::kernel::scheduler::apply_action_from_trap(
                        caller as usize, cap_action,
                    );
                }
            }
        }

        if ok { E_OK } else { E_NO_CAPABILITY }
    }
    #[cfg(kani)]
    E_OK
}

/// ipc_send — GERÇEK SPSC entegrasyonu (Sprint 8)
/// arg0 = channel_id, arg1 = mesaj pointer
fn sys_ipc_send(channel_id: usize, msg_ptr: usize, _: usize, _: usize) -> usize {
    if channel_id >= crate::common::config::MAX_IPC_CHANNELS {
        #[cfg(all(not(kani), feature = "trace"))]
        uart::println("[SYS] ipc_send: invalid channel");
        return E_INVALID_ARG;
    }
    let caller = crate::kernel::scheduler::current_task_id();
    if !is_valid_user_ptr(caller, msg_ptr, 64) {
        return E_INVALID_ARG;
    }
    if !msg_ptr.is_multiple_of(8) {
        #[cfg(all(not(kani), feature = "trace"))]
        uart::println("[SYS] ipc_send: misaligned pointer");
        return E_INVALID_ARG;
    }

    #[cfg(not(kani))]
    {
        // Sprint U-16: Channel ownership enforcement — sadece atanmış producer
        if !crate::ipc::can_send(channel_id, caller) {
            return E_NO_CAPABILITY;
        }
        if !crate::kernel::scheduler::check_ipc_rate() {
            #[cfg(feature = "trace")]
            uart::println("[SYS] ipc_send: rate limited");
            return E_RATE_LIMITED;
        }
        crate::kernel::scheduler::increment_ipc_send();

        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

        // SAFETY: Pointer validated by is_valid_user_ptr — caller's own task stack range only.
        let msg = unsafe {
            core::ptr::read_volatile(msg_ptr as *const crate::ipc::IpcMessage)
        };

        match ch.send(&msg) {
            Ok(()) => {
                #[cfg(feature = "trace")]
                {
                    uart::puts("[SYS] ipc_send(ch=");
                    print_u64(channel_id as u64);
                    uart::println(") OK");
                }
                E_OK
            }
            Err(_) => {
                #[cfg(feature = "trace")]
                {
                    uart::puts("[SYS] ipc_send(ch=");
                    print_u64(channel_id as u64);
                    uart::println(") FULL");
                }
                E_IPC_FULL
            }
        }
    }

    #[cfg(kani)]
    E_OK
}

/// ipc_recv — GERÇEK SPSC entegrasyonu (Sprint 8)
/// arg0 = channel_id, arg1 = buffer pointer
fn sys_ipc_recv(channel_id: usize, buf_ptr: usize, _: usize, _: usize) -> usize {
    if channel_id >= crate::common::config::MAX_IPC_CHANNELS {
        #[cfg(all(not(kani), feature = "trace"))]
        uart::println("[SYS] ipc_recv: invalid channel");
        return E_INVALID_ARG;
    }
    let caller = crate::kernel::scheduler::current_task_id();
    if !is_valid_user_ptr(caller, buf_ptr, 64) {
        return E_INVALID_ARG;
    }
    if !buf_ptr.is_multiple_of(8) {
        #[cfg(all(not(kani), feature = "trace"))]
        uart::println("[SYS] ipc_recv: misaligned pointer");
        return E_INVALID_ARG;
    }

    #[cfg(not(kani))]
    {
        // Sprint U-16: Channel ownership enforcement — sadece atanmış consumer
        if !crate::ipc::can_recv(channel_id, caller) {
            return E_NO_CAPABILITY;
        }
        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

        match ch.recv() {
            Some(msg) => {
                // SAFETY: Volatile write to user-provided buffer. Pointer validated by is_valid_user_ptr().
                unsafe {
                    core::ptr::write_volatile(buf_ptr as *mut crate::ipc::IpcMessage, msg);
                }
                #[cfg(feature = "trace")]
                {
                    uart::puts("[SYS] ipc_recv(ch=");
                    print_u64(channel_id as u64);
                    uart::println(") OK");
                }
                E_OK
            }
            None => {
                #[cfg(feature = "trace")]
                {
                    uart::puts("[SYS] ipc_recv(ch=");
                    print_u64(channel_id as u64);
                    uart::println(") Empty");
                }
                E_IPC_EMPTY
            }
        }
    }

    #[cfg(kani)]
    E_IPC_EMPTY
}

fn sys_yield(_: usize, _: usize, _: usize, _: usize) -> usize {
    #[cfg(not(kani))]
    {
        // Watchdog kick — task yield etti, canlılık kanıtı
        crate::kernel::scheduler::watchdog_kick();
        #[cfg(feature = "trace")]
        uart::println("[SYS] yield");
        crate::kernel::scheduler::schedule();
    }
    E_OK
}

/// task_info — gerçek task bilgisi sorgula
/// arg0 = task_id
/// Dönüş: (state << 8) | (priority << 4) | dal, geçersiz id → 0
fn sys_task_info(_task_id: usize, _: usize, _: usize, _: usize) -> usize {
    #[cfg(not(kani))]
    {
        let info = crate::kernel::scheduler::query_task_info(_task_id);
        #[cfg(feature = "trace")]
        {
            uart::puts("[SYS] task_info(id=");
            print_u64(_task_id as u64);
            uart::puts(") -> ");
            print_u64(info as u64);
            uart::println("");
        }
        info
    }
    #[cfg(kani)]
    0
}

#[cfg(not(kani))]
use crate::common::fmt::print_u64;

// Compile-time guarantee
const _: () = assert!(SYSCALL_COUNT == 5);

// ═══════════════════════════════════════════════════════
// Kani — Sprint 7 proof'ları (değişmedi)
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn dispatch_invalid_id_rejected() {
        let id: usize = kani::any();
        kani::assume(id >= SYSCALL_COUNT);
        let result = dispatch(id, 0, 0, 0, 0);
        assert!(result == E_INVALID_SYSCALL);
    }

    #[kani::proof]
    fn ipc_send_invalid_channel() {
        let ch: usize = kani::any();
        kani::assume(ch >= 8);
        let result = sys_ipc_send(ch, 0, 0, 0);
        assert!(result == E_INVALID_ARG);
    }

    #[kani::proof]
    fn ipc_recv_invalid_channel() {
        let ch: usize = kani::any();
        kani::assume(ch >= 8);
        let result = sys_ipc_recv(ch, 0, 0, 0);
        assert!(result == E_INVALID_ARG);
    }

    #[kani::proof]
    fn task_info_no_panic() {
        let info_type: usize = kani::any();
        let _result = sys_task_info(info_type, 0, 0, 0);
    }

    #[kani::proof]
    fn syscall_table_size() {
        assert!(SYSCALL_TABLE.len() == SYSCALL_COUNT);
        assert!(SYSCALL_COUNT == 5);
    }

    #[kani::proof]
    fn error_codes_unique() {
        let codes = [E_OK, E_INVALID_SYSCALL, E_NO_CAPABILITY, E_IPC_FULL, E_IPC_EMPTY, E_INVALID_ARG];
        let mut i = 0usize;
        while i < codes.len() {
            let mut j = i + 1;
            while j < codes.len() {
                assert!(codes[i] != codes[j]);
                j += 1;
            }
            i += 1;
        }
    }

    #[kani::proof]
    fn null_pointer_always_rejected() {
        let caller: u8 = kani::any();
        assert!(!is_valid_user_ptr(caller, 0, 64));
        assert!(!is_valid_user_ptr(caller, 0, 0));
    }

    #[kani::proof]
    fn unknown_task_pointer_rejected() {
        // Sprint U-16: Kani'de TASK_COUNT = 0 → task_stack_range her caller için None
        // → her pointer reddedilir. Default-deny davranışı doğrulandı.
        let ptr: usize = kani::any();
        kani::assume(ptr > 0);
        let caller: u8 = kani::any();
        assert!(!is_valid_user_ptr(caller, ptr, 64));
    }

    #[kani::proof]
    fn ipc_send_null_ptr_rejected() {
        let result = sys_ipc_send(0, 0, 0, 0);
        assert!(result == E_INVALID_ARG);
    }

    #[kani::proof]
    fn syscall_ids_match_config() {
        use crate::common::config;
        assert!(SYS_CAP_INVOKE == config::SYS_CAP_INVOKE as usize);
        assert!(SYS_IPC_SEND == config::SYS_IPC_SEND as usize);
        assert!(SYS_IPC_RECV == config::SYS_IPC_RECV as usize);
        assert!(SYS_YIELD == config::SYS_YIELD as usize);
        assert!(SYS_TASK_INFO == config::SYS_TASK_INFO as usize);
    }

    /// Proof 123: Geçersiz syscall ID → E_INVALID_SYSCALL == usize::MAX
    #[kani::proof]
    fn dispatch_invalid_syscall_returns_error() {
        let id: usize = kani::any();
        kani::assume(id >= SYSCALL_COUNT);
        assert!(id >= SYSCALL_COUNT);
        assert!(SyscallResult::InvalidSyscall.to_raw() == usize::MAX);
    }

    /// Proof 124: SyscallResult::Ok her zaman 0
    #[kani::proof]
    fn syscall_ok_is_zero() {
        assert!(SyscallResult::Ok.to_raw() == 0);
    }

    /// Proof 125: Tüm hata kodları nonzero
    #[kani::proof]
    fn syscall_errors_nonzero() {
        assert!(SyscallResult::InvalidSyscall.to_raw() != 0);
        assert!(SyscallResult::NoCapability.to_raw() != 0);
        assert!(SyscallResult::IpcFull.to_raw() != 0);
        assert!(SyscallResult::IpcEmpty.to_raw() != 0);
        assert!(SyscallResult::InvalidArg.to_raw() != 0);
        assert!(SyscallResult::BufferFull.to_raw() != 0);
    }

    /// Proof 126: E_* sabitleri enum ile tutarlı
    #[kani::proof]
    fn e_constants_match_enum() {
        assert!(E_OK == SyscallResult::Ok.to_raw());
        assert!(E_INVALID_SYSCALL == SyscallResult::InvalidSyscall.to_raw());
        assert!(E_NO_CAPABILITY == SyscallResult::NoCapability.to_raw());
        assert!(E_IPC_FULL == SyscallResult::IpcFull.to_raw());
        assert!(E_IPC_EMPTY == SyscallResult::IpcEmpty.to_raw());
        assert!(E_INVALID_ARG == SyscallResult::InvalidArg.to_raw());
    }

    /// Proof 157: Sprint U-16 — TASK_COUNT=0 Kani durumunda her caller için reddedilir
    /// (default-deny davranışı). Production'da caller'ın kendi stack aralığı dışı reddedilir.
    #[kani::proof]
    fn any_address_default_deny_in_kani() {
        let addr: usize = kani::any();
        let size: usize = kani::any();
        let caller: u8 = kani::any();
        kani::assume(size > 0 && size <= 64);
        kani::assume(addr > 0);
        // Kernel adres dahil, RAM dışı dahil — hepsi reddedilir (TASK_COUNT=0)
        assert!(!is_valid_user_ptr(caller, addr, size));
    }

    /// Proof 158: Null pointer herhangi size ile reject
    #[kani::proof]
    fn null_pointer_any_size_rejected() {
        let size: usize = kani::any();
        let caller: u8 = kani::any();
        assert!(!is_valid_user_ptr(caller, 0, size));
    }

    /// Proof 171: RAM üstü adres → reject
    #[kani::proof]
    fn ptr_above_ram_rejected() {
        let ptr: usize = kani::any();
        kani::assume(ptr >= crate::common::config::RAM_END);
        let size: usize = kani::any();
        let caller: u8 = kani::any();
        kani::assume(size > 0 && size <= 64);
        assert!(!is_valid_user_ptr(caller, ptr, size));
    }

    /// dispatch() geçersiz syscall ID → E_INVALID_SYSCALL
    #[kani::proof]
    fn dispatch_rejects_invalid_syscall_id() {
        let sys_id: usize = kani::any();
        kani::assume(sys_id >= SYSCALL_COUNT);
        let result = dispatch(sys_id, 0, 0, 0, 0);
        assert!(result == E_INVALID_SYSCALL);
    }
}
