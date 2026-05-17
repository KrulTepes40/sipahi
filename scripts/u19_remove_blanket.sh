#!/usr/bin/env bash
# U-19 GÖREV 3: Remove blanket #![allow(dead_code)] from listed files
#
# ARCHIVE — U-19 historical sweep (blanket #[allow] removal).
# post-U-29 v2.0: src/sandbox/ silindi (WASM tamamen kaldırıldı), ilgili
# satır yorum altına alındı. Bu script artık benzer haliyle çalıştırılamaz;
# gelecek benzer sweep'lerde örnek olarak korunur.
set -eu
cd "$(dirname "$0")/.."
files=(
    src/arch/csr.rs
    src/hal/secure_boot.rs
    src/hal/iopmp.rs
    src/hal/key.rs
    src/hal/device.rs
    src/common/config.rs
    src/common/types.rs
    src/common/sync.rs
    src/common/error.rs
    # src/sandbox/mod.rs  # U-29: sandbox/ silindi (WASM tamamen kaldırıldı)
    src/kernel/syscall/mod.rs
)
for f in "${files[@]}"; do
    sed -i 's|^#!\[allow(dead_code)\]$|// U-19 GÖREV 3: blanket #!\[allow(dead_code)\] kaldırıldı — tekil işaretlenir|' "$f"
done
echo "done"
