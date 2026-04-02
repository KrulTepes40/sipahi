// Sipahi — Syscall Dispatch (Sprint 7-8)
// Jump table dispatch — 5 syscall, O(1), deterministic
// Sprint 8: ipc_send/ipc_recv gerçek SPSC entegrasyonu

#[cfg(not(kani))]
use crate::arch::uart;

pub const SYS_CAP_INVOKE: usize = 0;
pub const SYS_IPC_SEND: usize = 1;
pub const SYS_IPC_RECV: usize = 2;
pub const SYS_YIELD: usize = 3;
pub const SYS_TASK_INFO: usize = 4;
pub const SYSCALL_COUNT: usize = 5;

pub const E_OK: usize = 0;
pub const E_INVALID_SYSCALL: usize = usize::MAX;
pub const E_NO_CAPABILITY: usize = usize::MAX - 1;
pub const E_IPC_FULL: usize = usize::MAX - 2;
pub const E_IPC_EMPTY: usize = usize::MAX - 3;
pub const E_INVALID_ARG: usize = usize::MAX - 4;

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
    unsafe { core::arch::asm!("rdcycle {}", out(reg) val); }
    val
}

#[cfg(kani)]
#[inline(always)]
fn rdcycle() -> u64 { 0 }

static mut WCET_MAX: [u64; SYSCALL_COUNT] = [0; SYSCALL_COUNT];
static mut WCET_LAST: [u64; SYSCALL_COUNT] = [0; SYSCALL_COUNT];

#[inline(always)]
fn wcet_update(id: usize, cycles: u64) {
    if id < SYSCALL_COUNT {
        unsafe {
            WCET_LAST[id] = cycles;
            if cycles > WCET_MAX[id] {
                WCET_MAX[id] = cycles;
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
        let (last, max) = unsafe { (WCET_LAST[i], WCET_MAX[i]) };
        print_u64(last);
        uart::puts(" max=");
        print_u64(max);
        uart::println(" cycles");
        i += 1;
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

    #[cfg(not(kani))]
    {
        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

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
            Err(()) => {
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

    #[cfg(not(kani))]
    {
        let ch = match crate::ipc::get_channel(channel_id) {
            Some(c) => c,
            None => return E_INVALID_ARG,
        };

        match ch.recv() {
            Some(msg) => {
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

fn sys_task_info(info_type: usize, _: usize, _: usize, _: usize) -> usize {
    match info_type {
        0 => {
            #[cfg(not(kani))]
            uart::println("[SYS] task_info(task_id) -> 0");
            0
        }
        1 => {
            #[cfg(not(kani))]
            uart::println("[SYS] task_info(priority) -> 1");
            1
        }
        2 => {
            #[cfg(not(kani))]
            uart::println("[SYS] task_info(budget) -> 10000");
            10_000
        }
        3 => {
            #[cfg(not(kani))]
            uart::println("[SYS] task_info(state) -> RUNNING");
            0
        }
        _ => {
            #[cfg(not(kani))]
            uart::println("[SYS] task_info: invalid type");
            E_INVALID_ARG
        }
    }
}

#[cfg(not(kani))]
fn print_u64(mut val: u64) {
    if val == 0 {
        uart::putc(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0usize;
    while val > 0 && i < 20 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        uart::putc(buf[i]);
    }
}

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
    fn syscall_ids_match_config() {
        use crate::common::config;
        assert!(SYS_CAP_INVOKE == config::SYS_CAP_INVOKE as usize);
        assert!(SYS_IPC_SEND == config::SYS_IPC_SEND as usize);
        assert!(SYS_IPC_RECV == config::SYS_IPC_RECV as usize);
        assert!(SYS_YIELD == config::SYS_YIELD as usize);
        assert!(SYS_TASK_INFO == config::SYS_TASK_INFO as usize);
    }
}
