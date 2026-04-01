// Sipahi — Syscall Dispatch (Sprint 7)
// Jump table dispatch — 5 syscall, O(1), deterministic
//
// ABI: a7 = syscall ID, a0-a3 = argümanlar, dönüş a0
// Jump table: SYSCALL_TABLE[id](args) — 1 load + 1 indirect call
// Geçersiz ID → E_INVALID_SYSCALL (bounds check)
//
// WCET: rdcycle ile giriş/çıkış farkı
// Kani: bounds check, panic-freedom, hata kodları

#[cfg(not(kani))]
use crate::arch::uart;

// ═══════════════════════════════════════════════════════
// Syscall ID sabitleri (dispatch'e özgü, usize)
// config.rs'teki u64 versiyonlarla Kani proof'ta eşleştirilir
// ═══════════════════════════════════════════════════════

pub const SYS_CAP_INVOKE: usize = 0;
pub const SYS_IPC_SEND: usize = 1;
pub const SYS_IPC_RECV: usize = 2;
pub const SYS_YIELD: usize = 3;
pub const SYS_TASK_INFO: usize = 4;

/// Jump table boyutu
pub const SYSCALL_COUNT: usize = 5;

// ═══════════════════════════════════════════════════════
// Hata kodları (usize — a0'a yazılır)
// ═══════════════════════════════════════════════════════

pub const E_OK: usize = 0;
pub const E_INVALID_SYSCALL: usize = usize::MAX;
pub const E_NO_CAPABILITY: usize = usize::MAX - 1;
pub const E_IPC_FULL: usize = usize::MAX - 2;
pub const E_IPC_EMPTY: usize = usize::MAX - 3;
pub const E_INVALID_ARG: usize = usize::MAX - 4;

// ═══════════════════════════════════════════════════════
// Jump Table — compile-time sabit
// ═══════════════════════════════════════════════════════

type SyscallHandler = fn(usize, usize, usize, usize) -> usize;

static SYSCALL_TABLE: [SyscallHandler; SYSCALL_COUNT] = [
    sys_cap_invoke,  // 0
    sys_ipc_send,    // 1
    sys_ipc_recv,    // 2
    sys_yield,       // 3
    sys_task_info,   // 4
];

// ═══════════════════════════════════════════════════════
// rdcycle — WCET ölçümü
// ═══════════════════════════════════════════════════════

#[cfg(not(kani))]
#[inline(always)]
fn rdcycle() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!("rdcycle {}", out(reg) val);
    }
    val
}

#[cfg(kani)]
#[inline(always)]
fn rdcycle() -> u64 {
    0
}

// ═══════════════════════════════════════════════════════
// WCET istatistik — tek hart, lock-free
// ═══════════════════════════════════════════════════════

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

/// WCET istatistiklerini yazdır (debug)
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

// ═══════════════════════════════════════════════════════
// Ana Dispatch — trap.rs'den çağrılır
// ═══════════════════════════════════════════════════════

/// Syscall dispatch — jump table ile O(1)
///
/// Geçerli ID: 1 bounds check + 1 table load + 1 call
/// Geçersiz ID: 1 bounds check + hata dönüşü
#[inline(never)]
pub fn dispatch(
    syscall_id: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    // Bounds check
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

    // Jump table — 1 load + 1 indirect call
    let handler = SYSCALL_TABLE[syscall_id];
    let result = handler(arg0, arg1, arg2, arg3);

    let end = rdcycle();
    wcet_update(syscall_id, end.wrapping_sub(start));

    result
}

// ═══════════════════════════════════════════════════════
// Syscall Stub Handler'lar
// Sprint 7: stub — Sprint 8+ gerçek implementasyon
// ═══════════════════════════════════════════════════════

/// SYS 0: cap_invoke — Capability korumalı kaynak erişimi
/// Sprint 9'da gerçek implementasyon
fn sys_cap_invoke(cap: usize, resource: usize, action: usize, _arg: usize) -> usize {
    #[cfg(not(kani))]
    {
        uart::puts("[SYS] cap_invoke(cap=");
        print_u64(cap as u64);
        uart::puts(", res=");
        print_u64(resource as u64);
        uart::puts(", act=");
        print_u64(action as u64);
        uart::println(")");
    }
    E_OK
}

/// SYS 1: ipc_send — IPC kanalına mesaj gönder
/// Sprint 8'de SPSC ring buffer entegrasyonu
fn sys_ipc_send(channel_id: usize, _msg_ptr: usize, _: usize, _: usize) -> usize {
    if channel_id >= 8 {
        #[cfg(not(kani))]
        uart::println("[SYS] ipc_send: invalid channel");
        return E_INVALID_ARG;
    }
    #[cfg(not(kani))]
    {
        uart::puts("[SYS] ipc_send(ch=");
        print_u64(channel_id as u64);
        uart::println(")");
    }
    E_OK
}

/// SYS 2: ipc_recv — IPC kanalından mesaj al
/// Sprint 8'de SPSC ring buffer entegrasyonu
fn sys_ipc_recv(channel_id: usize, _buf_ptr: usize, _: usize, _: usize) -> usize {
    if channel_id >= 8 {
        #[cfg(not(kani))]
        uart::println("[SYS] ipc_recv: invalid channel");
        return E_INVALID_ARG;
    }
    #[cfg(not(kani))]
    {
        uart::puts("[SYS] ipc_recv(ch=");
        print_u64(channel_id as u64);
        uart::println(") -> Empty");
    }
    E_IPC_EMPTY
}

/// SYS 3: yield — Gönüllü CPU bırakma
/// Sprint 10'da scheduler entegrasyonu
fn sys_yield(_: usize, _: usize, _: usize, _: usize) -> usize {
    #[cfg(not(kani))]
    uart::println("[SYS] yield");
    E_OK
}

/// SYS 4: task_info — Task bilgisi sorgula
/// Sprint 10'da gerçek scheduler state
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

// ═══════════════════════════════════════════════════════
// Yardımcı yazdırma
// ═══════════════════════════════════════════════════════

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
// Kani Formal Verification — Sprint 7
// ═══════════════════════════════════════════════════════

#[cfg(kani)]
mod verification {
    use super::*;

    /// Proof 26: Geçersiz syscall ID → E_INVALID_SYSCALL
    #[kani::proof]
    fn dispatch_invalid_id_rejected() {
        let id: usize = kani::any();
        kani::assume(id >= SYSCALL_COUNT);
        let result = dispatch(id, 0, 0, 0, 0);
        assert!(result == E_INVALID_SYSCALL);
    }

    /// Proof 27: ipc_send kanal ≥8 → E_INVALID_ARG
    #[kani::proof]
    fn ipc_send_invalid_channel() {
        let ch: usize = kani::any();
        kani::assume(ch >= 8);
        let result = sys_ipc_send(ch, 0, 0, 0);
        assert!(result == E_INVALID_ARG);
    }

    /// Proof 28: ipc_recv kanal ≥8 → E_INVALID_ARG
    #[kani::proof]
    fn ipc_recv_invalid_channel() {
        let ch: usize = kani::any();
        kani::assume(ch >= 8);
        let result = sys_ipc_recv(ch, 0, 0, 0);
        assert!(result == E_INVALID_ARG);
    }

    /// Proof 29: task_info herhangi input → panic-free
    #[kani::proof]
    fn task_info_no_panic() {
        let info_type: usize = kani::any();
        let _result = sys_task_info(info_type, 0, 0, 0);
        // panic olmazsa PASS
    }

    /// Proof 30: Jump table boyutu == SYSCALL_COUNT
    #[kani::proof]
    fn syscall_table_size() {
        assert!(SYSCALL_TABLE.len() == SYSCALL_COUNT);
        assert!(SYSCALL_COUNT == 5);
    }

    /// Proof 31: Hata kodları benzersiz
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

    /// Proof 32: Syscall ID'leri config.rs ile eşleşiyor
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
