#!/usr/bin/env bash
# U-26 SNTM Phase 4: task_hello ELF → per-section .bin + kernel embed.
#
# FIX-B: kernel build (cargo build / clippy / qemu) ÖNCESİ MUTLAKA çalıştırılır.
# include_bytes!("../../../target/native/task_hello.*.bin") clean clone'da
# dosyalar yok → cargo build fail. Bu script bu boşluğu doldurur:
#   1. cargo build -p task_hello (release)
#   2. sntm-pack (host target) ELF → 3 .bin (text/rodata/data)
#
# Makefile: build/check/run-self-test/debug → build-native depend.
# CI: her kernel job'ında bu script cargo'dan ÖNCE çalışır.
set -eo pipefail
cd "$(dirname "$0")/.."

OUT_DIR="target/native"
mkdir -p "$OUT_DIR"

# U-27.5: SIPAHI_CROSS_ISOLATION=1 → task_hello cross-isolation-demo feature
# ile build edilir (deliberate write to 0x80705000). Default unset → production.
TASK_HELLO_FEATURES=""
if [ "${SIPAHI_CROSS_ISOLATION:-0}" = "1" ]; then
    TASK_HELLO_FEATURES="--features cross-isolation-demo"
    echo "[native] task_hello build (RISC-V) — cross-isolation-demo ENABLED"
else
    echo "[native] task_hello build (RISC-V)"
fi
(cd tasks/task_hello && cargo build --release $TASK_HELLO_FEATURES 2>&1 | tail -3)

# U-27 SNTM Phase 5: task_world ikinci native task.
echo "[native] task_world build (RISC-V)"
(cd tasks/task_world && cargo build --release 2>&1 | tail -3)

HOST=$(rustc -vV | sed -n 's/^host: //p')
echo "[native] sntm-pack target host: $HOST"
# CI fix: RUSTFLAGS env (set at job level to "-C link-arg=-Tsipahi.ld" for the
# kernel build) leaks into sntm-pack's HOST compilation → cc rejects the
# kernel linker script. Strip it for host-tool invocations; the .cargo/config
# union-merge issue was the original reason RUSTFLAGS lives in env, but host
# builds must stay clean. `env -u RUSTFLAGS` unsets it just for this subshell.
(cd tools/sntm-pack && env -u RUSTFLAGS cargo run --target "$HOST" --release -- \
    --elf       ../../target/riscv64imac-unknown-none-elf/release/task_hello \
    --out-text   ../../$OUT_DIR/task_hello.text.bin \
    --out-rodata ../../$OUT_DIR/task_hello.rodata.bin \
    --out-data   ../../$OUT_DIR/task_hello.data.bin)

(cd tools/sntm-pack && env -u RUSTFLAGS cargo run --target "$HOST" --release -- \
    --elf       ../../target/riscv64imac-unknown-none-elf/release/task_world \
    --out-text   ../../$OUT_DIR/task_world.text.bin \
    --out-rodata ../../$OUT_DIR/task_world.rodata.bin \
    --out-data   ../../$OUT_DIR/task_world.data.bin)

echo "[native] output:"
ls -la "$OUT_DIR/" | tail -10
echo "[native] done."
