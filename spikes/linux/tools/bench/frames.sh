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
# - hello: hello's first frame (release build);
# - lazy, lazy-10k, lazy-100k: the same probe's rows in a LazyColumn (LAZY=1), a thousand, ten
#   thousand and a hundred thousand of them, driven as rows is: only the rows in view and a
#   few beyond are built;
# - lazy-relayout: the lazy thousand with MODE=layout.
#
#   tools/bench/frames.sh <release apps dir> <out dir> [rounds]
#
# LEGS (x11 sway mutter unless set) and CASES (every case above unless set) choose what runs,
# so two builds can take turns, a round each, through KEEP=1 and FIRST.
#
# After the rounds asked for (6 unless given, the fewest a 95% interval of a median takes), each
# judged case (rows and the three lazy lists) goes on a round at a time on each leg whose
# verdict is still undecided, until MAX_ROUNDS (15 unless set): a verdict is decided once the
# 95% interval of the median first frame (between two order statistics, distribution free) lies
# wholly under or wholly over its target, for the first frame and its layout alike.
#
# Each app starts once its session has settled, APP_DELAY seconds (1 unless set) after it is
# ready, as an app starts on a desktop whose compositor is idle. TOOLS (the drivers) is
# tools/bin.
#
# Appends to <out dir>/frames.txt (every [frame] line and every app-start TIMING line,
# prefixed by leg, case and round), emptied first unless KEEP=1, and prints, per leg and case,
# the medians over every run it holds, then BENCH lines judging each judged case's first frame
# against its targets (under 50 ms, its layout under 16 ms) on every leg. Zero rounds prints what
# <out dir>/frames.txt holds; FIRST numbers the first round (1 unless set), so runs appended by
# several calls keep apart.
set -u
release=$1; out=$2; rounds=${3:-6}
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

cases=${CASES:-rows relayout hello lazy lazy-10k lazy-100k lazy-relayout}

one_round() { # one_round <round> <legs> <cases>
  local round=$1 leg case base
  for leg in $2; do
    case $leg in
      x11) base="SKENE_SIZE=800x900" ;;
      sway) base="DRIVER=sway SKENE_SIZE=800x900 SWAY_BORDER=none MONITOR=800x900" ;;
      mutter) base="DRIVER=mutter-dbus SKENE_SIZE=800x900 HEADER=38" ;;
    esac
    for case in $3; do
      case $case in
        rows) SETTINGS="$base" run "$leg" rows "$round" "$leg" "$release/rows" bash "$sessions/scenarios/rows.sh" ;;
        relayout) SETTINGS="$base MODE=layout" run "$leg" relayout "$round" "$leg" "$release/rows" sleep 5.5 ;;
        hello) SETTINGS="" run "$leg" hello "$round" "$leg" "$release/hello" sleep 0.3 ;;
        lazy) SETTINGS="$base LAZY=1" run "$leg" lazy "$round" "$leg" "$release/rows" bash "$sessions/scenarios/rows.sh" ;;
        lazy-10k) SETTINGS="$base LAZY=1 ROWS=10000" run "$leg" lazy-10k "$round" "$leg" "$release/rows" bash "$sessions/scenarios/rows.sh" ;;
        lazy-100k) SETTINGS="$base LAZY=1 ROWS=100000" run "$leg" lazy-100k "$round" "$leg" "$release/rows" bash "$sessions/scenarios/rows.sh" ;;
        lazy-relayout) SETTINGS="$base LAZY=1 MODE=layout" run "$leg" lazy-relayout "$round" "$leg" "$release/rows" sleep 5.5 ;;
      esac
    done
  done
}

# The judged cases whose first frame is undecided against a target, as `<leg> <case>` a line,
# from frames.txt.
undecided() {
  python3 -I - "$out/frames.txt" <<'PY'
import re, sys
from math import comb

FRAME = re.compile(r"\[frame\] (\d+) total ([\d.]+) ms prepare ([\d.]+) layout ([\d.]+) ")
JUDGED = ("rows", "lazy", "lazy-10k", "lazy-100k")
firsts = {}

for line in open(sys.argv[1]):
  leg, case, round_, rest = line.split(" ", 3)
  match = FRAME.search(rest)

  if case in JUDGED and match and (leg, case, round_) not in firsts:
    firsts[(leg, case, round_)] = (float(match.group(2)), float(match.group(4)))

def interval(values):
  values = sorted(values)
  n, k = len(values), 0
  while sum(comb(n, i) for i in range(k + 1)) / 2 ** n <= 0.025:
    k += 1
  return (values[k - 1], values[n - k]) if k else None

for leg, case in sorted({(leg, case) for leg, case, _ in firsts}):
  for index, target in ((0, 50), (1, 16)):
    values = [value[index] for (one, kind, _), value in firsts.items() if (one, kind) == (leg, case)]
    bounds = interval(values)

    if bounds is None or bounds[0] < target < bounds[1] or target in bounds:
      print(leg, case)
      break
PY
}

first=${FIRST:-1}
for round in $(seq "$first" $(( first + rounds - 1 ))); do
  one_round "$round" "${LEGS:-x11 sway mutter}" "$cases"
done

if [ "$rounds" -gt 0 ]; then
  round=$(( first + rounds ))
  while [ "$round" -le "${MAX_ROUNDS:-15}" ]; do
    pending=$(undecided)
    [ -n "$pending" ] || break
    while read -r leg case; do
      one_round "$round" "$leg" "$case"
    done <<< "$pending"
    round=$(( round + 1 ))
  done
fi

python3 -I - "$out/frames.txt" <<'PY'
import re, statistics, sys
from math import comb

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

def interval(values):
  """The 95% interval of the median, between two order statistics; None under six values."""
  values = sorted(values)
  n, k = len(values), 0
  while sum(comb(n, i) for i in range(k + 1)) / 2 ** n <= 0.025:
    k += 1
  return (values[k - 1], values[n - k]) if k else None

def verdict(values, target):
  bounds = interval(values)
  middle = statistics.median(values)
  if bounds is None:
    return f"{middle:.2f} ms (n={len(values)}, too few rounds for an interval: undecided against {target})"
  low, high = bounds
  word = "under" if high < target else "NOT under" if low > target else "undecided against"
  return f"{middle:.2f} ms (95% interval {low:.2f}-{high:.2f}, n={len(values)}: {word} {target})"

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

  if case == "rows" or case in ("lazy", "lazy-10k", "lazy-100k"):
    # After the first frame and the second (the window's focus shown), a frame that paints a
    # small part is a hover; one that paints much of the window, or lays it out again (a lazy
    # list realizing the rows it scrolls to), is a scroll.
    driven = [frame for frames in rounds.values() for frame in frames[2:]]
    small = [float(frame[1]) for frame in driven if frame[7] == "damage" and float(frame[8]) * float(frame[9]) < area / 4]
    large = [
      float(frame[1]) for frame in driven
      if frame[7] == "full" or float(frame[8]) * float(frame[9]) >= area / 4
    ]
    print(f"{leg} {case}: hover {median(small)} ms (n={len(small)}); scroll {median(large)} ms (n={len(large)})")
    verdicts.append(
      f"BENCH {leg} {case} first frame {verdict(parts['total'], 50)}, layout {verdict(parts['layout'], 16)}"
    )

  if case.endswith("relayout"):
    relayouts = [float(frame[1]) for frame in later if frame[7] == "full"]
    parts = [[float(frame[index]) for frame in later if frame[7] == "full"] for index in (2, 3)]
    print(
      f"{leg} {case}: relayout {median(relayouts)} ms (n={len(relayouts)}; prepare "
      f"{median(parts[0])}, layout {median(parts[1])})"
    )

print("\n".join(verdicts))
PY
