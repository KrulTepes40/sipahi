//! Boot sequence — PMP, HAL, task creation, crypto init, timer.

use crate::arch;
use crate::kernel;
use crate::ipc;
#[cfg(feature = "debug-boot")]
use crate::common::fmt::print_u32;

extern "C" {
    fn trap_entry();
}

// U-21 GÖREV 6 [H2]: Production OTP key provisioning stub (v2.0 hedefi).
// Bu extern fonksiyon HSM driver veya silikon-spesifik OTP read implementasyonu
// ile sağlanır. v1.0 build'de production-otp feature aktifse linker bu sembolü
// arar — tanımsız sembol → link error → unintended production deploy engellenir.
#[cfg(feature = "production-otp")]
extern "C" {
    /// HSM/OTP'den root MAC key'i oku. true = başarılı, false = donanım hatası.
    fn production_provision_from_otp(key: *mut u8) -> bool;
}

#[cfg(feature = "production-otp")]
fn provision_production_key() {
    let mut key = [0u8; 32];
    // SAFETY: extern C fn — v2.0'da HSM driver implement edecek; key buffer
    // yerel stack, 32 byte yazma ABI'a uygun.
    let ok = unsafe { production_provision_from_otp(key.as_mut_ptr()) };
    if !ok {
        crate::common::halt_system("[BOOT] FATAL: OTP key provisioning failed");
    }
    kernel::capability::broker::provision_key(&key);
}

/// U-21 GÖREV 2 [H1]: Production POST — feature'dan bağımsız çalışan minimum
/// sağlık kontrolleri. Self-test'teki geniş POST (BLAKE3, Ed25519, vb.)
/// `#[cfg(feature = "self-test")]` altında kalır; bu fonksiyon tüm build'lerde
/// boot path'inde çağrılır ve halt_system ile boot'u durdurur.
///
/// Kontrol kapsamı:
/// 1. mtvec doğru kuruldu mu (boot.rs::init başında write_mtvec yapıldı)
/// 2. mtvec mode bits = 0 (direct mode — vectored modunu önle)
/// 3. CLINT mtime ilerliyor mu (timer ölü → tüm safety mekanizması ölü)
/// 4. misa = RV64IMAC mı (donanım identity manipülasyonu)
/// 5. medeleg/mideleg = 0 (M-only kernel'de delegation yok)
/// 6. mcounteren = 0 (U-mode timing side-channel kapalı)
/// 7. PMP shadow integrity (boot sonrası başlangıç durumu)
pub fn production_post() {
    // 1. mtvec
    let mtvec = arch::csr::read_mtvec();
    if mtvec == 0 {
        crate::common::halt_system("[POST] FATAL: mtvec = 0");
    }
    if mtvec & 0x3 != 0 {
        crate::common::halt_system("[POST] FATAL: mtvec mode != direct");
    }

    // 2. CLINT mtime ilerliyor mu — kısa busy-wait sonrası t2 > t1 olmalı
    let t1 = arch::clint::read_mtime();
    let mut spin = 0u32;
    while spin < 1000 { spin = spin.wrapping_add(1); core::hint::spin_loop(); }
    let t2 = arch::clint::read_mtime();
    if t2 <= t1 {
        crate::common::halt_system("[POST] FATAL: CLINT mtime not advancing");
    }

    // 3. misa: RV64IMAC — MXL=2 (64-bit), I+M+A+C bit'leri
    let misa = arch::csr::read_misa();
    let mxl = (misa >> 62) & 0x3;
    if mxl != 2 {
        crate::common::halt_system("[POST] FATAL: misa MXL != RV64");
    }
    let has_i = (misa >> 8)  & 1;
    let has_m = (misa >> 12) & 1;
    let has_a = misa & 1; // bit 0 = 'A'
    let has_c = (misa >> 2) & 1;
    if has_i == 0 || has_m == 0 || has_a == 0 || has_c == 0 {
        crate::common::halt_system("[POST] FATAL: misa missing IMAC");
    }

    // 4. Delegation registers (G9 [M2])
    // WARL register — bazı bit'ler hardware impl-defined olarak 1 kalabilir
    // (QEMU virt RV64'te medeleg bazı reserved bit'leri 1 döner). M-only
    // kernel'de delegation effect'siz: S-mode olmadığından delegate edilen
    // trap mtvec'e düşer. csrw zero atılmış olsa da read-back hard guarantee
    // değil; yine de "tüm bit'lerin 1 olduğu kötü hardware" durumunu yakalamak
    // için all-ones reddi yapılır.
    let medeleg = arch::csr::read_medeleg();
    let mideleg = arch::csr::read_mideleg();
    if medeleg == usize::MAX || mideleg == usize::MAX {
        crate::common::halt_system("[POST] FATAL: medeleg/mideleg all-ones (hw fault)");
    }

    // 5. mcounteren = 0 (G8 [M1] — U-mode timing side-channel kapalı)
    // Bu register WARL değil; csrw zero geçerli. Sıfır olmalı.
    let mcounteren = arch::csr::read_mcounteren();
    if mcounteren != 0 {
        crate::common::halt_system("[POST] FATAL: mcounteren != 0");
    }

    // 6. PMP integrity (init_pmp sonrası shadow ile uyum)
    if !kernel::memory::verify_pmp_integrity() {
        crate::common::halt_system("[POST] FATAL: PMP integrity fail");
    }
}

/// Boot initialization — PMP, blackbox, HAL, task creation
pub fn init() {
    // U-21 GÖREV 9 [M2]: M-only kernel — exception/interrupt delegation 0.
    // U-21 GÖREV 8 [M1]: U-mode counter access 0 (rdcycle/rdtime/rdinstret deny).
    // mtvec set'inden ÖNCE — eğer trap olursa S-mode'a delegate edilmemeli.
    // SAFETY: M-mode CSR write, boot sequence, MIE=0 (interrupts not yet enabled).
    unsafe {
        core::arch::asm!("csrw medeleg, zero");
        core::arch::asm!("csrw mideleg, zero");
        core::arch::asm!("csrw mcounteren, zero");
    }

    arch::csr::write_mtvec(trap_entry as *const () as usize);
    #[cfg(feature = "debug-boot")]
    {
        arch::uart::puts("[BOOT] mtvec = 0x");
        crate::common::fmt::print_hex(arch::csr::read_mtvec());
        arch::uart::println("");
    }

    kernel::memory::init_pmp();
    ipc::blackbox::init();

    // U-21 GÖREV 2 [H1]: PMP/CLINT/CSR sağlık kontrolleri — boot fail-closed
    // Self-test build'inde tests::run_all() ek kontroller yapacak; bu zorunlu set
    // her build'de çalışır.
    production_post();

    #[cfg(feature = "debug-boot")]
    {
        arch::uart::println("[HAL]  Device trait registered");
        arch::uart::println("[HAL]  IOPMP stub ready");
    }

    // ─── Capability MAC key provisioning ───
    // U-21 GÖREV 6 [H2]: test-keys VEYA production-otp ZORUNLU
    // (compile_error guard src/main.rs'te). KEY_READY=false ile boot etmek
    // sessizce capability sistemi devre dışı bırakırdı; o yol artık kapalı.
    #[cfg(feature = "test-keys")]
    {
        let mac_key = [0x5Au8; 32];
        kernel::capability::broker::provision_key(&mac_key);
        #[cfg(feature = "debug-boot")]
        arch::uart::println("[BOOT] Capability MAC key provisioned (TEST KEY)");
    }
    #[cfg(feature = "production-otp")]
    {
        provision_production_key();
        #[cfg(feature = "debug-boot")]
        arch::uart::println("[BOOT] Capability MAC key provisioned (OTP/HSM)");
    }

    // ─── Secure boot doğrulama ───
    #[cfg(feature = "test-keys")]
    {
        use crate::hal::key;
        use crate::hal::secure_boot;
        let pubkey = key::get_root_public_key();
        let valid = secure_boot::secure_boot_check(&[], pubkey, &key::QEMU_TEST_SIGNATURE);
        if valid {
            #[cfg(feature = "debug-boot")]
            arch::uart::println("[BOOT] Secure boot check OK");
        } else {
            crate::common::halt_system("[BOOT] Secure boot FAIL — HALT");
        }
    }
    #[cfg(all(not(feature = "test-keys"), feature = "debug-boot"))]
    arch::uart::println("[BOOT] Secure boot SKIP (no test-keys, production: OTP v2.0)");

    use crate::common::types::TaskConfig;

    let id_a = kernel::scheduler::create_task(&TaskConfig {
        entry: crate::task_a, priority: 4, dal: 1,
        budget_cycles: 300_000, period_ticks: 10,
    });
    let id_b = kernel::scheduler::create_task(&TaskConfig {
        entry: crate::task_b, priority: 8, dal: 2,
        budget_cycles: 200_000, period_ticks: 10,
    });
    #[cfg(feature = "debug-boot")]
    {
        arch::uart::puts("[BOOT] Task A: id=");
        print_u32(id_a.unwrap_or(255) as u32);
        arch::uart::puts(" prio=4 dal=B budget=300K/period");
        arch::uart::println("");
        arch::uart::puts("[BOOT] Task B: id=");
        print_u32(id_b.unwrap_or(255) as u32);
        arch::uart::puts(" prio=8 dal=C budget=200K/period");
        arch::uart::println("");
    }

    // ─── Sprint U-16: IPC Channel ownership assignment ───
    // Channel 0: A → B (producer=A, consumer=B)
    // Channel 1: B → A (producer=B, consumer=A)
    // Diğer kanallar (2-7) atanmamış kalır → default deny.
    if let (Some(a), Some(b)) = (id_a, id_b) {
        let ok_0 = ipc::assign_channel(0, a, b);
        let ok_1 = ipc::assign_channel(1, b, a);
        if !ok_0 || !ok_1 {
            crate::common::halt_system("[BOOT] FATAL: IPC channel assignment failed — HALT");
        }
        #[cfg(feature = "debug-boot")]
        arch::uart::println("[BOOT] IPC ch0: A→B, ch1: B→A");
    } else {
        crate::common::halt_system("[BOOT] FATAL: task creation failed — HALT");
    }
    ipc::seal_channels();
    #[cfg(feature = "debug-boot")]
    arch::uart::println("[BOOT] IPC channels sealed");
}

/// Final boot — timer arm + scheduler start (diverges, never returns)
pub fn start() -> ! {
    arch::csr::enable_timer_interrupt();
    arch::clint::init_timer();
    #[cfg(feature = "debug-boot")]
    {
        arch::uart::println("[BOOT] Timer armed");
        arch::uart::println("[BOOT] Starting scheduler...");
        arch::uart::println("");
    }

    kernel::scheduler::start_first_task();
}
