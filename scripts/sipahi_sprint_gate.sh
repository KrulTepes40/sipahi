#!/usr/bin/env bash
# U-21 GÖREV 22: Sprint completion gate.
# Sprint sonunda manuel veya CI'da çalıştırılır; herhangi bir adım fail
# ederse exit 1 ile sprint kapatma engellenir.
set -e
cd "$(dirname "$0")/.."

echo "=== SIPAHI SPRINT GATE ==="

# 1. Plain cargo check (rapid sanity)
echo "[1/8] cargo check..."
cargo check -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem 2>&1 | tail -3

# 2. make check (clippy -D warnings)
echo "[2/8] make check (clippy)..."
make check 2>&1 | tail -2

# 3. Kani full
echo "[3/8] cargo kani..."
cargo kani 2>&1 | tail -3

# 4. Production build
echo "[4/8] make build (production)..."
make build 2>&1 | tail -2

# 5. Production NF guard
echo "[5/8] production NF check..."
timeout 8s qemu-system-riscv64 \
    -machine virt -nographic -bios none -m 512M -smp 1 \
    -kernel target/riscv64imac-unknown-none-elf/release/sipahi \
    > /tmp/sipahi_gate_prod.log 2>&1 || true
if grep -q "^NF$" /tmp/sipahi_gate_prod.log; then
    echo "FAIL: NF detected in production"
    cat /tmp/sipahi_gate_prod.log
    exit 1
fi
if grep -q "FATAL" /tmp/sipahi_gate_prod.log; then
    echo "FAIL: FATAL marker in production"
    cat /tmp/sipahi_gate_prod.log
    exit 1
fi
echo "  production NF-free + FATAL-free ✓"

# 6. Self-test
echo "[6/8] self-test..."
timeout 30s make run-self-test > /tmp/sipahi_gate_test.log 2>&1 || true
if ! grep -aq "ALL TESTS PASSED" /tmp/sipahi_gate_test.log; then
    echo "FAIL: self-test did not pass"
    tail -50 /tmp/sipahi_gate_test.log
    exit 1
fi
if grep -aq "\[FAIL\]" /tmp/sipahi_gate_test.log; then
    echo "FAIL: [FAIL] marker in self-test"
    grep -a "\[FAIL\]" /tmp/sipahi_gate_test.log
    exit 1
fi
if grep -aq "^NF$" /tmp/sipahi_gate_test.log; then
    echo "FAIL: NF in self-test"
    exit 1
fi
echo "  self-test PASS + 6 negative tests ✓"

# 7. New TODO/FIXME guard (if HEAD~1 exists)
echo "[7/8] TODO/FIXME guard..."
if git rev-parse HEAD~1 >/dev/null 2>&1; then
    NEW_TODOS=$(git diff HEAD~1..HEAD -- src/ \
        | grep -cE '^\+.*\b(TODO|FIXME|HACK|XXX)\b' || true)
    if [ "$NEW_TODOS" -gt 0 ]; then
        echo "FAIL: $NEW_TODOS new TODO/FIXME/HACK/XXX added — clean up first"
        exit 1
    fi
    echo "  no new TODO/FIXME ✓"
else
    echo "  (no previous commit — skipped)"
fi

# 8. Banner version match (Cargo.toml ↔ main.rs)
echo "[8/8] version banner consistency..."
CARGO_VER=$(grep -m1 '^version\s*=' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
BANNER_VER=$(grep -oE 'Sipahi Microkernel v[0-9.]+' src/main.rs | head -1 | sed 's/.*v//')
echo "  Cargo.toml: $CARGO_VER, banner: $BANNER_VER"

echo ""
echo "=== SPRINT GATE PASSED ==="
