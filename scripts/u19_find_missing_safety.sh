#!/usr/bin/env bash
# U-19 GÖREV 2: Find unsafe { ... } blocks WITHOUT a SAFETY comment in line above
set -eu
cd "$(dirname "$0")/.."
# For each unsafe { match, check the preceding line for SAFETY
grep -rn "unsafe {" src/ | while IFS=: read -r file lineno _; do
    if [ "$lineno" -gt 1 ]; then
        prev_line=$(sed -n "$((lineno-1))p" "$file")
        # If prev line doesn't contain SAFETY (case-insensitive), report
        if ! echo "$prev_line" | grep -qi "SAFETY"; then
            # Also check the same line (one-liner unsafe block could have inline comment)
            same_line=$(sed -n "${lineno}p" "$file")
            if ! echo "$same_line" | grep -qi "SAFETY"; then
                echo "$file:$lineno: $(echo "$same_line" | sed 's/^[[:space:]]*//')"
            fi
        fi
    fi
done
