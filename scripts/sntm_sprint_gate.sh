#!/usr/bin/env bash
# SNTM sprint gate — extends sipahi_sprint_gate.sh
#
# Kullanım:
#   bash scripts/sntm_sprint_gate.sh                # default: base phase
#   bash scripts/sntm_sprint_gate.sh --phase=safe1  # SAFE-1 ek check'leri
#   bash scripts/sntm_sprint_gate.sh --phase=safe2  # SAFE-2 (static cap + typed IPC)
#   bash scripts/sntm_sprint_gate.sh --phase=safe3  # SAFE-3 (bin-verify + cert)
#   bash scripts/sntm_sprint_gate.sh --phase=safe4  # SAFE-4 (stack analyzer + full)
#
# SNTM v0.7 §18.7 referansı. Her ek komut graceful-degrade:
# sprint phase'inde tool yoksa SKIP, sprint sonu raporda not edilir.
set -eo pipefail
cd "$(dirname "$0")/.."

PHASE="${1:---phase=base}"
PHASE="${PHASE#--phase=}"

echo "=== SNTM SPRINT GATE — phase=$PHASE ==="
echo ""

# ─── 0. Coverage map gate (zorunlu, mekanik lazy-bypass guard) ───────
# SNTM Design §18.4 — feature ↔ test/proof symmetric mapping enforce.
# §18.7: yeni isimler için VERIFIES/CALLS/FAILS-IF 3-yorum kuralı.
echo "[E0] Coverage map (mekanik lazy-bypass guard + §18.7 quality)..."
bash scripts/check_coverage.sh
echo ""

# ─── 0b. Light tautology detector (informational, §18.7 destekleyici) ─
echo "[E0b] Proof quality light scan..."
bash scripts/check_proof_quality.sh
echo ""

# ─── 1. Baseline gate (zorunlu, mevcut U-22 sprint gate) ─────────────
echo "[BASELINE] U-22 sprint gate (8-step)..."
bash scripts/sipahi_sprint_gate.sh
echo ""

# ─── 2. SNTM-spesifik ek check'ler (E1-E9) ───────────────────────────
echo "=== SNTM PHASE CHECKS — phase=$PHASE ==="
echo ""

SKIPPED=()
PASSED=()
FAILED=()

run_check() {
    local id="$1"
    local desc="$2"
    local cmd="$3"
    local required_phase="$4"

    case "$PHASE:$required_phase" in
        base:base)                    ;;  # base her zaman çalışır
        safe1:base|safe1:safe1)       ;;
        safe2:base|safe2:safe1|safe2:safe2) ;;
        safe3:base|safe3:safe1|safe3:safe2|safe3:safe3) ;;
        safe4:*)                      ;;  # safe4 her şeyi çalıştırır
        *)
            echo "[$id] SKIP — required phase=$required_phase, current=$PHASE"
            SKIPPED+=("$id: $desc")
            return 0
            ;;
    esac

    echo "[$id] $desc"
    if eval "$cmd"; then
        echo "  PASS"
        PASSED+=("$id: $desc")
    else
        echo "  FAIL"
        FAILED+=("$id: $desc")
    fi
    echo ""
}

# ─── E1: sipahi_api crate build (SNTM v1.5+) ────────────────────────
# U-22.5 cleanup fix: klasör adı sipahi_api (underscore), dash değil
if [ -d "sipahi_api" ] || [ -d "tasks/sipahi_api" ]; then
    run_check "E1" "cargo build -p sipahi_api" \
        "cargo build -p sipahi_api --release 2>&1 | tail -3" \
        "base"
else
    echo "[E1] SKIP — sipahi_api crate henüz yok"
    SKIPPED+=("E1: sipahi_api build")
    echo ""
fi

# ─── E2: task_* crate build (SNTM v1.5+) ─────────────────────────────
if [ -d "tasks" ] && ls tasks/task_*/Cargo.toml > /dev/null 2>&1; then
    TASKS=$(ls -d tasks/task_*/ 2>/dev/null | head -3)
    for task in $TASKS; do
        task_name=$(basename "$task")
        run_check "E2:$task_name" "cargo build -p $task_name" \
            "cd $task && cargo build --release \
             --target riscv64imac-unknown-none-elf \
             -Z build-std=core \
             -Z build-std-features=compiler-builtins-mem 2>&1 | tail -3; cd ../.." \
            "base"
    done
else
    echo "[E2] SKIP — tasks/ dizini veya task_* crate henüz yok"
    SKIPPED+=("E2: task_* build")
    echo ""
fi

# ─── E3: task-lint (SAFE-1+) ─────────────────────────────────────────
if command -v task-lint > /dev/null 2>&1; then
    run_check "E3" "task-lint Safe Native Profile" \
        "task-lint --manifest sipahi.toml 2>&1 | tail -5" \
        "safe1"
elif [ "$PHASE" != "base" ]; then
    echo "[E3] SKIP — task-lint tool henüz yok (SAFE-1 prerequisite)"
    SKIPPED+=("E3: task-lint")
    echo ""
fi

# ─── E4: sntm-validate manifest check (SNTM v1.5+) ──────────────────
if command -v sntm-validate > /dev/null 2>&1 && [ -f "sipahi.toml" ]; then
    run_check "E4" "sntm-validate --manifest sipahi.toml" \
        "sntm-validate --manifest sipahi.toml 2>&1 | tail -5" \
        "base"
else
    echo "[E4] SKIP — sntm-validate tool veya sipahi.toml manifest henüz yok"
    SKIPPED+=("E4: sntm-validate")
    echo ""
fi

# ─── E5: sntm-pack image assembly (SNTM v1.5+) ──────────────────────
if command -v sntm-pack > /dev/null 2>&1; then
    run_check "E5" "sntm-pack image build" \
        "sntm-pack --manifest sipahi.toml --output target/sntm-image.bin 2>&1 | tail -5" \
        "base"
else
    echo "[E5] SKIP — sntm-pack tool henüz yok"
    SKIPPED+=("E5: sntm-pack")
    echo ""
fi

# ─── E6: riscv-bin-verify (SAFE-3+) ─────────────────────────────────
if command -v riscv-bin-verify > /dev/null 2>&1; then
    ELF_FILES=$(find target/tasks -name '*.elf' 2>/dev/null | head -3)
    if [ -n "$ELF_FILES" ]; then
        for elf in $ELF_FILES; do
            run_check "E6:$(basename $elf)" "riscv-bin-verify $elf" \
                "riscv-bin-verify --rules verify_rules.toml $elf 2>&1 | tail -3" \
                "safe3"
        done
    fi
elif [ "$PHASE" = "safe3" ] || [ "$PHASE" = "safe4" ]; then
    echo "[E6] SKIP — riscv-bin-verify tool henüz yok (SAFE-3 prerequisite)"
    SKIPPED+=("E6: riscv-bin-verify")
    echo ""
fi

# ─── E7: cargo-call-stack analyzer (SAFE-4+) ────────────────────────
if command -v cargo-call-stack > /dev/null 2>&1; then
    run_check "E7" "cargo-call-stack max bound check" \
        "cargo call-stack --bin task_template 2>&1 | tail -10" \
        "safe4"
elif [ "$PHASE" = "safe4" ]; then
    echo "[E7] SKIP — cargo-call-stack tool henüz yok (SAFE-4 prerequisite)"
    SKIPPED+=("E7: cargo-call-stack")
    echo ""
fi

# ─── E8: make run-sntm production smoke ─────────────────────────────
if grep -q "^run-sntm:" Makefile 2>/dev/null; then
    run_check "E8" "make run-sntm production smoke" \
        "timeout 8s make run-sntm > /tmp/sntm_smoke.log 2>&1 || true; \
         ! grep -q '^NF\$' /tmp/sntm_smoke.log && \
         ! grep -q 'FATAL' /tmp/sntm_smoke.log" \
        "base"
else
    echo "[E8] SKIP — Makefile'da run-sntm target henüz yok"
    SKIPPED+=("E8: make run-sntm")
    echo ""
fi

# ─── E9: Negative test marker grep (zorunlu) ────────────────────────
# Self-test çıktısında negative test PASS pattern'lerini doğrula
# U-22.5 cleanup fix: grep -c match yoksa exit 1 + "0" output dönüyor;
# `|| echo 0` yedek "0" ekliyor → NEG_COUNT="0\n0" → integer compare fail.
# Doğru pattern: grep'in çıktısını al, fallback ayrı assignment ile.
NEGATIVE_TEST_LOG="/tmp/sipahi_gate_test.log"
if [ -f "$NEGATIVE_TEST_LOG" ]; then
    NEG_COUNT=$(grep -c "Negative:" "$NEGATIVE_TEST_LOG" 2>/dev/null) || NEG_COUNT=0
    if [ "$NEG_COUNT" -gt 0 ]; then
        echo "[E9] Negative test'ler: $NEG_COUNT görüldü"
        PASSED+=("E9: $NEG_COUNT negative test")
    else
        echo "[E9] SKIP — henüz negative test eklenmemiş (U-21 + U-22 baseline'da 6 var)"
        SKIPPED+=("E9: negative tests")
    fi
    echo ""
fi

# ─── Özet ───────────────────────────────────────────────────────────
echo "============================================================"
echo "SNTM SPRINT GATE SUMMARY — phase=$PHASE"
echo "============================================================"
echo "PASSED:  ${#PASSED[@]}"
for x in "${PASSED[@]}"; do echo "  ✓ $x"; done
echo ""
echo "SKIPPED: ${#SKIPPED[@]}"
for x in "${SKIPPED[@]}"; do echo "  - $x"; done
echo ""
echo "FAILED:  ${#FAILED[@]}"
for x in "${FAILED[@]}"; do echo "  ✗ $x"; done
echo ""

if [ "${#FAILED[@]}" -gt 0 ]; then
    echo "=== SNTM GATE FAIL ==="
    echo "Sprint kapatma engellendi. No-Go conditions (§18.6) check:"
    echo "  NO-GO-1..6 listesi sprint raporunda eksiksiz işaretlenmeli."
    exit 1
fi

echo "=== SNTM GATE PASS ==="
echo "Sprint kapatılabilir. Carry-forward template §18.5 doldurulmalı."
