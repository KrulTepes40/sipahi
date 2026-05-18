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

echo "=== SNTM-SAFE GATE (SAFE-2, sprint-u31) ==="
echo "Active: [1] cargo check + [2] task-lint + [3] typed IPC build + [6] sntm-validate +"
echo "        [7] cap_generated drift + [8] channels drift"
echo "Deferred: [4] riscv-bin-verify (SAFE-3), [5] cargo-call-stack (SAFE-4),"
echo "          [9] task cert sign (SAFE-3), [10] image assemble + sig (SAFE-3)"
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

# [3/10] cargo +nightly build (typed IPC codegen build sanity)
# SAFE-2 (sprint-u31): sipahi_api channels.rs + tasks must compile after
# regen with both task features enabled (covers all cfg-gated wrappers).
echo "[3/10] cargo +nightly build (typed IPC)..."
(cd sipahi_api && cargo build --release --target riscv64imac-unknown-none-elf \
    -Z build-std=core --features task_task_hello,task_task_world \
    > /tmp/safe2-sipahi-api.log 2>&1) || {
    echo "  FAIL: sipahi_api typed IPC build failed"
    tail -20 /tmp/safe2-sipahi-api.log
    exit 1
}
echo "  PASS"
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

# [7/10] static cap table codegen — manifest → cap_generated.rs drift guard
# SAFE-2 (sprint-u31, CR-5): regenerate + git diff = empty.
echo "[7/10] static cap table codegen drift guard..."
bash scripts/regen_safe_codegen.sh > /tmp/safe2-regen.log 2>&1 || {
    echo "  FAIL: regen_safe_codegen.sh"
    tail -20 /tmp/safe2-regen.log
    exit 1
}
if ! git diff --quiet src/kernel/capability/cap_generated.rs; then
    echo "  FAIL: cap_generated.rs drift detected — manifest and codegen diverged"
    git --no-pager diff src/kernel/capability/cap_generated.rs | head -40
    exit 1
fi
echo "  PASS"
echo ""

# [8/10] typed IPC API codegen — channels.rs drift guard
echo "[8/10] typed IPC API codegen drift guard..."
if ! git diff --quiet sipahi_api/src/channels.rs; then
    echo "  FAIL: channels.rs drift detected — manifest and codegen diverged"
    git --no-pager diff sipahi_api/src/channels.rs | head -40
    exit 1
fi
echo "  PASS"
echo ""

# [9/10] DEFER SAFE-3 — task certificate ed25519 sign
echo "[9/10] task certificate ed25519 sign — DEFER SAFE-3"
echo ""

# [10/10] DEFER SAFE-3 — image assemble + final ed25519
echo "[10/10] image assemble + final ed25519 — DEFER SAFE-3"
echo ""

echo "=== SAFE-2 GATE PASS (scaffold + SAFE-2 active) ==="
echo "Active gates: [1] cargo check + [2] task-lint + [3] typed IPC build +"
echo "              [6] sntm-validate + [7] cap_generated drift + [8] channels drift"
echo "Deferred gates: [4, 5, 9, 10] = 4 gate (SAFE-3/4)"
