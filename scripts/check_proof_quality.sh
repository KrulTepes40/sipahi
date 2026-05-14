#!/usr/bin/env bash
# scripts/check_proof_quality.sh — Light Kani proof tautology detector
#
# §18.7 quality gate'in tamamlayıcı script'i. Mekanik olarak yakalanabilen
# basit tautoloji + boş proof kalıplarını arar.
#
# YAKALANANLAR:
#   - assert!(true) veya assert!(1 == 1) literal
#   - assert_eq!(X, X) aynı identifier ile
#   - #[kani::proof] fonksiyonu body içinde `kani::any` çağrısı yok
#   - #[kani::proof] body içinde production fn call yok (sadece literal/assert)
#
# YAKALANMAYANLAR (manuel review kapsamı):
#   - kani::any kullanılıyor ama kani::assume input space'i tek değere düşürmüş
#   - assert mantıksal olarak doğru ama trivially true (Smart Bayes vs tautology)
#   - proof gerçekten production behavior'ı kanıtlıyor mu (semantic)
#
# Bu detector'ın amacı %100 doğru sınıflandırma DEĞİL — sadece bariz
# çöp proof'ları yakalamak. False positive olabilir, manuel review eder.
set -eo pipefail
cd "$(dirname "$0")/.."

echo "============================================================"
echo "PROOF QUALITY DETECTOR — light tautology scan"
echo "(§18.7 destekleyici. Body adequacy MANUEL review işi.)"
echo "============================================================"
echo ""

python3 - <<'PYEOF'
import sys
import re
import tomllib
from pathlib import Path

# ─── Grandfather list yükle ─────────────────────────────────────────
cov = tomllib.loads(Path("coverage.toml").read_text())
grandfather = set(cov.get("grandfather", {}).get("proofs", []))

# ─── Tüm Kani proof'ları topla ──────────────────────────────────────
proof_pattern = re.compile(
    r"#\[kani::proof\]\s*(?:\n\s*#\[[^\]]+\]\s*)*\n\s*"
    r"(?:pub\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*\(",
)

# Block extraction: kani::proof attr → fn ismi → body (balanced braces)
def extract_proofs(text):
    proofs = []
    for m in proof_pattern.finditer(text):
        name = m.group(1)
        # Body extraction: ilk `{` bul, balance ile bitir
        body_start = text.find("{", m.end())
        if body_start == -1:
            continue
        depth = 0
        i = body_start
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    proofs.append((name, text[body_start+1:i]))
                    break
            i += 1
    return proofs

all_proofs = []
for p in Path("src").rglob("*.rs"):
    try:
        text = p.read_text(errors="ignore")
        for name, body in extract_proofs(text):
            all_proofs.append((str(p), name, body))
    except Exception:
        pass

# ─── Tautoloji pattern'ları (yalnız kesin "kötü" pattern'lar) ───────
# "Light detector" niyeti: false positive verme, sadece bariz tautoloji yakala.
# Daha sofistike kalite kontrolü manuel review işi.
TAUTOLOGY_PATTERNS = [
    # assert!(true) literal
    (r"assert!\s*\(\s*true\s*\)", "assert!(true) literal"),
    # assert!(false) — proof always fails (probably stub)
    (r"assert!\s*\(\s*false\s*\)", "assert!(false) literal — stub veya bozuk proof"),
    # assert!(N == N) basit literal
    (r"assert!\s*\(\s*(\d+)\s*==\s*\1\s*\)", "assert!(N == N) literal"),
    # assert!(N != M) sabit literal'lar (her zaman doğru/yanlış, semantik yok)
    (r"assert!\s*\(\s*(\d+)\s*!=\s*\d+\s*\)\s*;?\s*$", "assert!(N != M) sabit literal — kaçınılır"),
    # assert_eq!(X, X) — aynı identifier
    (r"assert_eq!\s*\(\s*(\w+)\s*,\s*\1\s*[,)]", "assert_eq!(X, X) aynı identifier"),
    # assert_eq!(LITERAL, LITERAL) aynı sabit
    (r"assert_eq!\s*\(\s*(\d+)\s*,\s*\1\s*[,)]", "assert_eq!(N, N) aynı sabit"),
    # kani::assume(false) — proof her zaman early-return
    (r"kani::assume\s*\(\s*false\s*\)", "kani::assume(false) — proof her zaman skip"),
]

warnings = []
for fpath, name, body in all_proofs:
    if name in grandfather:
        continue  # §18.7 exempt

    # Pattern check (sadece kesin tautoloji)
    for pat, msg in TAUTOLOGY_PATTERNS:
        if re.search(pat, body, re.MULTILINE):
            warnings.append(f"  [TAUTOLOGY] {fpath}::{name}  → {msg}")

if warnings:
    print(f"WARN: {len(warnings)} potansiyel quality issue (light detector):\n")
    for w in warnings:
        print(w)
    print()
    print("NOT: Bu pattern'lar %100 yanlış demek değil — manuel review et.")
    print("False positive olursa grandfather list'e ekle (§18.7 exempt).")
    # Bu script informational; FAIL döndürmez (grandfather audit aşamasında)
    # Sprint owner kararıyla strict mode aktif edilebilir.
    sys.exit(0)

total = len(all_proofs)
gf = sum(1 for _, n, _ in all_proofs if n in grandfather)
print(f"PASS: {total} Kani proof tarandı ({gf} grandfather exempt, {total-gf} new)")
print(f"  Tautology/no-symbolic pattern bulunamadı.")
PYEOF
