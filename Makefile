# Sipahi Microkernel — Build & Test
TARGET = riscv64imac-unknown-none-elf
KERNEL = target/$(TARGET)/release/sipahi
KERNEL_DBG = target/$(TARGET)/debug/sipahi
QEMU = qemu-system-riscv64

# build-std burada — config.toml'da değil (Kani çakışması önlenir)
BUILD_STD = -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem

# U-23: Kernel linker script `.cargo/config.toml`'dan Makefile'a taşındı.
# Sebep: rustflags child dir'lere `union` merge ile sızıyordu (tasks/task_hello
# build'inde sipahi.ld bulunamıyordu). Sadece kernel build'lerinde aktif.
KERNEL_RUSTFLAGS = -C link-arg=-Tsipahi.ld

.PHONY: build run clean check kani debug run-self-test regen-pmp build-native run-cross-isolation

# U-26 SNTM Phase 4 FIX-B: task_hello ELF → embed binaries.
# build/check/run-self-test/debug HEPSI build-native'a depend.
# Aksi halde clean clone'da include_bytes! target/native/*.bin yok → fail.
build-native:
	bash scripts/build_native_tasks.sh

# Production binary — test/POST kodu YOK, minimal attack surface
build: build-native
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release $(BUILD_STD)

# Production binary'i QEMU'da çalıştır (boot → scheduler, test yok)
run: build
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-m 512M \
		-smp 1 \
		-kernel $(KERNEL)

# Sprint U-16: Self-test build — POST + integration + FI suite aktif.
# CI ve geliştirme için. Production'da KAPALI.
run-self-test: build-native
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release --features self-test $(BUILD_STD)
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-m 512M \
		-smp 1 \
		-kernel $(KERNEL)

# U-27.5: Cross-task PMP runtime ihlal demo (opt-in, default OFF).
# Codex fix: kernel `self-test` feature DAHIL EDILMEZ — tests::run_all
# scheduler START öncesi çalışır, runtime gözlem için uygun değil. Sadece
# `cross-isolation-demo + debug-boot` features (TICK markerları script Gate 3
# için zorunlu). SIPAHI_CROSS_ISOLATION=1 ile task_hello deliberate write
# build-time propagate edilir. QEMU log /tmp/u275_xi.log'a yazılır,
# scripts/check_cross_isolation.sh ile 4-gate doğrulanır.
# U-29 fix: timeout 30 → 60 (QEMU TCG yavaş, marker sonrası 3+ TICK için
# yeterli süre lazım — flaky Gate 3 fail önlenir).
run-cross-isolation:
	SIPAHI_CROSS_ISOLATION=1 bash scripts/build_native_tasks.sh
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release \
		--features cross-isolation-demo,debug-boot $(BUILD_STD)
	timeout 60 $(QEMU) \
		-machine virt -nographic -bios none -m 512M -smp 1 \
		-kernel $(KERNEL) 2>&1 | tee /tmp/u275_xi.log || true
	bash scripts/check_cross_isolation.sh /tmp/u275_xi.log

# Debug modda çalıştır (GDB bağlantısı için bekler)
debug: build-native
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build $(BUILD_STD)
	qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios none \
		-m 512M \
		-smp 1 \
		-kernel $(KERNEL_DBG) \
		-s -S

# Lint + clippy
check: build-native
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo clippy $(BUILD_STD) -- -D warnings

# Kani formal verification (build-std OLMADAN — Kani kendi core'unu kullanır)
# U-21 GÖREV 14 [M13]: --all-harnesses Kani 0.67+ ile unsupported flag.
# `cargo kani` (flag'siz) tüm harness'leri çalıştırır (CI ile align).
kani:
	cargo kani

# U-25 SNTM Phase 3: manifest → src/kernel/pmp/generated.rs codegen.
# sipahi.toml değiştiğinde manuel çalıştır + commit. CI drift gate'i
# bunu çalıştırıp git diff'i kontrol eder.
regen-pmp:
	bash scripts/regen_pmp_profiles.sh

# Temizle
clean:
	cargo clean
