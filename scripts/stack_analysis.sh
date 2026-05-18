#!/usr/bin/env bash
# SAFE-4 (sprint-u33) Section 8 CR-2 Plan B: cargo-call-stack 0.1.16 current
# nightly ile uyumsuz (rustc wrapper intercept 2023-11 hard-coded). Yerine
# LLVM `-Z emit-stack-sizes` ELF section üretilir, host tool `tools/sntm-stack/`
# parse eder (sub-workspace pattern, object crate).
#
# Section 8 CR-8 doctrine: kernel RUSTFLAGS host tool subshell'e SIZAR.
# `env -u RUSTFLAGS` ile temizle, sonra emit-stack-sizes flag'i bizden gelir.
#
# Çıktı: target/native/<task>.stack.txt — sntm-stack rapor format. Tüketici:
# - sntm-validate --call-stack-report (G2)
# - sntm-cert-gen --call-stack-report (G3)
# - sntm_safe_gate.sh [5/10] (G4)

set -eo pipefail
cd "$(dirname "$0")/.."

OUT_DIR="target/native"
mkdir -p "$OUT_DIR"

# Build sntm-stack host tool (idempotent) before invoking.
HOST=$(rustc -vV | sed -n 's/^host: //p')
echo "[sntm-stack] build host: $HOST"
(cd tools/sntm-stack && env -u RUSTFLAGS \
    cargo +stable build --release --target "$HOST" 2>&1 | tail -3)
SNTM_STACK_BIN="tools/sntm-stack/target/$HOST/release/sntm-stack"
if [ ! -x "$SNTM_STACK_BIN" ]; then
    echo "FAIL: sntm-stack binary missing: $SNTM_STACK_BIN"
    exit 1
fi

for task in task_hello task_world; do
    echo "[stack] $task — build with -Z emit-stack-sizes"
    # SAFE-4 CR-8: temizle, sonra emit-stack-sizes ekle.
    # Pinned toolchain (rust-toolchain.toml) — +flag YOK; default kullan.
    (cd "tasks/$task" && env -u RUSTFLAGS RUSTFLAGS="-Z emit-stack-sizes" \
        cargo build --release 2>&1 | tail -3)

    ELF="target/riscv64imac-unknown-none-elf/release/$task"
    OUT="$OUT_DIR/$task.stack.txt"
    if [ ! -f "$ELF" ]; then
        echo "FAIL: ELF missing: $ELF"
        exit 1
    fi
    echo "[stack] $task — analyze ELF → $OUT"
    "$SNTM_STACK_BIN" --bin "$ELF" --output "$OUT"

    # Doctrine: stdout her zaman göster (CI log için).
    grep -E "^(status|max_stack_bytes|reason):" "$OUT" || true
done

echo "[stack] done. Reports: $OUT_DIR/*.stack.txt"
