#!/bin/bash
# ct_eq_16 constant-time doğrulama — audit artifact
# Sprint U-14: Manuel audit tool, CI'ya bağlanmıyor.
# LTO ile inline edilmiş olabilir — call-site artifact topluyor.

set -euo pipefail

BINARY="target/riscv64imac-unknown-none-elf/release/sipahi"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found — run 'make build' first"
    exit 1
fi

# cargo-binutils kontrolü
if ! command -v cargo-objdump &> /dev/null; then
    echo "INSTALL: cargo install cargo-binutils && rustup component add llvm-tools"
    exit 1
fi

# Sembol ara
SYMBOL=$(cargo nm --release 2>/dev/null | grep "ct_eq_16" | awk '{print $3}' || true)

if [ -z "$SYMBOL" ]; then
    echo "INFO: ct_eq_16 inlined by LTO — extracting call-site disassembly"
    # validate_cached/validate_full içinde inline olmuş olabilir
    cargo objdump --release -- --disassemble 2>/dev/null \
        | grep -B5 -A20 "xor.*or\|ct_eq\|validate_cached" \
        > scripts/ct-eq-call-sites.txt 2>/dev/null || true
    echo "Call-site disassembly saved: scripts/ct-eq-call-sites.txt"
    echo "Manual review needed for branch-free verification"
    exit 0
fi

# Sembol bulundu — branch count kontrol
DISASM=$(cargo objdump --release -- --disassemble-symbols="$SYMBOL" 2>/dev/null)
BRANCH_COUNT=$(echo "$DISASM" | grep -cE "beq|bne|blt|bge|jal |jalr" || true)

if [ "$BRANCH_COUNT" -gt 1 ]; then
    echo "FAIL: ct_eq_16 has $BRANCH_COUNT branch instructions"
    echo "$DISASM"
    exit 1
fi

echo "OK: ct_eq_16 branch-free (or single ret)"
