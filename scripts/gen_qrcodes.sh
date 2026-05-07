#!/usr/bin/env bash
# Generates a QR code PNG for each token in the CSV into the qrcodes/ folder.
# Usage: ./gen_qrcodes.sh [benutzer.csv]
set -euo pipefail

CSV="${1:-benutzer.csv}"
OUT="qrcodes"
BASE_URL="https://bj.eibach2026.de"

mkdir -p "$OUT"

# Skip header line, read Token column (field 1)
tail -n +2 "$CSV" | while IFS=',' read -r token _rest; do
    [[ -z "$token" ]] && continue
    url="$BASE_URL/$token"
    qrencode -o "$OUT/$token.png" -s 8 -m 0 "$url"
    echo "Generated: $OUT/$token.png -> $url"
done

echo "Done. QR codes saved to $OUT/"
