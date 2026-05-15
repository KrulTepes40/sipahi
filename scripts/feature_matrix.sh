#!/usr/bin/env bash
# U-22 GÖREV 27: Sipahi feature combination build matrix.
# G5 (v2-hal), G6 (production-otp), G8 (wasm-sandbox), G25 (entropy check)
# sonrası feature kombinasyonları breakage riski oluşturur. Her geçerli
# kombinasyon CI'da derlenebilir olmalı.
set -eo pipefail
cd "$(dirname "$0")/.."

echo "=== SIPAHI FEATURE MATRIX ==="
echo ""

# Geçerli kombinasyonlar:
# NOT: `production-otp` listede YOK — bu feature production deployer'ın harici
# olarak sağlaması gereken `production_provision_from_otp` extern fn'ini
# referans verir; lokal stub yok -> link error tasarımsal (deploy-time wired).
# CI kapsamında sadece BUILD edilebilen kombinasyonlar test edilir.
COMBOS=(
    "fast-crypto,fast-sign,test-keys"
    "fast-crypto,fast-sign,test-keys,trace"
    "fast-crypto,fast-sign,test-keys,debug-boot"
    "fast-crypto,fast-sign,test-keys,trace,debug-boot"
    "fast-crypto,fast-sign,self-test"
    "fast-crypto,fast-sign,test-keys,wasm-sandbox"
    "fast-crypto,fast-sign,test-keys,v2-hal"
    "fast-crypto,fast-sign,test-keys,wasm-sandbox,v2-hal"
    "fast-crypto,fast-sign,test-keys,sntm"           # U-23 SNTM Phase 1
    "fast-crypto,fast-sign,self-test,sntm"           # U-23 SNTM Phase 1
)

PASS=0
FAIL=0
FAIL_LIST=()

for features in "${COMBOS[@]}"; do
    echo "Building: $features"
    LOG=$(mktemp)
    # U-23: KERNEL_RUSTFLAGS Makefile'dan taşındı (sipahi.ld linker script).
    # feature_matrix kernel build de aynı linker arg'ini geçirmeli.
    if RUSTFLAGS="-C link-arg=-Tsipahi.ld" cargo build --release \
        --no-default-features --features "$features" \
        -Z build-std=core,alloc \
        -Z build-std-features=compiler-builtins-mem \
        --target riscv64imac-unknown-none-elf > "$LOG" 2>&1; then
        PASS=$((PASS+1))
        tail -1 "$LOG"
        echo "  PASS"
    else
        FAIL=$((FAIL+1))
        FAIL_LIST+=("$features")
        tail -10 "$LOG"
        echo "  FAIL"
    fi
    rm -f "$LOG"
    echo ""
done

echo "=== RESULTS ==="
echo "PASS: $PASS / ${#COMBOS[@]}"
echo "FAIL: $FAIL"
if [ "$FAIL" -gt 0 ]; then
    echo "Failed combinations:"
    for f in "${FAIL_LIST[@]}"; do
        echo "  - $f"
    done
    exit 1
fi
echo "=== ALL FEATURE COMBOS BUILD SUCCESSFULLY ==="
