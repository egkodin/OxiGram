#!/bin/sh
# Runs oxigram against a real account. The API credentials (values from
# https://my.telegram.org) come from the git-ignored `.env` at the project
# root (mode 600):
#   OXIGRAM_API_ID=...
#   OXIGRAM_API_HASH=...
# or, without it, from the macOS login Keychain:
#   security add-generic-password -U -a oxigram -s oxigram.api_id -w
#   security add-generic-password -U -a oxigram -s oxigram.api_hash -w
# They are never printed or logged.
# Usage: scripts/run-live.sh [--debug] [log file]
# OXIGRAM_WITHHOLD_VIEWS=1 (the default here) browses without reading messages.
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
    security find-generic-password -a oxigram -s "$1" -w 2>/dev/null || true
}
OXIGRAM_API_ID="${OXIGRAM_API_ID:-$(secret oxigram.api_id)}"
OXIGRAM_API_HASH="${OXIGRAM_API_HASH:-$(secret oxigram.api_hash)}"
if [ -z "$OXIGRAM_API_ID" ] || [ -z "$OXIGRAM_API_HASH" ]; then
    echo "OXIGRAM_API_ID or OXIGRAM_API_HASH is missing (.env or Keychain)" >&2
    exit 1
fi
export OXIGRAM_API_ID OXIGRAM_API_HASH
export OXIGRAM_WITHHOLD_VIEWS="${OXIGRAM_WITHHOLD_VIEWS:-1}"

scripts/check-metal.sh
cargo build --profile "$profile" -p oxigram
dir=release
[ "$profile" = dev ] && dir=debug
exec "target/$dir/oxigram" 2>"$log"
