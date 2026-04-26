# Sipahi Microkernel — Build & Test
TARGET = riscv64imac-unknown-none-elf
KERNEL = target/$(TARGET)/release/sipahi
KERNEL_DBG = target/$(TARGET)/debug/sipahi
QEMU = qemu-system-riscv64

# build-std burada — config.toml'da değil (Kani çakışması önlenir)
BUILD_STD = -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem

.PHONY: build run clean check kani debug run-self-test

# Production binary — test/POST kodu YOK, minimal attack surface
build:
	cargo build --release $(BUILD_STD)

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
run-self-test:
	cargo build --release --features self-test $(BUILD_STD)
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-m 512M \
		-smp 1 \
		-kernel $(KERNEL)

# Debug modda çalıştır (GDB bağlantısı için bekler)
debug:
	cargo build $(BUILD_STD)
	qemu-system-riscv64 \
		-machine virt \
		-nographic \
		-bios none \
		-m 512M \
		-smp 1 \
		-kernel $(KERNEL_DBG) \
		-s -S

# Lint + clippy
check:
	cargo clippy $(BUILD_STD) -- -D warnings

# Kani formal verification (build-std OLMADAN — Kani kendi core'unu kullanır)
kani:
	cargo kani --all-harnesses

# Temizle
clean:
	cargo clean
