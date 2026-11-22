# Paths are supplied as environment variables so filenames are never parsed as code.
set -eu
stage=$OXIGRAM_UPDATE_STAGE
root=$OXIGRAM_UPDATE_ROOT
incoming=$OXIGRAM_UPDATE_INCOMING
backup="$stage/previous"
marker="$root/$OXIGRAM_UPDATE_MARKER"

cleanup() {
    if [ ! -e "$backup" ]; then
        rm -rf -- "$stage"
    fi
}
trap cleanup EXIT

# TDLib stops before the helper is launched. Wait for the process handle to
# disappear before moving the directory, and stop if shutdown has stalled.
attempt=0
while kill -0 "$OXIGRAM_UPDATE_PID" 2>/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 150 ] || exit 1
    sleep 0.2
done

restart() {
    (
        cd "${OXIGRAM_UPDATE_WORKDIR:-$root}" 2>/dev/null || cd "$root"
        exec "$root/$OXIGRAM_UPDATE_EXE"
    ) </dev/null >/dev/null 2>&1 &
    launched=$!
}

[ "$(sed -n '2p' "$marker")" = "$OXIGRAM_UPDATE_FROM" ] || exit 1
if [ "$OXIGRAM_UPDATE_MARKER" = OxiGram.package ]; then
    (cd "$root" && LC_ALL=C find . -mindepth 1 -print) > "$stage/current"
    LC_ALL=C sort "$stage/current" > "$stage/inventory"
    sed '1,2d' "$marker" > "$stage/expected"
    if ! cmp -s "$stage/inventory" "$stage/expected"; then
        restart
        exit 1
    fi
fi
if ! mv -- "$root" "$backup"; then
    restart
    exit 1
fi
if [ ! -e "$root" ] && [ ! -L "$root" ] && mv -- "$incoming" "$root"; then
    restart
    sleep 0.5
    if kill -0 "$launched" 2>/dev/null; then
        rm -rf -- "$backup"
        exit 0
    fi
    mv -- "$root" "$stage/failed"
fi

# Leave the old package in the staging folder if even rollback cannot move it.
[ ! -e "$root" ] && [ ! -L "$root" ] || exit 1
mv -- "$backup" "$root"
restart
