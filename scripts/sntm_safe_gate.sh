#!/usr/bin/env bash
# SAFE-1 (U-30): SNTM-SAFE gate (scaffold).
#
# §17.10 10-gate yapısı — SAFE-1'de [1/10] + [2/10] aktif, [3-10] SAFE-2..4'te:
#   [1/10] cargo check (her task)
#   [2/10] task-lint (Safe Native Profile uygulama)
#   [3/10] cargo +nightly build --release      ← SAFE-2 (typed IPC)
#   [4/10] riscv-bin-verify                    ← SAFE-3 (binary verifier)
#   [5/10] cargo-call-stack                    ← SAFE-4 (stack analyzer)
#   [6/10] sntm-validate (manifest invariants) ← PARTIAL (SAFE-1 trust_tier check eklendi)
#   [7/10] Static cap table codegen check      ← SAFE-2
#   [8/10] Typed IPC API codegen check         ← SAFE-2
#   [9/10] Task certificate ed25519 sign       ← SAFE-3
#   [10/10] Image assemble + final ed25519     ← SAFE-3

set -eo pipefail
cd "$(dirname "$0")/.."

HOST=$(rustc -vV | sed -n 's/^host: //p')

echo "=== SNTM-SAFE GATE (SAFE-1, scaffold) ==="
echo "Active: [1/10] cargo check + [2/10] task-lint + [6/10] sntm-validate (partial)"
echo "Deferred: [3-5, 7-10] SAFE-2..4'te"
echo ""

# [1/10] cargo check (her task)
echo "[1/10] cargo check..."
(cd tasks/task_hello && cargo check --release 2>&1 | tail -2)
(cd tasks/task_world && cargo check --release 2>&1 | tail -2)
echo "  PASS"
echo ""

# [2/10] task-lint (Safe Native Profile uygulama)
echo "[2/10] task-lint (Safe Native Profile)..."
TASK_LINT_BIN="tools/task-lint/target/$HOST/release/task-lint"
if [ ! -x "$TASK_LINT_BIN" ]; then
    echo "  Building task-lint..."
    (cd tools/task-lint && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: task-lint build failed"
        exit 1
    }
fi

# U-30.1: task-lint integration tests (18 fixture) BEFORE real run.
# Drift attack senaryosu: birisi lint kod yolunu kırarsa, integration test
# yakalar — REAL run'da false PASS dönmeden önce.
echo "  [2.1] task-lint integration tests (cargo test)..."
(cd tools/task-lint && cargo +stable test --target "$HOST" --release > /tmp/task-lint-test.log 2>&1) || {
    echo "  FAIL: task-lint integration tests failed"
    tail -40 /tmp/task-lint-test.log
    exit 1
}
echo "  [2.2] task-lint real run..."
"$TASK_LINT_BIN" --manifest sipahi.toml --tasks-dir tasks/
echo "  PASS"
echo ""

# U-30.1: sntm-validate integration tests (orphan/default-ON/missing).
echo "[2.5/10] sntm-validate integration tests (cargo test)..."
(cd tools/sntm-validate && cargo +stable test --target "$HOST" --release > /tmp/sntm-validate-test.log 2>&1) || {
    echo "  FAIL: sntm-validate integration tests failed"
    tail -40 /tmp/sntm-validate-test.log
    exit 1
}
echo "  PASS"
echo ""

# [3/10] DEFER SAFE-2 — cargo +nightly typed IPC codegen build
echo "[3/10] cargo +nightly build (typed IPC) — DEFER SAFE-2"
echo ""

# [4/10] DEFER SAFE-3 — riscv-bin-verify (forbidden opcode + section + relocation)
echo "[4/10] riscv-bin-verify — DEFER SAFE-3"
echo ""

# [5/10] DEFER SAFE-4 — cargo-call-stack (stack bound + recursion)
echo "[5/10] cargo-call-stack — DEFER SAFE-4"
echo ""

# [6/10] sntm-validate (manifest invariants — partial: SAFE-1 trust_tier check eklendi)
echo "[6/10] sntm-validate (manifest invariants, SAFE-1 trust_tier extension)..."
SNTM_VALIDATE_BIN="tools/sntm-validate/target/$HOST/release/sntm-validate"
if [ ! -x "$SNTM_VALIDATE_BIN" ]; then
    echo "  Building sntm-validate..."
    (cd tools/sntm-validate && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: sntm-validate build failed"
        exit 1
    }
fi
"$SNTM_VALIDATE_BIN" --manifest sipahi.toml > /dev/null
echo "  PASS"
echo ""

# [7/10] DEFER SAFE-2 — static cap table codegen
echo "[7/10] static cap table codegen — DEFER SAFE-2"
echo ""

# [8/10] DEFER SAFE-2 — typed IPC API codegen
echo "[8/10] typed IPC API codegen — DEFER SAFE-2"
echo ""

# [9/10] DEFER SAFE-3 — task certificate ed25519 sign
echo "[9/10] task certificate ed25519 sign — DEFER SAFE-3"
echo ""

# [10/10] DEFER SAFE-3 — image assemble + final ed25519
echo "[10/10] image assemble + final ed25519 — DEFER SAFE-3"
echo ""

echo "=== SAFE-1 SCAFFOLD PASS ==="
echo "Active gates: [1] cargo check + [2] task-lint + [6] sntm-validate"
echo "Deferred gates: [3, 4, 5, 7, 8, 9, 10] = 7 gate"
