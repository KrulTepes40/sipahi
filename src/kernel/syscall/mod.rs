// Sipahi — Syscall Modülü (Sprint 7)
// 5 syscall: cap_invoke, ipc_send, ipc_recv, yield, task_info
//
// İki taraf:
//   dispatch.rs — kernel tarafı (trap.rs'den çağrılır)
//   bu dosya    — userspace ecall wrapper'lar (task'lardan çağrılır)
//
// ABI: a7 = syscall ID, a0-a3 = argümanlar, dönüş a0

pub mod dispatch;

// Re-export — trap.rs ve test'ler kullanır
pub use dispatch::dispatch;
#[allow(unused_imports)]
pub use dispatch::{
    E_IPC_EMPTY, E_IPC_FULL, E_INVALID_ARG, E_INVALID_SYSCALL,
    E_NO_CAPABILITY, E_OK, SYSCALL_COUNT, SYS_CAP_INVOKE,
    SYS_IPC_RECV, SYS_IPC_SEND, SYS_TASK_INFO, SYS_YIELD,
};

// ═══════════════════════════════════════════════════════
// Userspace Syscall Wrappers
// Task'lardan çağrılır, ecall tetikler
//
// Şu an M-mode'da: ecall → mcause=11 → trap handler
// Sprint 10 U-mode: ecall → mcause=8 → trap handler
// Wrapper'lar değişmez, trap.S ikisini de destekliyor.
// ═══════════════════════════════════════════════════════

/// cap_invoke — Capability korumalı kaynak erişimi
#[cfg(not(kani))]
#[inline(always)]
pub fn cap_invoke(cap: usize, resource: usize, action: usize, arg: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SYS_CAP_INVOKE,
            inlateout("a0") cap => result,
            in("a1") resource,
            in("a2") action,
            in("a3") arg,
        );
    }
    result
}

/// ipc_send — IPC kanalına mesaj gönder
#[cfg(not(kani))]
#[inline(always)]
pub fn ipc_send(channel_id: usize, msg_ptr: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SYS_IPC_SEND,
            inlateout("a0") channel_id => result,
            in("a1") msg_ptr,
        );
    }
    result
}

/// ipc_recv — IPC kanalından mesaj al
#[cfg(not(kani))]
#[inline(always)]
pub fn ipc_recv(channel_id: usize, buf_ptr: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SYS_IPC_RECV,
            inlateout("a0") channel_id => result,
            in("a1") buf_ptr,
        );
    }
    result
}

/// yield — Gönüllü CPU bırakma
#[cfg(not(kani))]
#[inline(always)]
pub fn yield_cpu() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SYS_YIELD,
            lateout("a0") result,
        );
    }
    result
}

/// task_info — Task bilgisi sorgula
#[cfg(not(kani))]
#[inline(always)]
pub fn task_info(info_type: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SYS_TASK_INFO,
            inlateout("a0") info_type => result,
        );
    }
    result
}

// ═══════════════════════════════════════════════════════
// Kani stubs — ecall assembly Kani'de çalışmaz
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
pub fn cap_invoke(_: usize, _: usize, _: usize, _: usize) -> usize { E_OK }

#[cfg(kani)]
pub fn ipc_send(_: usize, _: usize) -> usize { E_OK }

#[cfg(kani)]
pub fn ipc_recv(_: usize, _: usize) -> usize { E_IPC_EMPTY }

#[cfg(kani)]
pub fn yield_cpu() -> usize { E_OK }

#[cfg(kani)]
pub fn task_info(_: usize) -> usize { 0 }
