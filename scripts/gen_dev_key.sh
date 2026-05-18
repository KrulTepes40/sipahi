#!/usr/bin/env bash
# SAFE-3 (sprint-u32, Section 8 CR-7): ephemeral dev signing key bootstrap.
#
# Idempotent local dev keypair generator for build-time cert + image signing.
#
# Doctrine:
#   - Private key (keys/*.priv) ASLA repo'da (.gitignore enforce).
#   - Public key (keys/*.pub) committed — verify roundtrip için repo'da.
#   - CI her run kendi ephemeral key'ini üretir (drift guard signed artifact
#     için değil, sadece sign+verify roundtrip — Section 8 CR-7).
#   - Production sig HSM/OTP-provisioned (SAFE-4 sonrası ayrı sprint).
#
# Local dev: idempotent — mevcut keys/dev-image.priv varsa skip.
# CI: bu script clean tree'de çalışır, keypair tek run ömrü.

set -eo pipefail
cd "$(dirname "$0")/.."

mkdir -p keys

PRIV="keys/dev-image.priv"
PUB="keys/dev-image.pub"

if [ -f "$PRIV" ] && [ -f "$PUB" ]; then
    echo "[gen_dev_key] keys/dev-image.{priv,pub} mevcut — skip"
    exit 0
fi

if ! command -v openssl >/dev/null 2>&1; then
    echo "[gen_dev_key] FAIL: openssl bulunamadı (apt install openssl)"
    exit 1
fi

# Ed25519 keypair üret (PEM PKCS#8 format, OpenSSL 1.1.1+).
openssl genpkey -algorithm ED25519 -out "$PRIV"
openssl pkey -in "$PRIV" -pubout > "$PUB"

# Permission lockdown — sadece owner okuyabilir.
chmod 600 "$PRIV"
chmod 644 "$PUB"

echo "[gen_dev_key] PASS: keys/dev-image.{priv,pub} oluşturuldu"
echo "[gen_dev_key] priv (gitignored): $PRIV"
echo "[gen_dev_key] pub  (committed):  $PUB"
