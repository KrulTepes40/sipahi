#!/usr/bin/env bash
# SAFE-2 (sprint-u31): regenerate cap_generated.rs + channels.rs from sipahi.toml.
#
# Pattern mirrors scripts/regen_pmp_profiles.sh — host-tool rebuild + emit.
# Drift guard: sntm_safe_gate.sh [7/10] + [8/10] runs this + git diff --exit-code.

set -eo pipefail
cd "$(dirname "$0")/.."

HOST=$(rustc -vV | sed -n 's/^host: //p')

echo "=== Regenerating SAFE-2 codegen artifacts ==="

# Build sntm-validate (cargo +stable to bypass nightly serde_core ICE).
SNTM_BIN="tools/sntm-validate/target/$HOST/release/sntm-validate"
if [ ! -x "$SNTM_BIN" ]; then
    echo "[build] sntm-validate (release, $HOST)..."
    (cd tools/sntm-validate && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: sntm-validate build"
        exit 1
    }
fi

# 1. cap_generated.rs — LOCAL_CAP_TABLE + BOOT_CHANNELS.
echo "[regen] src/kernel/capability/cap_generated.rs"
"$SNTM_BIN" --manifest sipahi.toml \
    --output-cap-table src/kernel/capability/cap_generated.rs

# 2. channels.rs — typed IPC API for sipahi_api.
echo "[regen] sipahi_api/src/channels.rs"
"$SNTM_BIN" --manifest sipahi.toml \
    --output-channels sipahi_api/src/channels.rs

echo "=== PASS: cap_generated.rs + channels.rs regenerated ==="
