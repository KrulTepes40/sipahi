#!/usr/bin/env bash
# scripts/check_coverage.sh — Sipahi feature coverage map enforcement
#
# WARNING: Bu gate GERÇEK COVERAGE KANITI DEĞİLDİR — isim-tabanlı mekanik guard.
# Amacı: feature eklendi ama matching negative test / Kani proof eklenmedi → fail.
# Test/proof body adequacy = MANUEL review işi.
#
# SNTM Design Doc §18.4 — referans. Gate fail → sprint kapatma bloklanır.
set -eo pipefail
cd "$(dirname "$0")/.."

echo "============================================================"
echo "COVERAGE GATE — name-based mechanical guard (NOT proof of"
echo "test/proof adequacy). Catches lazy bypass: feature added"
echo "without matching test/proof name in repo."
echo "============================================================"
echo ""

python3 - <<'PYEOF'
import sys
import re
import tomllib
from pathlib import Path

# ─── Load coverage.toml ──────────────────────────────────────────────
cov_path = Path("coverage.toml")
if not cov_path.exists():
    print("FAIL: coverage.toml not found at repo root")
    sys.exit(1)

cov = tomllib.loads(cov_path.read_text())

# ─── Parse Cargo.toml [features] section ─────────────────────────────
cargo_path = Path("Cargo.toml")
if not cargo_path.exists():
    print("FAIL: Cargo.toml not found")
    sys.exit(1)

cargo_text = cargo_path.read_text()

# [features] bloğunu izole et — sonraki [section] veya EOF'a kadar
features_match = re.search(
    r"^\[features\]\s*\n(.+?)(?=^\[|\Z)",
    cargo_text, re.MULTILINE | re.DOTALL,
)
if not features_match:
    print("FAIL: Cargo.toml [features] block not found")
    sys.exit(1)

cargo_features = set()
for line in features_match.group(1).splitlines():
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    m = re.match(r"^([a-z][a-z0-9_-]*)\s*=", line)
    if m and m.group(1) != "default":
        cargo_features.add(m.group(1))

cov_features = set(cov.get("feature", {}).keys())

# ─── Check 1: Symmetry — every Cargo.toml feature → coverage entry ──
errors = []

missing_cov = cargo_features - cov_features
if missing_cov:
    errors.append(f"Cargo.toml'da olup coverage.toml'da OLMAYAN feature(lar):")
    for f in sorted(missing_cov):
        errors.append(f"  - {f}  → coverage.toml'a [feature.{f}] entry ekle")

stale_cov = cov_features - cargo_features
if stale_cov:
    errors.append(f"coverage.toml'da olup Cargo.toml'da OLMAYAN entry(ler) (stale):")
    for f in sorted(stale_cov):
        errors.append(f"  - {f}  → coverage.toml'dan [feature.{f}] sil")

# ─── Check 2: Required test/proof names must exist in source tree ───
tests_text = ""
tests_path = Path("src/tests/mod.rs")
if tests_path.exists():
    tests_text = tests_path.read_text()

# Tüm Rust source dosyalarını proof arama için tara
all_source = ""
for p in Path("src").rglob("*.rs"):
    try:
        all_source += p.read_text(errors="ignore") + "\n"
    except Exception:
        pass

def fn_exists(name: str, haystack: str) -> bool:
    """Match `fn NAME(` or `fn NAME<` (generic) anywhere in haystack."""
    pattern = rf"\bfn\s+{re.escape(name)}\s*[(<]"
    return bool(re.search(pattern, haystack))

def find_fn_attribution(name: str, source_files: list[Path]) -> tuple[str, int] | None:
    """Find file + line number where `fn NAME(` is defined."""
    pattern = rf"\bfn\s+{re.escape(name)}\s*[(<]"
    for p in source_files:
        try:
            text = p.read_text(errors="ignore")
            for lineno, line in enumerate(text.splitlines(), start=1):
                if re.search(pattern, line):
                    return (str(p), lineno)
        except Exception:
            pass
    return None

def check_three_comments(name: str, source_files: list[Path]) -> list[str]:
    """
    §18.7: Each non-grandfathered test/proof must have 3 comments
    above its `fn` definition:
        // VERIFIES: <REQUIREMENT-ID>
        // CALLS:    <production fn names>
        // FAILS-IF: <fault model description>
    Returns list of missing comment kinds (empty list = all 3 present).
    """
    location = find_fn_attribution(name, source_files)
    if location is None:
        return ["definition_not_found"]
    fpath, lineno = location

    text = Path(fpath).read_text(errors="ignore").splitlines()
    # 10 satır yukarı bak (yorum bloğu için yeterli pencere)
    start = max(0, lineno - 11)
    window = "\n".join(text[start:lineno-1])

    missing = []
    if not re.search(r"//\s*VERIFIES:\s*\S+", window):
        missing.append("VERIFIES")
    if not re.search(r"//\s*CALLS:\s*\S+", window):
        missing.append("CALLS")
    if not re.search(r"//\s*FAILS-IF:\s*\S+", window):
        missing.append("FAILS-IF")
    return missing

source_paths = list(Path("src").rglob("*.rs"))
grandfather_proofs = set(cov.get("grandfather", {}).get("proofs", []))
grandfather_tests = set(cov.get("grandfather", {}).get("tests", []))

for feat_name, feat in cov.get("feature", {}).items():
    # non_safety = true → diagnostic/meta feature, test required değil
    if feat.get("non_safety"):
        continue

    deferred = str(feat.get("deferred", ""))
    sunset = "sunset_target" in feat

    # ─── Test check ─────────────────────────────────────────────────
    skip_tests = "test" in deferred  # "negative_test" veya "test+proof"
    if not skip_tests:
        for test_name in feat.get("required_negative_tests", []):
            if not fn_exists(test_name, tests_text):
                errors.append(
                    f"[feature.{feat_name}] required_negative_tests: "
                    f"'{test_name}' src/tests/mod.rs'te bulunamadı"
                )
                continue
            # §18.7: grandfather değilse 3-yorum şart
            if test_name not in grandfather_tests:
                missing = check_three_comments(test_name, source_paths)
                for kind in missing:
                    errors.append(
                        f"[feature.{feat_name}] test '{test_name}': "
                        f"// {kind}: yorumu eksik (§18.7 quality gate)"
                    )

    # ─── Proof check ────────────────────────────────────────────────
    skip_proofs = "proof" in deferred or "test+proof" in deferred
    if not skip_proofs:
        for proof_name in feat.get("required_kani_proofs", []):
            if not fn_exists(proof_name, all_source):
                errors.append(
                    f"[feature.{feat_name}] required_kani_proofs: "
                    f"'{proof_name}' src/ ağacında bulunamadı"
                )
                continue
            # §18.7: grandfather değilse 3-yorum şart
            if proof_name not in grandfather_proofs:
                missing = check_three_comments(proof_name, source_paths)
                for kind in missing:
                    errors.append(
                        f"[feature.{feat_name}] proof '{proof_name}': "
                        f"// {kind}: yorumu eksik (§18.7 quality gate)"
                    )

    # ─── Deferred entry validation ──────────────────────────────────
    if deferred:
        if not feat.get("deferred_reason"):
            errors.append(
                f"[feature.{feat_name}] deferred='{deferred}' ama "
                f"deferred_reason eksik (zorunlu — neden ertelendiği yazılmalı)"
            )
        if not feat.get("deferred_target"):
            errors.append(
                f"[feature.{feat_name}] deferred='{deferred}' ama "
                f"deferred_target eksik (zorunlu — hangi sprint'te yapılacak)"
            )

    # ─── Sunset entry validation ────────────────────────────────────
    if sunset:
        # sunset feature için sunset_target zorunlu (zaten varlık kontrolü kapsamı)
        pass

# ─── Requirement traceability check (§18.7 requirement → test/proof) ──
# Her [requirement.X] bloğu için: required_tests/required_proofs varsa
# isimler source'da bulunmalı, ayrıca // VERIFIES: X yorumu yer almalı.
requirements = cov.get("requirement", {})
all_text_lines = "\n".join(p.read_text(errors="ignore") for p in source_paths)
for req_id, req in requirements.items():
    desc = req.get("description")
    fault = req.get("fault_model")
    if not desc:
        errors.append(f"[requirement.{req_id}] description eksik")
    if not fault:
        errors.append(f"[requirement.{req_id}] fault_model eksik")
    for t in req.get("required_tests", []):
        if not fn_exists(t, tests_text):
            errors.append(
                f"[requirement.{req_id}] required_tests: "
                f"'{t}' src/tests/mod.rs'te bulunamadı"
            )
        elif not re.search(rf"//\s*VERIFIES:\s*{re.escape(req_id)}\b", all_text_lines):
            errors.append(
                f"[requirement.{req_id}] test '{t}' var ama "
                f"hiçbir source'da '// VERIFIES: {req_id}' yorumu yok"
            )
    for p in req.get("required_proofs", []):
        if not fn_exists(p, all_source):
            errors.append(
                f"[requirement.{req_id}] required_proofs: "
                f"'{p}' src/ ağacında bulunamadı"
            )
        elif not re.search(rf"//\s*VERIFIES:\s*{re.escape(req_id)}\b", all_text_lines):
            errors.append(
                f"[requirement.{req_id}] proof '{p}' var ama "
                f"hiçbir source'da '// VERIFIES: {req_id}' yorumu yok"
            )

# ─── Sonuç ──────────────────────────────────────────────────────────
if errors:
    print("FAIL: coverage map gap(ler) bulundu:\n")
    for e in errors:
        print(e)
    print()
    print(f"Toplam {len(errors)} eksiklik. Sprint kapatma bloklanır.")
    print("Çözüm yolları:")
    print("  - Eksik test/proof'u repo'ya ekle, ya da")
    print("  - coverage.toml'da deferred=test|proof|test+proof işaretle")
    print("    (reason + target ile birlikte, gerekçe explicit olsun)")
    sys.exit(1)

# Summary
total_features = len(cov_features)
deferred_count = sum(1 for f in cov.get("feature", {}).values() if f.get("deferred"))
non_safety_count = sum(1 for f in cov.get("feature", {}).values() if f.get("non_safety"))
active_count = total_features - deferred_count - non_safety_count
req_count = len(requirements)
gf_proofs = len(grandfather_proofs)
gf_tests = len(grandfather_tests)

print(f"PASS: coverage map symmetric, {total_features} feature(s) mapped")
print(f"  - {active_count} aktif feature: test+proof present in source tree")
print(f"  - {deferred_count} deferred entry: reason + target documented")
print(f"  - {non_safety_count} non-safety feature: check skipped (diagnostic/meta)")
print(f"  - {req_count} requirement ID with VERIFIES traceability")
print(f"  - {gf_tests} test + {gf_proofs} proof grandfathered (§18.7 exempt)")
PYEOF
