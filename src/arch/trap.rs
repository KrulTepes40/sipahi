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
#[allow(dead_code)]
pub fn get_tick_count() -> u64 {
    // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
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
        // debug-boot feature off iken _ arm boş — single_match uyarısını bastır
        #[allow(clippy::single_match)]
        match code {
            7 => {
                // Machine Timer Interrupt
                // SAFETY: MIE=0 in trap context, single-hart — no concurrent access.
                unsafe { *TICK_COUNT.get_mut() += 1 };
                let ticks = unsafe { *TICK_COUNT.get() };

                #[cfg(feature = "debug-boot")]
                {
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
                }

                let overrun = clint::schedule_next_tick();
                // Grace period: ilk 10 tick'te overrun ignore (boot testleri
                // mtime'ı çok ilerletiyor, false positive önle)
                if overrun && ticks > 10 {
                    let task_id = scheduler::current_task_id();
                    // SAFETY: MIE=0 in trap context, single-hart.
                    let dal = unsafe {
                        crate::kernel::scheduler::TASKS.get()[task_id as usize].dal
                    };
                    crate::ipc::blackbox::log(
                        crate::ipc::blackbox::BlackboxEvent::DeadlineMiss,
                        task_id, &[],
                    );
                    let action = crate::kernel::policy::apply_policy(
                        task_id,
                        crate::kernel::policy::PolicyEvent::DeadlineMiss,
                        dal,
                    );
                    scheduler::apply_action_from_trap(task_id as usize, action);
                }
                scheduler::schedule();
            }
            _ => {
                #[cfg(feature = "debug-boot")]
                {
                    uart::puts("[TRAP] Unknown interrupt: ");
                    print_u64(code as u64);
                    uart::println("");
                }
            }
        }
        0 // interrupt: trap.S saved a0'a dokunmaz
    } else {
        // ═══ Exception ═══
        match mcause {
            ECALL_U | ECALL_M => {
                // Sprint U-15: MPP kontrolü dispatch ÖNCESİNE alındı.
                // Önceden dispatch sonrası kontrol ediliyordu — kötü niyetli
                // ecall yine de dispatch ediliyordu. Şimdi privilege escalation
                // tespiti syscall hiç çalışmadan yapılıyor.
                if mcause == ECALL_U {
                    verify_mpp_is_user_mode();
                }
                // ecall → syscall dispatch
                // mepc+4 trap.S'de yapıldı, burada yapılmaz
                crate::kernel::syscall::dispatch(syscall_id, arg0, arg1, arg2, arg3)
            }
            2 => {
                // Illegal instruction — task izole edilmeli
                #[cfg(feature = "debug-boot")]
                {
                    uart::puts("[TRAP] Illegal instruction at 0x");
                    print_hex(_mepc);
                    uart::println(" → policy");
                }
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    0xFF, &[],
                );
                crate::kernel::scheduler::handle_task_fault();
                0
            }
            5 | 7 => {
                // Load/StoreAccessFault — PMP violation
                // Fault adresi task stacks bölgesinde → StackOverflow (policy path)
                // Dışında → genel PmpFail (WasmTrap via handle_task_fault)
                let fault_addr = crate::arch::csr::read_mtval();
                let task_id = crate::kernel::scheduler::current_task_id();

                #[cfg(feature = "debug-boot")]
                {
                    let fault_name = if mcause == 5 {
                        "LoadAccessFault"
                    } else {
                        "StoreAccessFault"
                    };
                    uart::puts("[TRAP] ");
                    uart::puts(fault_name);
                    uart::puts(" at 0x");
                    print_hex(fault_addr);
                    uart::puts(" mepc=0x");
                    print_hex(_mepc);
                    uart::println("");
                }

                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PmpFail,
                    task_id, &[],
                );

                // Fault adresi task stacks bölgesinde mi?
                let (stack_start, stack_end) =
                    crate::kernel::memory::task_stacks_range();
                let in_task_stacks = fault_addr >= stack_start
                                  && fault_addr < stack_end;

                if in_task_stacks {
                    // Stack overflow veya cross-task stack erişimi
                    // SAFETY: MIE=0 in trap context, single-hart.
                    let dal = unsafe {
                        crate::kernel::scheduler::TASKS.get()[task_id as usize].dal
                    };
                    let action = crate::kernel::policy::apply_policy(
                        task_id,
                        crate::kernel::policy::PolicyEvent::StackOverflow,
                        dal,
                    );
                    crate::kernel::scheduler::apply_action_from_trap(
                        task_id as usize, action,
                    );
                } else {
                    // Genel PMP violation (WASM arena, kernel bölgesi vb.)
                    crate::kernel::scheduler::handle_task_fault();
                }
                0
            }
            _ => {
                #[cfg(feature = "debug-boot")]
                {
                    uart::puts("[TRAP] Exception: cause=");
                    print_u64(mcause as u64);
                    uart::puts(" at 0x");
                    print_hex(_mepc);
                    uart::println("");
                }
                0
            }
        }
    }
}

#[cfg(all(not(kani), feature = "debug-boot"))]
use crate::common::fmt::{print_u64, print_hex};
