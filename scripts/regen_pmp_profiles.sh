#!/usr/bin/env bash
# U-25 SNTM Phase 3: sipahi.toml → src/kernel/pmp/generated.rs codegen.
#
# Sprint owner manuel çalıştırır manifest değiştiğinde.
# CI drift gate (sntm-validate-drift job) bunu çalıştırıp git diff'i
# kontrol eder — divergence = FAIL.
set -eo pipefail
cd "$(dirname "$0")/.."

HOST=$(rustc -vV | sed -n 's/^host: //p')
echo "[regen] target host: $HOST"
echo "[regen] sntm-validate --manifest sipahi.toml --output-rs src/kernel/pmp/generated.rs"
# SAFE-1 (U-30): cargo +stable kanal (nightly serde_core ICE bypass).
# 1.89.0 stable serde 1.0.228 + toml 1.1.2 ile uyumlu çalışır.
(cd tools/sntm-validate && cargo +stable run --target "$HOST" --release -- \
    --manifest ../../sipahi.toml \
    --output-rs ../../src/kernel/pmp/generated.rs)
echo "[regen] done."
