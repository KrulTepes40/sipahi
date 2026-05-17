#!/usr/bin/env bash
# U-27.5 SNTM-R12 runtime verification — QEMU log grep gate.
#
# Codex pre-review fix: kernel `tests::run_all()` scheduler START öncesi
# çalıştığı için kernel self-test ile cross-isolation observation FALSE-PASS/
# FAIL eder. Dış grep gate ile QEMU runtime log üzerinden doğrula.
#
# Kullanıcı ek dikkat (5 maddesi tümü uygulandı):
#   1. Gerçek fail durumunda non-zero exit
#   2. [FAIL] marker varsa kesin fail
#   3. Marker SADECE task=2 attacker Isolated + task=3 Ready/Running iken bassın
#      (trap.rs IN-HANDLER state check + conditional emit — burada doğrulanır)
#   4. Production make run çıktısında marker kesinlikle görünmemeli
#      (G7 no-go regression guard kontrol eder, bu script make run-cross-isolation
#      log'una bakar)
#   5. src/tests/mod.rs içine cross-isolation-demo eklenmez (kernel self-test'e
#      girmez — bu script sadece runtime gate'tir)
#
# Beklenen runtime sırası:
#   1. Kernel boot, scheduler START.
#   2. task_hello scheduled → _start çağrısı.
#   3. cfg(cross-isolation-demo) deliberate write 0x80705000 → trap mcause=7.
#   4. trap.rs mcause 5|7 → handle_task_fault → isolate_task(task_id=2).
#   5. trap.rs IN-HANDLER state check: task=2 Isolated, task=3 Ready/Running.
#   6. State check PASS → marker emit: "[OK] Cross-task PMP isolation enforced...".
#   7. Scheduler devam: task_world (prio=7) Ready, [TICK] timer ilerler.
#   8. Hiçbir [FAIL]/FATAL/SHUTDOWN/POLICY DEGRADE markerı yok.
set -eo pipefail
cd "$(dirname "$0")/.."

LOG=${1:-/tmp/u275_xi.log}
if [ ! -f "$LOG" ]; then
    echo "FAIL: log file $LOG yok — make run-cross-isolation ile üret"
    exit 1
fi

PASS_MARKER='[OK] Cross-task PMP isolation enforced: task=2'
FAIL_MARKER='[FAIL] Cross-task PMP isolation BROKEN'

# ─── Gate 1: PASS markerı en az 1 kez var + task=2 attacker spesifik
# (Codex hardening: marker SADECE task=2 saldırgan için emit edilmeli;
#  trap.rs guard `if task_id == 2 && attacker_isolated && victim_runnable`
#  kuralı zaten enforce ediyor, burada da grep ile doğrula).
if ! grep -qF "$PASS_MARKER" "$LOG"; then
    echo "FAIL Gate 1: '$PASS_MARKER' marker not found in $LOG"
    echo "  — task_hello hiç violate etmedi VEYA trap path isolate çağırmadı"
    echo "  — VEYA marker task=2 dışında bir attacker için emit edildi (yanlış)"
    exit 1
fi

# ─── Gate 2: [FAIL] BROKEN markerı kesinlikle YOK
# (state check trap.rs'te fail ise emit edilir; isolate çalışmadı demektir)
if grep -qF "$FAIL_MARKER" "$LOG"; then
    echo "FAIL Gate 2: '$FAIL_MARKER' marker found — isolate path broken"
    grep -F "$FAIL_MARKER" "$LOG" | head -5
    exit 1
fi

# ─── Gate 3: marker sonrası en az 3 [TICK] devam ediyor
# (task_world unaffected + scheduler progress kanıtı — collateral damage YOK)
POST_MARKER_TICKS=$(awk -v m="$PASS_MARKER" '
    BEGIN { found=0; count=0 }
    found && /\[TICK\]/ { count++ }
    index($0, m) > 0 { found=1 }
    END { print count }
' "$LOG")
if [ "$POST_MARKER_TICKS" -lt 3 ]; then
    echo "FAIL Gate 3: marker sonrası sadece $POST_MARKER_TICKS [TICK] var (>=3 beklenen)"
    echo "  — scheduler post-trap ilerleyemedi VEYA task_world bir şekilde durdu"
    exit 1
fi

# ─── Gate 4: NF / FATAL / POLICY SHUTDOWN markerı YOK
# (panik/halt → izolasyon güvenli kapanış değil; fail-fast yakala)
if grep -qE 'FATAL|\[NF\]|\[POLICY\] SHUTDOWN' "$LOG"; then
    echo "FAIL Gate 4: panic/halt marker found"
    grep -E 'FATAL|\[NF\]|\[POLICY\] SHUTDOWN' "$LOG" | head -5
    exit 1
fi

# ─── Tüm gate'ler PASS
echo "PASS: cross-task PMP isolation runtime observed (SNTM-R12 runtime)"
echo "  Gate 1: '$PASS_MARKER' ✓ (task=2 attacker spesifik)"
echo "  Gate 2: '[FAIL] ... BROKEN' yok ✓"
echo "  Gate 3: post-marker [TICK] count = $POST_MARKER_TICKS (>=3) ✓"
echo "  Gate 4: no FATAL / NF / POLICY SHUTDOWN ✓"
exit 0
