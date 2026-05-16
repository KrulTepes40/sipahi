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

echo "[native] task_hello build (RISC-V)"
(cd tasks/task_hello && cargo build --release 2>&1 | tail -3)

HOST=$(rustc -vV | sed -n 's/^host: //p')
echo "[native] sntm-pack target host: $HOST"
(cd tools/sntm-pack && cargo run --target "$HOST" --release -- \
    --elf       ../../target/riscv64imac-unknown-none-elf/release/task_hello \
    --out-text   ../../$OUT_DIR/task_hello.text.bin \
    --out-rodata ../../$OUT_DIR/task_hello.rodata.bin \
    --out-data   ../../$OUT_DIR/task_hello.data.bin)

echo "[native] output:"
ls -la "$OUT_DIR/" | tail -5
echo "[native] done."
