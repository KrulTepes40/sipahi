# Sipahi Microkernel — Build & Test
TARGET = riscv64imac-unknown-none-elf
KERNEL = target/$(TARGET)/release/sipahi
KERNEL_DBG = target/$(TARGET)/debug/sipahi

# build-std burada — config.toml'da değil (Kani çakışması önlenir)
BUILD_STD = -Z build-std=core -Z build-std-features=compiler-builtins-mem

.PHONY: build run clean check kani debug

# Derle
build:
	cargo build --release $(BUILD_STD)

# QEMU'da çalıştır (Ctrl+A sonra X ile çık)
run: build
	qemu-system-riscv64 \
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
