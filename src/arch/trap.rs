// Sipahi — Trap Handler (Sprint 2-4)
// trap.S'den çağrılır: a0 = mcause, a1 = mepc
//
// Sprint 4: Timer interrupt → schedule()

#[cfg(not(kani))]
use crate::arch::uart;
#[cfg(not(kani))]
use crate::arch::clint;
#[cfg(not(kani))]
use crate::kernel::scheduler;

const INTERRUPT_BIT: usize = 1 << 63;

#[cfg(not(kani))]
static mut TICK_COUNT: u64 = 0;

#[cfg(not(kani))]
pub fn get_tick_count() -> u64 {
    unsafe { TICK_COUNT }
}

#[cfg(not(kani))]
#[no_mangle]
pub extern "C" fn trap_handler(mcause: usize, mepc: usize) {
    if mcause & INTERRUPT_BIT != 0 {
        let code = mcause & !INTERRUPT_BIT;
        match code {
            7 => {
                // Machine Timer Interrupt
                unsafe { TICK_COUNT += 1 };
                let ticks = get_tick_count();

                // İlk 5 tick'i yazdır
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

                // Sonraki tick'i ayarla
                clint::schedule_next_tick();

                // Scheduler'ı çağır — task switch
                scheduler::schedule();
            }
            _ => {
                uart::puts("[TRAP] Unknown interrupt: ");
                print_u64(code as u64);
                uart::println("");
            }
        }
    } else {
        match mcause {
            8 => {
                uart::println("[TRAP] ecall (syscall)");
            }
            2 => {
                uart::puts("[TRAP] Illegal instruction at 0x");
                print_hex(mepc);
                uart::println("");
            }
            _ => {
                uart::puts("[TRAP] Exception: cause=");
                print_u64(mcause as u64);
                uart::puts(" at 0x");
                print_hex(mepc);
                uart::println("");
            }
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
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        uart::putc(buf[i]);
    }
}

#[cfg(not(kani))]
fn print_hex(mut val: usize) {
    let hex = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = 0;
    if val == 0 {
        uart::putc(b'0');
        return;
    }
    while val > 0 {
        buf[i] = hex[val & 0xF];
        val >>= 4;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        uart::putc(buf[i]);
    }
}
