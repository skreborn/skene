# Sourced by each scenario: helpers over the session's environment (OUT, KIND, TOOLS, CHECK).
LOG=$OUT/app.log
shot() { sleep "${2:-0.4}"; cp "$OUT/frame.png" "$OUT/$1.png"; }
await_shot() { # await_shot <name> <x> <y> <rrggbb>: the presented frame once that pixel is that color (5 s at most), kept as <name>
  $CHECK await "$OUT/frame.png" "$OUT/$1.png" "$2" "$3" "$4"
}
await() { # await <pattern> [<after>]: a line of the app's log holds the pattern (after the first line holding <after>, where given), 5 s at most
  local _
  for _ in $(seq 100); do
    if awk -v want="$1" -v after="${2:-}" '
      after == "" || seen { if (index($0, want)) found = 1 }
      after != "" && index($0, after) { seen = 1 }
      END { exit !found }' "$LOG"; then
      return 0
    fi
    sleep 0.05
  done
  echo "INFO $KIND: no line of the app's log holds '$1' after 5 s"
  return 1
}
step() { kill -USR1 "$1"; } # step <pid>: the stand-in with that process id takes its next step
pass() { echo "PASS $*"; }
fail() { echo "FAIL $*"; }
expect() { # expect <pattern> <what>: a line of the app's log holds the pattern
  if grep -qF -- "$1" "$LOG"; then pass "$KIND: $2"; else fail "$KIND: $2"; fi
}
count() { # count <pattern> <n> <what>: exactly n lines of the app's log hold the pattern
  local found
  found=$(grep -cF -- "$1" "$LOG")
  if [ "$found" = "$2" ]; then pass "$KIND: $3 ($found)"; else fail "$KIND: $3 (found $found, expected $2)"; fi
}
pixel() { $CHECK pixel "$OUT/$1.png" "$2" "$3" "$4" "${5:-2}" | sed "s/^\(PASS\|FAIL\) /\1 $KIND: /"; }
size() { $CHECK size "$OUT/$1.png" "$2" "$3" | sed "s/^\(PASS\|FAIL\) /\1 $KIND: /"; }
framed() { grep -q "^\[frame\]" "$LOG"; }
absent() { # absent <pattern> <what>: the app drew a frame, and no line of its log matches
  if ! framed; then
    fail "$KIND: $2 (the app drew no frame)"
  elif grep -q -- "$1" "$LOG"; then
    fail "$KIND: $2"
  else
    pass "$KIND: $2"
  fi
}
quit() { # quit <what>: the app drew a frame, and its process is gone
  if ! framed; then
    fail "$KIND: $1 (the app drew no frame)"
  elif kill -0 "$APP_PID" 2>/dev/null; then
    fail "$KIND: $1"
  else
    pass "$KIND: $1"
  fi
}
x11_window() { xdotool search --sync --name "$1" | head -1; }
