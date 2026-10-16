#!/usr/bin/env bash
# What the drawn realization's frames cost, over several runs on each leg (Xvfb, headless
# sway, headless mutter), each run in a private session (tools/sessions/session.sh inside
# dbus-run-session):
# - rows: the 1000-row probe (release build), driven by tools/sessions/scenarios/rows.sh: its
#   first frame, with its parts (prepare: styles resolved and texts shaped; layout; record;
#   raster; glyphs), the time from the app's start to that frame, the hover frames (small
#   damage) and the wheel frames (a scroll);
# - relayout: the same probe with MODE=layout, row 0's padding changed twelve times, each a
#   relayout of the list;
# - hello: hello's first frame (release build).
#
#   tools/bench/frames.sh <release apps dir> <out dir> [rounds]
#
# Each app starts once its session has settled, APP_DELAY seconds (1 unless set) after it is
# ready, as an app starts on a desktop whose compositor is idle. TOOLS (the drivers) is
# tools/bin.
#
# Appends to <out dir>/frames.txt (every [frame] line and every app-start TIMING line,
# prefixed by leg, case and round), emptied first unless KEEP=1, and prints, per leg and case,
# the medians over every run it holds, then BENCH lines for the first frame of the rows
# against its targets (under 50 ms, its layout under 16 ms) on every leg. Zero rounds prints
# what <out dir>/frames.txt holds; FIRST numbers the first round (1 unless set), so runs
# appended by several calls keep apart.
set -u
release=$1; out=$2; rounds=${3:-5}
here=$(cd "$(dirname "$0")/../.." && pwd)
sessions=$here/tools/sessions
export TOOLS=${TOOLS:-$here/tools/bin} CHECK="python3 -I $sessions/check.py" APP_DELAY=${APP_DELAY:-1}
mkdir -p "$out"
if [ "${KEEP:-0}" != 1 ] && [ "$rounds" -gt 0 ]; then : > "$out/frames.txt"; fi

run() { # run <leg> <name> <round> <kind> <app> <command...> (VAR=value settings in SETTINGS)
  local leg=$1 name=$2 round=$3 kind=$4 app=$5
  shift 5
  local dir="$out/run-$leg-$name-$round"
  rm -rf "$dir"; mkdir -p "$dir"
  # shellcheck disable=SC2086
  env $SETTINGS OUT="$dir" APP="$app" timeout 120 dbus-run-session -- \
    "$sessions/session.sh" "$kind" "$@" > "$dir/session.log" 2>&1
  grep -E '^\[frame\]' "$dir/app.log" | sed "s/^/$leg $name $round /" >> "$out/frames.txt"
  grep -E '^TIMING .* app start to first frame' "$dir/session.log" | sed "s/^/$leg $name $round /" >> "$out/frames.txt"
}

for round in $(seq "${FIRST:-1}" $(( ${FIRST:-1} + rounds - 1 ))); do
  for leg in x11 sway mutter; do
    case $leg in
      x11) base="SKENE_SIZE=800x900" ;;
      sway) base="DRIVER=sway SKENE_SIZE=800x900 SWAY_BORDER=none MONITOR=800x900" ;;
      mutter) base="DRIVER=mutter-dbus SKENE_SIZE=800x900 HEADER=38" ;;
    esac
    SETTINGS="$base" run "$leg" rows "$round" "$leg" "$release/rows" bash "$sessions/scenarios/rows.sh"
    SETTINGS="$base MODE=layout" run "$leg" relayout "$round" "$leg" "$release/rows" sleep 5.5
    SETTINGS="" run "$leg" hello "$round" "$leg" "$release/hello" sleep 0.3
  done
done

python3 -I - "$out/frames.txt" <<'PY'
import re, statistics, sys

FRAME = re.compile(
  r"\[frame\] (\d+) total ([\d.]+) ms prepare ([\d.]+) layout ([\d.]+) record ([\d.]+) "
  r"raster ([\d.]+) glyphs ([\d.]+) (full|damage) region (\d+)x(\d+)"
)
runs = {}
starts = {}

for line in open(sys.argv[1]):
  leg, case, round_, rest = line.split(" ", 3)

  if rest.startswith("TIMING"):
    starts.setdefault((leg, case), []).append(float(re.search(r"(\d+) ms", rest).group(1)))

    continue

  match = FRAME.search(rest)

  if match:
    runs.setdefault((leg, case), {}).setdefault(round_, []).append(match.groups())

def median(values):
  return f"{statistics.median(values):.2f} [{min(values):.2f}-{max(values):.2f}]" if values else "none"

verdicts = []

for (leg, case), rounds in sorted(runs.items()):
  firsts = [frames[0] for frames in rounds.values()]
  later = [frame for frames in rounds.values() for frame in frames[1:]]
  area = float(firsts[0][8]) * float(firsts[0][9])
  parts = {
    name: [float(first[index]) for first in firsts]
    for index, name in enumerate(["total", "prepare", "layout", "record", "raster", "glyphs"], 1)
  }
  print(
    f"{leg} {case} (runs {len(firsts)}): first frame {median(parts['total'])} ms "
    f"(prepare {median(parts['prepare'])}, layout {median(parts['layout'])}, record "
    f"{median(parts['record'])}, raster {median(parts['raster'])}, glyphs {median(parts['glyphs'])}); "
    f"app start to first frame {median(starts.get((leg, case), []))} ms"
  )

  if case == "rows":
    small = [float(frame[1]) for frame in later if frame[7] == "damage" and float(frame[8]) * float(frame[9]) < area / 4]
    large = [float(frame[1]) for frame in later if frame[7] == "damage" and float(frame[8]) * float(frame[9]) >= area / 4]
    print(f"{leg} {case}: hover {median(small)} ms (n={len(small)}); scroll {median(large)} ms (n={len(large)})")
    total, layout = statistics.median(parts["total"]), statistics.median(parts["layout"])
    verdicts.append(
      f"BENCH {leg} rows first frame {total:.2f} ms ({'under' if total < 50 else 'NOT under'} 50), "
      f"layout {layout:.2f} ms ({'under' if layout < 16 else 'NOT under'} 16)"
    )

  if case == "relayout":
    relayouts = [float(frame[1]) for frame in later if frame[7] == "full"]
    parts = [[float(frame[index]) for frame in later if frame[7] == "full"] for index in (2, 3)]
    print(
      f"{leg} {case}: relayout {median(relayouts)} ms (n={len(relayouts)}; prepare "
      f"{median(parts[0])}, layout {median(parts[1])})"
    )

print("\n".join(verdicts))
PY
