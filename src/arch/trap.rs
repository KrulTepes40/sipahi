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
//   ecall -> syscall sonucu (trap.S saved a0'a yazar)
//   interrupt -> 0 (trap.S saved a0'a dokunmaz)

#[cfg(all(not(kani), any(feature = "debug-boot", feature = "cross-isolation-demo")))]
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
        crate::ipc::blackbox::log(
            crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
            crate::common::config::SYSTEM_TASK_ID, &[],
        );
        crate::common::halt_system("[TRAP] PRIVILEGE ESCALATION DETECTED — SHUTDOWN");
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
                // SAFETY: Read-only access to TICK_COUNT; MIE=0 in trap context.
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
                scheduler::schedule_timer_tick();
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
                // ecall -> syscall dispatch
                // mepc+4 trap.S'de yapıldı, burada yapılmaz
                crate::kernel::syscall::dispatch(syscall_id, arg0, arg1, arg2, arg3)
            }
            2 => {
                // Illegal instruction — task izole edilmeli
                #[cfg(feature = "debug-boot")]
                {
                    uart::puts("[TRAP] Illegal instruction at 0x");
                    print_hex(_mepc);
                    uart::println(" -> policy");
                }
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::kernel::scheduler::handle_task_fault();
                0
            }
            5 | 7 => {
                // Load/StoreAccessFault — PMP violation
                // Fault adresi task stacks bölgesinde -> StackOverflow (policy path)
                // Dışında -> genel PmpFail (TaskFault via handle_task_fault)
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

                // U-27.5 SNTM-R12 runtime observation: IN-HANDLER state check.
                //
                // Kernel policy (TaskFault, DAL-D): restart_count < MAX_RESTART_FAULT=3
                // iken Restart, sonra Isolate (decide_action policy/mod.rs:107-110).
                // İlk 3 trap'te task Restart edilir (state=Ready), 4. trap'te
                // Isolated. Bu policy DAL-D için 3-şans davranışı — DOĞRU.
                //
                // Marker pattern (kullanıcı dikkat 2+3 uyumlu):
                //   - [OK]: SADECE task=2 attacker + Isolated + victim Ready/Running
                //   - [FAIL]: GERÇEK anomaly (attacker ≠ 2, ya da victim_runnable=0)
                //   - Restart sırasında (attacker=2 ama henüz Isolated değil,
                //     victim runnable): SILENT — marker emit ETME. Bu beklenen
                //     policy davranışı, sahte [FAIL] yazmak yanlış olur.
                //
                // Script Gate 1 ([OK] var): 4. trap sonu marker görür.
                // Script Gate 2 ([FAIL] yok): collateral damage YOKSA gate PASS.
                // Production build'de feature compile-out → marker hiç yazılmaz.
                #[cfg(feature = "cross-isolation-demo")]
                {
                    use crate::common::types::TaskState;
                    let attacker_state =
                        crate::kernel::scheduler::task_state_for_test(task_id);
                    let victim_state =
                        crate::kernel::scheduler::task_state_for_test(3);
                    let attacker_isolated =
                        matches!(attacker_state, TaskState::Isolated);
                    let victim_runnable = matches!(
                        victim_state,
                        TaskState::Ready | TaskState::Running
                    );

                    if task_id == 2 && attacker_isolated && victim_runnable {
                        // [OK]: Isolate path tamamlandı, victim sağlam.
                        uart::puts("[OK] Cross-task PMP isolation enforced: task=");
                        print_u64(task_id as u64);
                        uart::puts(" attempted=0x");
                        print_hex(fault_addr);
                        uart::println(" REJECTED");
                    } else if task_id == 2 && !victim_runnable {
                        // [FAIL]: Collateral damage — task_world durdu/öldü.
                        // Kernel policy yanlış task'ı etkiledi VEYA cross-task
                        // izolasyon bozuldu (PMP bypass).
                        uart::puts("[FAIL] Cross-task PMP isolation BROKEN: attacker=");
                        print_u64(task_id as u64);
                        uart::puts(" victim_runnable=0 attacker_isolated=");
                        print_u64(if attacker_isolated { 1 } else { 0 });
                        uart::println("");
                    } else if task_id != 2 {
                        // [FAIL]: Yanlış attacker — beklenmeyen task ihlal yaptı.
                        uart::puts("[FAIL] Cross-task PMP isolation BROKEN: unexpected_attacker=");
                        print_u64(task_id as u64);
                        uart::println("");
                    }
                    // else: attacker=2, !isolated, victim_runnable → restart
                    //       sırasında, SILENT (beklenen policy davranışı).
                }
                0
            }
            // U-21 GÖREV 4 [H6]: Unknown exception triage — fail-closed.
            // Önceden default arm `_ => 0` dönüyordu; ecall dışında trap.S
            // mepc += 4 yapmadığı için faulting instruction'a dönüyor ->
            // sonsuz trap loop / livelock DoS. Şimdi her exception class
            // için explicit dispatch:
            //   - Task fault'lar (misaligned, breakpoint) -> handle_task_fault
            //   - Hardware integrity (bus error, page fault) -> SHUTDOWN
            //   - Bilinmeyen -> fail-closed SHUTDOWN
            0 => {
                // Instruction misaligned — RV64IMAC C-extension ile imkansız
                // (16-bit instructions allowed); fail-closed isolate
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::kernel::scheduler::handle_task_fault();
                0
            }
            3 => {
                // Breakpoint (ebreak) — debugger trap, U-mode'dan task isolate
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::kernel::scheduler::handle_task_fault();
                0
            }
            4 | 6 => {
                // Load/Store address misaligned — task fault, isolate
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyIsolate,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::kernel::scheduler::handle_task_fault();
                0
            }
            1 => {
                // Instruction access fault — bus error / hw integrity
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PmpFail,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::common::halt_system("[TRAP] FATAL: instruction access fault");
            }
            9 | 10 => {
                // S-mode/H-mode ecall (impossible — S-mode yok)
                crate::common::halt_system("[TRAP] FATAL: S/H-mode ecall (impossible)");
            }
            12..=15 => {
                // Page fault — paging yok, donanım/firmware hatası
                crate::common::halt_system("[TRAP] FATAL: page fault (no paging)");
            }
            _ => {
                // Bilinmeyen exception class — fail-closed SHUTDOWN
                #[cfg(feature = "debug-boot")]
                {
                    uart::puts("[TRAP] Unknown exception: cause=");
                    print_u64(mcause as u64);
                    uart::puts(" at 0x");
                    print_hex(_mepc);
                    uart::println("");
                }
                crate::ipc::blackbox::log(
                    crate::ipc::blackbox::BlackboxEvent::PolicyShutdown,
                    crate::common::config::SYSTEM_TASK_ID, &[],
                );
                crate::common::halt_system("[TRAP] FATAL: unknown exception");
            }
        }
    }
}

#[cfg(all(not(kani), any(feature = "debug-boot", feature = "cross-isolation-demo")))]
use crate::common::fmt::{print_u64, print_hex};
