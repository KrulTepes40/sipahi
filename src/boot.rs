//! Boot sequence — PMP, HAL, task creation, crypto init, timer.

use crate::arch;
use crate::kernel;
use crate::ipc;
use crate::common::fmt::print_u32;

extern "C" {
    fn trap_entry();
}

/// Boot initialization — PMP, blackbox, HAL, task creation
pub fn init() {
    arch::csr::write_mtvec(trap_entry as *const () as usize);
    arch::uart::puts("[BOOT] mtvec = 0x");
    crate::common::fmt::print_hex(arch::csr::read_mtvec());
    arch::uart::println("");

    kernel::memory::init_pmp();
    ipc::blackbox::init();

    arch::uart::println("[HAL]  Device trait registered");
    arch::uart::println("[HAL]  IOPMP stub ready");

    use crate::common::types::TaskConfig;

    let id_a = kernel::scheduler::create_task(&TaskConfig {
        entry: crate::task_a, priority: 4, dal: 1,
        budget_cycles: 300_000, period_ticks: 10,
    });
    let id_b = kernel::scheduler::create_task(&TaskConfig {
        entry: crate::task_b, priority: 8, dal: 2,
        budget_cycles: 200_000, period_ticks: 10,
    });
    arch::uart::puts("[BOOT] Task A: id=");
    print_u32(id_a.unwrap_or(255) as u32);
    arch::uart::puts(" prio=4 dal=B budget=300K/period");
    arch::uart::println("");
    arch::uart::puts("[BOOT] Task B: id=");
    print_u32(id_b.unwrap_or(255) as u32);
    arch::uart::puts(" prio=8 dal=C budget=200K/period");
    arch::uart::println("");
}

/// Final boot — timer arm + scheduler start (diverges, never returns)
pub fn start() -> ! {
    arch::csr::enable_timer_interrupt();
    arch::clint::init_timer();
    arch::uart::println("[BOOT] Timer armed");
    arch::uart::println("[BOOT] Starting scheduler...");
    arch::uart::println("");

    kernel::scheduler::start_first_task();
}
