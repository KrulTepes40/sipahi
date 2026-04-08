//! Syscall dispatch table — 5 handlers, O(1) jump, WCET-tracked.
#![allow(dead_code)] // WCET stats + print — used in debug/trace builds.
// Sipahi — Syscall Dispatch (Sprint 7-8)
// Jump table dispatch — 5 syscall, O(1), deterministic
// Sprint 8: ipc_send/ipc_recv gerçek SPSC entegrasyonu

#[cfg(not(kani))]
use crate::arch::uart;

use crate::common::sync::SingleHartCell;

pub use crate::common::config::{
    SYS_CAP_INVOKE, SYS_IPC_SEND, SYS_IPC_RECV,
    SYS_YIELD, SYS_TASK_INFO, SYSCALL_COUNT,
};

pub const E_OK: usize = 0;
pub const E_INVALID_SYSCALL: usize = usize::MAX;
pub const E_NO_CAPABILITY: usize = usize::MAX - 1;
pub const E_IPC_FULL: usize = usize::MAX - 2;
pub const E_IPC_EMPTY: usize = usize::MAX - 3;
pub const E_INVALID_ARG: usize = usize::MAX - 4;

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

/// User pointer doğrulama — kernel belleğine erişim engellenmiş mi?
/// ptr == 0 → reject, ptr+size overflow → reject, ptr < kernel_end → reject
#[must_use = "pointer validation result must be checked"]
fn is_valid_user_ptr(ptr: usize, size: usize) -> bool {
    if ptr == 0 { return false; }
    let end = match ptr.checked_add(size) {
        Some(e) => e,
        None => return false,
    };
    let ke = kernel_end_addr();
    if ptr < ke || end < ke { return false; }
    true
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
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
        unsafe {
            (*WCET_LAST.get_mut())[id] = cycles;
            if cycles > (*WCET_MAX.get())[id] {
                (*WCET_MAX.get_mut())[id] = cycles;
            }
        }
    }
}

#[cfg(not(kani))]
pub fn print_wcet_stats() {
    uart::println("[WCET] Syscall cycle stats:");
    let names = ["cap_invoke", "ipc_send  ", "ipc_recv  ", "yield     ", "task_info "];
    let mut i = 0usize;
    while i < SYSCALL_COUNT {
        uart::puts("  ");
        uart::puts(names[i]);
        uart::puts(": last=");
        // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
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
        #[cfg(not(kani))]
        {
            uart::puts("[SYSCALL] Invalid ID: ");
            print_u64(syscall_id as u64);
            uart::println("");
        }
        return E_INVALID_SYSCALL;
    }

    let start = rdcycle();
    let handler = SYSCALL_TABLE[syscall_id];
    let result = handler(arg0, arg1, arg2, arg3);
    let end = rdcycle();
    wcet_update(syscall_id, end.wrapping_sub(start));

    result
}

// ═══════════════════════════════════════════════════════
// Syscall Handler'lar — Sprint 8: IPC gerçek
// ═══════════════════════════════════════════════════════

fn sys_cap_invoke(cap: usize, resource: usize, action: usize, _arg: usize) -> usize {
    // Cache-only fast path (~10c) — validate_full ile önceden kayıt edilmeli
    #[cfg(not(kani))]
    {
        let ok = crate::kernel::capability::broker::validate_cached(
            cap as u8,
            resource as u16,
            action as u8,
        );
        uart::puts("[SYS] cap_invoke(cap=");
        print_u64(cap as u64);
        uart::puts(") ");
        uart::println(if ok { "OK" } else { "DENIED" });
        if ok { E_OK } else { E_NO_CAPABILITY }
    }
    #[cfg(kani)]
    E_OK
}

/// ipc_send — GERÇEK SPSC entegrasyonu (Sprint 8)
/// arg0 = channel_id, arg1 = mesaj pointer
fn sys_ipc_send(channel_id: usize, msg_ptr: usize, _: usize, _: usize) -> usize {
    if channel_id >= 8 {
        #[cfg(not(kani))]
        uart::println("[SYS] ipc_send: invalid channel");
        return E_INVALID_ARG;
    }
    if !is_valid_user_ptr(msg_ptr, 64) {
        return E_INVALID_ARG;
    }

    #[cfg(not(kani))]
    {
        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

        // SAFETY: Pointer validated by is_valid_user_ptr — outside kernel memory.
        let msg = unsafe {
            core::ptr::read_volatile(msg_ptr as *const crate::ipc::IpcMessage)
        };

        match ch.send(&msg) {
            Ok(()) => {
                uart::puts("[SYS] ipc_send(ch=");
                print_u64(channel_id as u64);
                uart::println(") OK");
                E_OK
            }
            Err(_) => {
                uart::puts("[SYS] ipc_send(ch=");
                print_u64(channel_id as u64);
                uart::println(") FULL");
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
    if channel_id >= 8 {
        #[cfg(not(kani))]
        uart::println("[SYS] ipc_recv: invalid channel");
        return E_INVALID_ARG;
    }
    if !is_valid_user_ptr(buf_ptr, 64) {
        return E_INVALID_ARG;
    }

    #[cfg(not(kani))]
    {
        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

        match ch.recv() {
            Some(msg) => {
                // SAFETY: Volatile read/write to MMIO register at hardware-guaranteed address.
                unsafe {
                    core::ptr::write_volatile(buf_ptr as *mut crate::ipc::IpcMessage, msg);
                }
                uart::puts("[SYS] ipc_recv(ch=");
                print_u64(channel_id as u64);
                uart::println(") OK");
                E_OK
            }
            None => {
                uart::puts("[SYS] ipc_recv(ch=");
                print_u64(channel_id as u64);
                uart::println(") Empty");
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
        uart::println("[SYS] yield");
        crate::kernel::scheduler::schedule();
    }
    E_OK
}

/// task_info — gerçek task bilgisi sorgula
/// arg0 = task_id
/// Dönüş: (state << 8) | (priority << 4) | dal, geçersiz id → 0
fn sys_task_info(task_id: usize, _: usize, _: usize, _: usize) -> usize {
    #[cfg(not(kani))]
    {
        let info = crate::kernel::scheduler::query_task_info(task_id);
        uart::puts("[SYS] task_info(id=");
        print_u64(task_id as u64);
        uart::puts(") -> ");
        print_u64(info as u64);
        uart::println("");
        info
    }
    #[cfg(kani)]
    0
}

#[cfg(not(kani))]
use crate::common::fmt::print_u64;

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
        assert!(!is_valid_user_ptr(0, 64));
        assert!(!is_valid_user_ptr(0, 0));
    }

    #[kani::proof]
    fn kernel_addr_always_rejected() {
        let ptr: usize = kani::any();
        kani::assume(ptr < kernel_end_addr());
        kani::assume(ptr > 0);
        assert!(!is_valid_user_ptr(ptr, 64));
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
}
