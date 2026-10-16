#!/usr/bin/env bash
# Compares one API level's text-parity run on the host: validates the record sets, puts Parley's
# records over the images the platform drew of its glyph runs (option C, `drawGlyphs`, and its
# floor, `drawTextRun`), and compares every pair the findings read, the advance-rule sweep's per
# density.
set -uo pipefail

tp=$(realpath "$1")
dir=$2
cd "$dir" || exit 1

for set in a-simple a-quality a-subpixel b-fed b-stock; do
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
compare a-subpixel b-fed
compare a-simple a-subpixel
compare a-simple c diff
compare b-fed c
compare a-simple cfloor
compare b-fed cfloor
compare c cfloor

# The advance-rule sweep: a directory per density, C restamped from the B set it was drawn from.
for sweep in d*/; do
  [ -d "$sweep" ] || continue
  (
    cd "$sweep" || exit 0
    density=${sweep%/}

    for set in a-simple b-linear b-hinted b-rule; do
      [ -f "$set/records.json" ] && "$tp" validate "$set/records.json" > "validate-$set.txt" 2>&1
      echo "$density validate $set: $(tail -1 "validate-$set.txt" 2>/dev/null)"
    done

    for set in linear hinted; do
      [ -d "c-$set/png" ] && "$tp" restamp "b-$set/records.json" --images "c-$set" --rasterizer "android Canvas.drawGlyphs (skia)"
      [ -d "cfloor-$set/png" ] && "$tp" restamp "b-$set/records.json" --images "cfloor-$set" --rasterizer "android Canvas.drawTextRun (minikin, skia)"
    done

    for pair in "a-simple b-linear" "a-simple b-hinted" "a-simple b-rule" "a-simple c-linear" \
      "a-simple c-hinted diff" "a-simple cfloor-linear" "a-simple cfloor-hinted" "b-hinted c-hinted" \
      "b-linear cfloor-linear" "b-hinted cfloor-hinted"; do
      # shellcheck disable=SC2086
      compare $pair | sed "s/^/$density /"
    done
  )
done
exit 0
