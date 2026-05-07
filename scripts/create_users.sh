#!/usr/bin/env bash
# Creates 100 users via the admin API and prints their tokens.
# Usage: ./create_users.sh <admin_uuid_or_base64>
set -euo pipefail

INPUT="${1:?Usage: $0 <admin_uuid_or_base64>}"
BASE_URL="https://bj.eibach2026.de"
COUNT=100

# If input is 22 chars it's base64url — decode to plain UUID
if [[ ${#INPUT} -eq 22 ]]; then
    ADMIN_UUID=$(python3 -c "
import base64, uuid, sys
raw = base64.urlsafe_b64decode(sys.argv[1] + '==')
print(str(uuid.UUID(bytes=raw)))
" "$INPUT")
    echo "Decoded base64 -> UUID: $ADMIN_UUID"
else
    ADMIN_UUID="$INPUT"
fi

echo "Creating $COUNT users..."

for i in $(seq 1 $COUNT); do
    response=$(curl -sf -X POST "$BASE_URL/admin/$ADMIN_UUID/users" -H "Content-Type: application/json")
    echo "$i: $response"
done

echo "Done."
