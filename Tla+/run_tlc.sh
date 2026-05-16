#!/usr/bin/env bash
# U-18 GÖREV 5: Run TLC on all 7 Sipahi specs
set -u
cd "$(dirname "$0")"
rm -f *_TTrace_*.tla *_TTrace_*.bin
TLA_JAR="$HOME/.tlaplus/tla2tools.jar"
RC=0
for spec in SipahiScheduler SipahiCapability SipahiPolicy SipahiWatchdog SipahiDegradeRecover SipahiBudgetFairness SipahiIPC SipahiSNTM; do
    echo "=== $spec ==="
    sleep 1   # avoid timestamp collision in states/ dir
    out=$(timeout 240s java -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC -workers auto -config "$spec.cfg" "$spec.tla" 2>&1)
    if echo "$out" | grep -q "Model checking completed. No error"; then
        states=$(echo "$out" | grep -oE '[0-9,]+ distinct states found' | head -1)
        echo "PASS — $states"
    else
        echo "FAIL"
        echo "$out" | tail -8
        RC=1
    fi
done
exit $RC
