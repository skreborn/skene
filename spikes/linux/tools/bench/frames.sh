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
# prefixed by leg, case and round), emptied first unless KEEP=1, and prints through
# tools/bench/frames.py, per leg and case, the medians over every run it holds, then BENCH lines
# judging each judged case's first frame against its targets (under 50 ms, its layout under
# 16 ms) on every leg. Zero rounds prints what <out dir>/frames.txt holds; FIRST numbers the
# first round (1 unless set), so runs appended by several calls keep apart.
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

# The judged cases whose first frame or layout is undecided against its target, as
# `<leg> <case>` a line, from frames.txt.
undecided() {
  python3 -I "$here/tools/bench/frames.py" undecided "$out/frames.txt"
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

python3 -I "$here/tools/bench/frames.py" summary "$out/frames.txt"
