#!/bin/sh
# Runs rgram against a real account. The API credentials (values from
# https://my.telegram.org) come from the git-ignored `.env` at the project
# root (mode 600):
#   RGRAM_API_ID=...
#   RGRAM_API_HASH=...
# or, without it, from the macOS login Keychain:
#   security add-generic-password -U -a rgram -s rgram.api_id -w
#   security add-generic-password -U -a rgram -s rgram.api_hash -w
# They are never printed or logged.
# Usage: scripts/run-live.sh [--debug] [log file]
# RGRAM_WITHHOLD_VIEWS=1 (the default here) browses without reading messages.
set -eu

cd "$(dirname "$0")/.."
profile=release
if [ "${1:-}" = "--debug" ]; then
    profile=dev
    shift
fi
log="${1:-target/run-live.log}"

if [ -f .env ]; then
    set -a
    . ./.env
    set +a
fi
secret() {
    security find-generic-password -a rgram -s "$1" -w 2>/dev/null || true
}
RGRAM_API_ID="${RGRAM_API_ID:-$(secret rgram.api_id)}"
RGRAM_API_HASH="${RGRAM_API_HASH:-$(secret rgram.api_hash)}"
if [ -z "$RGRAM_API_ID" ] || [ -z "$RGRAM_API_HASH" ]; then
    echo "RGRAM_API_ID or RGRAM_API_HASH is missing (.env or Keychain)" >&2
    exit 1
fi
export RGRAM_API_ID RGRAM_API_HASH
export RGRAM_WITHHOLD_VIEWS="${RGRAM_WITHHOLD_VIEWS:-1}"

cargo build --profile "$profile" -p rgram
dir=release
[ "$profile" = dev ] && dir=debug
exec "target/$dir/rgram" 2>"$log"
