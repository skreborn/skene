#!/usr/bin/env bash
# Compares one API level's text-parity run on the host: validates the record sets, puts Parley's
# records over the images the platform drew of its glyph runs (option C, `drawGlyphs`, and its
# floor, `drawTextRun`), and compares every pair the findings read.
set -uo pipefail

tp=$(realpath "$1")
dir=$2
cd "$dir" || exit 1

for set in a-simple a-quality b-fed b-stock; do
  [ -f "$set/records.json" ] && "$tp" validate "$set/records.json" > "validate-$set.txt" 2>&1
  echo "validate $set: $(tail -1 "validate-$set.txt" 2>/dev/null)"
done

if [ -d c/png ]; then
  "$tp" restamp b-fed/records.json --images c --rasterizer "android Canvas.drawGlyphs (skia)"
fi

if [ -d cfloor/png ]; then
  "$tp" restamp b-fed/records.json --images cfloor --rasterizer "android Canvas.drawTextRun (minikin, skia)"
fi

# compare A B [diff]
compare() {
  local a=$1 b=$2
  [ -f "$a/records.json" ] && [ -f "$b/records.json" ] || return 0
  local args=(--out "cmp-$a-$b.md" --json "cmp-$a-$b.json")
  [ "${3:-}" = diff ] && args+=(--diff "diff-$a-$b")
  "$tp" compare "$a/records.json" "$b/records.json" "${args[@]}" 2>&1 | sed "s/^/$a vs $b: /"
}

compare a-simple b-fed diff
compare a-simple b-stock
compare b-stock b-fed
compare a-quality b-fed
compare a-simple a-quality
compare a-simple c diff
compare b-fed c
compare a-simple cfloor
compare b-fed cfloor
compare c cfloor
exit 0
