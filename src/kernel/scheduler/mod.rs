// Sipahi — Scheduler (Sprint 4)
// Preemptive round-robin scheduler
//
// 8 statik task, priority-based (Sprint 7'de)
// Şimdilik: round-robin, timer tick ile preemption

use crate::common::config::{MAX_TASKS, TASK_STACK_SIZE};
use crate::common::types::TaskState;

// ═══════════════════════════════════════════════════════
// TaskContext: callee-saved registers
// ═══════════════════════════════════════════════════════

/// Callee-saved register'lar — context.S ile eşleşmeli
/// 14 register × 8 byte = 112 byte
#[repr(C)]
pub struct TaskContext {
    pub ra:  usize,  // Return address
    pub sp:  usize,  // Stack pointer
    pub s0:  usize,  // Saved registers
    pub s1:  usize,
    pub s2:  usize,
    pub s3:  usize,
    pub s4:  usize,
    pub s5:  usize,
    pub s6:  usize,
    pub s7:  usize,
    pub s8:  usize,
    pub s9:  usize,
    pub s10: usize,
    pub s11: usize,
}

impl TaskContext {
    pub const fn zero() -> Self {
        TaskContext {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0,
            s4: 0, s5: 0, s6: 0, s7: 0,
            s8: 0, s9: 0, s10: 0, s11: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════
// Task Control Block
// ═══════════════════════════════════════════════════════

pub struct Task {
    pub id: u8,
    pub state: TaskState,
    pub context: TaskContext,
}

impl Task {
    pub const fn empty() -> Self {
        Task {
            id: 0,
            state: TaskState::Dead, // Başlatılmamış = Dead
            context: TaskContext::zero(),
        }
    }
}

// ═══════════════════════════════════════════════════════
// Scheduler
// ═══════════════════════════════════════════════════════

/// Task stack'leri — statik, heap yok
static mut TASK_STACKS: [[u8; TASK_STACK_SIZE]; MAX_TASKS] = [[0u8; TASK_STACK_SIZE]; MAX_TASKS];

/// Task array — statik
static mut TASKS: [Task; MAX_TASKS] = [
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
    Task::empty(), Task::empty(), Task::empty(), Task::empty(),
];

/// Aktif task sayısı
static mut TASK_COUNT: usize = 0;

/// Şu anki çalışan task index'i
static mut CURRENT_TASK: usize = 0;

// context.S'deki switch_context fonksiyonu
extern "C" {
    fn switch_context(old: *mut TaskContext, new: *const TaskContext);
}

/// Yeni task oluştur
pub fn create_task(entry: fn() -> !) -> Option<u8> {
    unsafe {
        if TASK_COUNT >= MAX_TASKS {
            return None;
        }

        let id = TASK_COUNT;

        // Stack'in tepesini hesapla (stack yukarıdan aşağı büyür)
        // 16-byte aligned olmalı (RISC-V ABI)
        let stack_top = TASK_STACKS[id].as_ptr() as usize + TASK_STACK_SIZE;
        let stack_top_aligned = stack_top & !0xF;

        TASKS[id].id = id as u8;
        TASKS[id].state = TaskState::Ready;
        TASKS[id].context = TaskContext::zero();
        TASKS[id].context.ra = entry as usize;
        TASKS[id].context.sp = stack_top_aligned;

        TASK_COUNT += 1;
        Some(id as u8)
    }
}

/// Scheduler tick — timer interrupt'tan çağrılır
/// Round-robin: sıradaki Ready task'a geç
pub fn schedule() {
    unsafe {
        if TASK_COUNT < 2 {
            return;
        }

        let old = CURRENT_TASK;
        let mut next = (old + 1) % TASK_COUNT;

        // Sıradaki Ready task'ı bul (bounded loop)
        let mut i = 0;
        while i < TASK_COUNT {
            if TASKS[next].state == TaskState::Ready || TASKS[next].state == TaskState::Running {
                break;
            }
            next = (next + 1) % TASK_COUNT;
            i += 1;
        }

        if next == old {
            return;
        }

        if TASKS[old].state == TaskState::Running {
            TASKS[old].state = TaskState::Ready;
        }
        TASKS[next].state = TaskState::Running;
        CURRENT_TASK = next;

        let old_ctx = &mut TASKS[old].context as *mut TaskContext;
        let new_ctx = &TASKS[next].context as *const TaskContext;
        switch_context(old_ctx, new_ctx);
    }
}

/// İlk task'ı başlat (boot'tan çağrılır)
pub fn start_first_task() -> ! {
    unsafe {
        if TASK_COUNT == 0 {
            panic!("No tasks to run!");
        }

        TASKS[0].state = TaskState::Running;
        CURRENT_TASK = 0;

        let ctx = &TASKS[0].context;
        let entry = ctx.ra;
        let sp = ctx.sp;

        core::arch::asm!(
            "mv sp, {sp}",
            "jr {entry}",
            sp = in(reg) sp,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}
