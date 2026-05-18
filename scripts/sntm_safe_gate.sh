#!/usr/bin/env bash
# SAFE-4 (sprint-u33): SNTM-SAFE gate — 10/10 active, SAFE faz kapanışı.
#
# §17.10 10-gate yapısı:
#   [1/10] cargo check (her task)
#   [2/10] task-lint (Safe Native Profile uygulama)
#   [3/10] cargo +nightly build --release      ← SAFE-2 (typed IPC)
#   [4/10] riscv-bin-verify                    ← SAFE-3 (binary verifier)
#   [5/10] sntm-stack (.stack_sizes + recursion + indirect call) ← SAFE-4 Plan B
#   [6/10] sntm-validate (manifest invariants) ← PARTIAL (SAFE-1 trust_tier check eklendi)
#   [7/10] Static cap table codegen check      ← SAFE-2
#   [8/10] Typed IPC API codegen check         ← SAFE-2
#   [9/10] Task certificate ed25519 sign       ← SAFE-3
#   [10/10] Image assemble + final ed25519     ← SAFE-3
#
# SAFE-4 Plan B note: cargo-call-stack 0.1.16 current nightly (2026-03-01)
# ile uyumsuz (rustc wrapper intercept 2023-11 hard-coded). LLVM
# `-Z emit-stack-sizes` ELF section + `tools/sntm-stack/` direkt parse.
# Section 8 CR-2 doctrine.

set -eo pipefail
cd "$(dirname "$0")/.."

HOST=$(rustc -vV | sed -n 's/^host: //p')

echo "=== SNTM-SAFE GATE (SAFE-4, sprint-u33) ==="
echo "Active: [1] cargo check + [2] task-lint + [3] typed IPC build +"
echo "        [4] riscv-bin-verify + [5] sntm-stack +"
echo "        [6] sntm-validate + [7] cap_generated drift + [8] channels drift +"
echo "        [9] task certificate ed25519 sign + [10] image assemble + sig"
echo "Deferred: yok — 10/10 aktif, SAFE faz kapanışı."
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

# [4/10] riscv-bin-verify (forbidden opcode + section + region + jal CFI)
# SAFE-3 (sprint-u32): build verifier + check production task ELFs.
echo "[4/10] riscv-bin-verify..."
RBVERIFY="tools/riscv-bin-verify/target/$HOST/release/riscv-bin-verify"
if [ ! -x "$RBVERIFY" ]; then
    echo "  Building riscv-bin-verify..."
    (cd tools/riscv-bin-verify && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: riscv-bin-verify build"
        exit 1
    }
fi
echo "  [4.1] riscv-bin-verify integration tests..."
(cd tools/riscv-bin-verify && cargo +stable test --target "$HOST" --release > /tmp/rbverify-test.log 2>&1) || {
    echo "  FAIL: riscv-bin-verify integration tests"
    tail -40 /tmp/rbverify-test.log
    exit 1
}
echo "  [4.2] riscv-bin-verify real run..."
for task in task_hello task_world; do
    "$RBVERIFY" --elf "target/riscv64imac-unknown-none-elf/release/$task" \
                --manifest sipahi.toml --task-name "$task" || {
        echo "  FAIL: riscv-bin-verify($task)"
        exit 1
    }
done
echo "  PASS"
echo ""

# [5/10] SAFE-4 (sprint-u33) — Plan B: sntm-stack (.stack_sizes ELF parse +
# indirect call detect + recursion cycle detect; cargo-call-stack 0.1.16
# current nightly ile uyumsuz — Section 8 CR-2 doctrine).
echo "[5/10] sntm-stack (.stack_sizes + recursion + indirect call)..."
bash scripts/stack_analysis.sh > /tmp/safe4-stack.log 2>&1 || {
    echo "  FAIL: sntm-stack analysis (see /tmp/safe4-stack.log)"
    tail -20 /tmp/safe4-stack.log
    exit 1
}
SNTM_VALIDATE_BIN="tools/sntm-validate/target/$HOST/release/sntm-validate"
if [ ! -x "$SNTM_VALIDATE_BIN" ]; then
    echo "  Building sntm-validate..."
    (cd tools/sntm-validate && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: sntm-validate build failed"
        exit 1
    }
fi
for task in task_hello task_world; do
    REPORT="target/native/${task}.stack.txt"
    if [ ! -f "$REPORT" ]; then
        echo "  FAIL: missing $REPORT (stack_analysis.sh did not emit)"
        exit 1
    fi
    "$SNTM_VALIDATE_BIN" \
        --manifest sipahi.toml \
        --call-stack-report "$REPORT" \
        --task-name "$task" > /tmp/safe4-stack-$task.log 2>&1 || {
        echo "  FAIL: stack bound exceeded for $task (Section 8 CR-5 margin)"
        cat /tmp/safe4-stack-$task.log
        exit 1
    }
done
echo "  PASS"
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

# [9/10] task certificate ed25519 sign + sign+verify roundtrip drift guard
# SAFE-3 (sprint-u32, Section 8 CR-6 + CR-7): cert artifact ephemeral (NOT
# git diff drift); roundtrip verify yeterli.
echo "[9/10] task certificate ed25519 sign..."
CERTGEN="tools/sntm-cert-gen/target/$HOST/release/sntm-cert-gen"
if [ ! -x "$CERTGEN" ]; then
    echo "  Building sntm-cert-gen..."
    (cd tools/sntm-cert-gen && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: sntm-cert-gen build"
        exit 1
    }
fi
echo "  [9.1] sntm-cert-gen integration tests (RFC 8032 + tamper)..."
(cd tools/sntm-cert-gen && cargo +stable test --target "$HOST" --release > /tmp/certgen-test.log 2>&1) || {
    echo "  FAIL: sntm-cert-gen integration tests"
    tail -40 /tmp/certgen-test.log
    exit 1
}
echo "  [9.2] ephemeral keypair bootstrap..."
bash scripts/gen_dev_key.sh > /tmp/devkey.log 2>&1 || {
    echo "  FAIL: gen_dev_key"
    tail -10 /tmp/devkey.log
    exit 1
}
echo "  [9.3] task_hello + task_world cert generate..."
# SAFE-4 (sprint-u33) Section 8 CR-4 doctrine: cert max_stack_bytes refinement
# zorunlu — --call-stack-report VERILMEZSE cert UNKNOWN_SENTINEL ile çıkar
# (DAL audit reject). Bu gate'te stack report dosyası mevcut olmalı + hata
# durumunda exit. [5/10] gate'te stack_analysis.sh çoktan çalışmış; yine de
# defansif olarak existence check.
for task in task_hello task_world; do
    case "$task" in
        task_hello) tid=2 ;;
        task_world) tid=3 ;;
    esac
    STACK_REPORT="target/native/${task}.stack.txt"
    if [ ! -s "$STACK_REPORT" ]; then
        echo "  FAIL: stack report missing for ${task} ($STACK_REPORT) — "
        echo "        scripts/stack_analysis.sh önce çalışmalı (Section 8 CR-4)"
        exit 1
    fi
    "$CERTGEN" \
        --manifest sipahi.toml --task-name "$task" --task-id "$tid" \
        --text-bin   "target/native/${task}.text.bin" \
        --rodata-bin "target/native/${task}.rodata.bin" \
        --data-bin   "target/native/${task}.data.bin" \
        --signing-key keys/dev-image.priv \
        --out-cert   "target/native/${task}.cert.bin" \
        --out-sig    "target/native/${task}.cert.sig" \
        --call-stack-report "$STACK_REPORT" \
        > /tmp/certgen-${task}.log 2>&1 || {
        echo "  FAIL: cert generate ${task}"
        tail -10 /tmp/certgen-${task}.log
        exit 1
    }
    # CR-4: UNKNOWN sentinel cert'e sızmasın. Cert binary'sini parse edip
    # max_stack_bytes alanını oku (offset 248, u32 LE). 0xFFFF_FFFF ⇒ FAIL.
    MS_HEX=$(xxd -s 248 -l 4 -p "target/native/${task}.cert.bin")
    if [ "$MS_HEX" = "ffffffff" ]; then
        echo "  FAIL: cert max_stack_bytes UNKNOWN sentinel (CR-4 doctrine — "
        echo "        stack report parse veya cert-gen pipeline kırık)"
        exit 1
    fi
done
echo "  PASS"
echo ""

# [10/10] image assemble + final ed25519 + roundtrip verify
echo "[10/10] image assemble + final ed25519..."
SNTM_IMG="tools/sntm-image/target/$HOST/release/sntm-image"
if [ ! -x "$SNTM_IMG" ]; then
    echo "  Building sntm-image..."
    (cd tools/sntm-image && cargo +stable build --release --target "$HOST" > /dev/null 2>&1) || {
        echo "  FAIL: sntm-image build"
        exit 1
    }
fi
echo "  [10.1] sntm-image integration tests (roundtrip + tamper)..."
(cd tools/sntm-image && cargo +stable test --target "$HOST" --release > /tmp/sntmimg-test.log 2>&1) || {
    echo "  FAIL: sntm-image integration tests"
    tail -40 /tmp/sntmimg-test.log
    exit 1
}
echo "  [10.2] assemble + sign image..."
"$SNTM_IMG" \
    --manifest sipahi.toml \
    --kernel target/riscv64imac-unknown-none-elf/release/sipahi \
    --task task_hello target/native/task_hello \
    --task task_world target/native/task_world \
    --signing-key keys/dev-image.priv \
    --output target/sipahi-image.bin \
    > /tmp/image-assemble.log 2>&1 || {
    echo "  FAIL: image assemble"
    tail -10 /tmp/image-assemble.log
    exit 1
}
echo "  [10.3] verify image roundtrip..."
"$SNTM_IMG" --verify target/sipahi-image.bin --pubkey keys/dev-image.pub > /tmp/image-verify.log 2>&1 || {
    echo "  FAIL: image verify"
    tail -10 /tmp/image-verify.log
    exit 1
}
echo "  PASS"
echo ""

echo "=== SAFE-4 GATE PASS (10/10 active, DEFER YOK — SAFE faz kapandı) ==="
echo "Active gates: [1] cargo check + [2] task-lint + [3] typed IPC build +"
echo "              [4] riscv-bin-verify + [5] sntm-stack (Plan B) +"
echo "              [6] sntm-validate + [7] cap_generated drift + [8] channels drift +"
echo "              [9] task cert ed25519 sign + [10] image assemble + sig"
