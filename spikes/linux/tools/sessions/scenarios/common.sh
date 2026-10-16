# Sourced by each scenario: helpers over the session's environment (OUT, KIND, TOOLS, CHECK).
LOG=$OUT/app.log
shot() { sleep "${2:-0.4}"; cp "$OUT/frame.png" "$OUT/$1.png"; }
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
x11_window() { xdotool search --sync --name "$1" | head -1; }
