//! M-mode trap handler — timer interrupt, ECALL dispatch, fault reporting.
// Sipahi — Trap Handler (Sprint 2-7)
// trap.S'den çağrılır:
//   a0 = mcause
//   a1 = mepc (ecall için trap.S'de +4 ilerletilmiş)
//   a2 = orijinal a7 (syscall ID)
//   a3 = orijinal a0 (arg0)
//   a4 = orijinal a1 (arg1)
//   a5 = orijinal a2 (arg2)
//   a6 = orijinal a3 (arg3)
//
// Dönüş: usize
//   ecall → syscall sonucu (trap.S saved a0'a yazar)
//   interrupt → 0 (trap.S saved a0'a dokunmaz)

#[cfg(not(kani))]
use crate::arch::uart;
#[cfg(not(kani))]
use crate::arch::clint;
#[cfg(not(kani))]
use crate::kernel::scheduler;
#[cfg(not(kani))]
use crate::common::sync::SingleHartCell;

/// RV64 mcause interrupt bit — bit 63
const INTERRUPT_BIT: usize = 1 << 63;

/// ecall from U-mode
const ECALL_U: usize = 8;

/// ecall from M-mode (şu an M-mode'da çalışıyoruz)
const ECALL_M: usize = 11;

/// Tick sayacı
#[cfg(not(kani))]
static TICK_COUNT: SingleHartCell<u64> = SingleHartCell::new(0);

#[cfg(not(kani))]
pub fn get_tick_count() -> u64 {
    // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
    unsafe { *TICK_COUNT.get() }
}

/// mstatus.MPP kontrol — U-mode görev M-mode'a yükselemez
#[cfg(not(kani))]
#[inline(always)]
fn verify_mpp_is_user_mode() {
    let mstatus = crate::arch::csr::read_mstatus();
    let mpp = (mstatus >> 11) & 0x3;
    if mpp != 0 {
        uart::println("[TRAP] PRIVILEGE ESCALATION DETECTED — SHUTDOWN");
        crate::ipc::blackbox::log(
            crate::ipc::blackbox::BlackboxEvent::PolicyShutdown, 0xFF, &[],
        );
        loop { unsafe { core::arch::asm!("wfi"); } }
    }
}

#[cfg(not(kani))]
#[no_mangle]
pub extern "C" fn trap_handler(
    mcause: usize,
    _mepc: usize,
    syscall_id: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    if mcause & INTERRUPT_BIT != 0 {
        // ═══ Interrupt ═══
        let code = mcause & !INTERRUPT_BIT;
        match code {
            7 => {
                // Machine Timer Interrupt
                // SAFETY: Single-hart system, interrupts disabled during boot — no concurrent access.
                unsafe { *TICK_COUNT.get_mut() += 1 };
                let ticks = get_tick_count();

                if ticks <= 5 {
                    uart::puts("[TICK] #");
                    print_u64(ticks);
                    uart::puts(" mtime=");
                    print_u64(clint::read_mtime());
                    uart::println("");
                }

                if ticks == 5 {
                    uart::println("[TICK] (further ticks silent)");
                }

                clint::schedule_next_tick();
                scheduler::schedule();
            }
            _ => {
                uart::puts("[TRAP] Unknown interrupt: ");
                print_u64(code as u64);
                uart::println("");
            }
        }
        0 // interrupt: trap.S saved a0'a dokunmaz
    } else {
        // ═══ Exception ═══
        match mcause {
            ECALL_U | ECALL_M => {
                // ecall → syscall dispatch
                // mepc+4 trap.S'de yapıldı, burada yapılmaz
                let r = crate::kernel::syscall::dispatch(
                    syscall_id, arg0, arg1, arg2, arg3,
                );
                // MPP kontrolü sadece U-mode ecall'da — M-mode ecall'da MPP=3 doğru
                if mcause == ECALL_U {
                    verify_mpp_is_user_mode();
                }
                r
            }
            2 => {
                // Illegal instruction — task izole edilmeli
                uart::puts("[TRAP] Illegal instruction at 0x");
                print_hex(_mepc);
                uart::println(" → policy");
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    0xFF, &[],
                );
                crate::kernel::scheduler::handle_illegal_instruction();
                0
            }
            _ => {
                uart::puts("[TRAP] Exception: cause=");
                print_u64(mcause as u64);
                uart::puts(" at 0x");
                print_hex(_mepc);
                uart::println("");
                0
            }
        }
    }
}

#[cfg(not(kani))]
use crate::common::fmt::{print_u64, print_hex};
