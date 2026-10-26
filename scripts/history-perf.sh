#!/bin/sh
# Runs the history-perf example (docs/perf.md, M3) and reports gpui's frame
# durations (draw and present) for the scroll between its markers.
# Usage: scripts/history-perf.sh [output directory]
set -eu

out="${1:-target/history-perf}"
mkdir -p "$out"
cargo build --release -p rgram --example history-perf
ZED_MEASUREMENTS=1 cargo run --release -p rgram --example history-perf -- "$out" \
    2>"$out/stderr.log"
awk '
    /^scroll: start/ { on = 1; next }
    /^scroll: end/ { on = 0 }
    on && /^frame duration: / {
        v = $3
        if (v ~ /ns$/) { sub(/ns$/, "", v); v /= 1000000 }
        else if (v ~ /µs$/) { sub(/µs$/, "", v); v /= 1000 }
        else if (v ~ /ms$/) { sub(/ms$/, "", v) }
        else { sub(/s$/, "", v); v *= 1000 }
        print v
    }
' "$out/stderr.log" | sort -n | awk '
    { a[NR] = $1; if ($1 > 8) over++ }
    END {
        if (NR == 0) { print "no frame durations between the markers"; exit 1 }
        printf "frame build: frames %d p50 %.2f ms p99 %.2f ms max %.2f ms over 8 ms %d\n",
            NR, a[int(NR * 0.5) + 1], a[int(NR * 0.99) + 1], a[NR], over
    }
'
