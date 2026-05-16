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
(cd tools/sntm-validate && cargo run --target "$HOST" --release -- \
    --manifest ../../sipahi.toml \
    --output-rs ../../src/kernel/pmp/generated.rs)
echo "[regen] done."
