#!/bin/sh
# Regenerates the collapsible to-do list in README.md, between the
# <!-- todo:begin --> and <!-- todo:end --> markers, from docs/parity.md.
# Run it after updating the registry.
set -eu

cd "$(dirname "$0")/.."
list="$(mktemp)"
trap 'rm -f "$list" "$list.readme"' EXIT

awk -F'|' '
function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
function flush() {
    if (title == "") return
    printf "<details>\n<summary><b>%s</b>: %d of %d done", title, done, total
    if (progress > 0) printf ", %d in progress", progress
    printf "</summary>\n\n%s\n</details>\n\n", items
}
/^## / {
    flush()
    title = substr($0, 4)
    sub(/ \(.*$/, "", title)
    items = ""; done = 0; total = 0; progress = 0
    next
}
NF == 7 && $2 !~ /^ *(Area|---) *$/ {
    feature = trim($3); milestone = trim($4); status = trim($5)
    if (status == "not applicable") next
    total++
    if (status == "done") { done++; box = "[x]"; note = "" }
    else if (status == "in progress") { progress++; box = "[ ]"; note = " *(in progress)*" }
    else { box = "[ ]"; note = "" }
    items = items "- " box " " feature " (" milestone ")" note "\n"
}
END { flush() }
' docs/parity.md > "$list"

grep -q '<!-- todo:begin -->' README.md || { echo "README.md has no todo markers" >&2; exit 1; }
awk -v list="$list" '
/<!-- todo:begin -->/ { print; while ((getline line < list) > 0) print line; skip = 1; next }
/<!-- todo:end -->/ { skip = 0 }
!skip
' README.md > "$list.readme"
mv "$list.readme" README.md
