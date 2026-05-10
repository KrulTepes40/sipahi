# TLA+ Verification Results

This directory holds saved TLC output for the 7 Sipahi specs.

Files are dated; the most recent run is the canonical result. To regenerate:
```bash
bash Tla+/run_tlc.sh > Tla+/results/tlc_run_$(date +%Y-%m-%d).txt 2>&1
```

## Latest run summary

| Spec                    | Result | Distinct states |
|-------------------------|--------|-----------------|
| SipahiScheduler         | PASS   | 13              |
| SipahiCapability        | PASS   | 2,348           |
| SipahiPolicy            | PASS   | 27,556          |
| SipahiWatchdog          | PASS   | 611             |
| SipahiDegradeRecover    | PASS   | 100             |
| SipahiBudgetFairness    | PASS   | 5,120           |
| SipahiIPC               | PASS   | 22              |
| **Total**               | **7/7**| **35,770**      |

TLC version: 2.19. Each spec uses BFS exhaustive search (no probabilistic).
Liveness checked via temporal `[]<>` operators in `.cfg` files.

## What `.tla` files prove

See ARCHITECTURE.md "Formal Verification Scope & Limitations" for the
authoritative scope statement; specs do not claim starvation freedom or
real-time deadline meetings (intentional — see `SipahiScheduler.tla`
disclaimer).
