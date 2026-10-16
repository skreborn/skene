#!/usr/bin/env bash
# Inside a session (tools/controls/session.sh <kind> with APP=none): starts $PROBE under each
# environment of the input method choice and prints `[<label>] PASS|FAIL detect <env>: <chosen>`
# against what each should choose: the variables GTK and Qt read first, then XMODIFIERS with
# the D-Bus name present, then the names alone.
#
#   detect.sh <expected for nothing set> <env=expected>...
set -u
label=${LABEL:-$KIND}
run() {
  local expected=$1; shift
  local log=$OUT/detect-$RANDOM.log
  env -u GTK_IM_MODULE -u QT_IM_MODULE -u XMODIFIERS SKENE_TRACE=1 PROBE_FIELDS_ONLY=1 \
    SKENE_PROBE=$OUT/detect-probe SKENE_SHOT= "$@" "$PROBE" >"$log" 2>&1 &
  local pid=$!
  for _ in $(seq 100); do grep -q -- '-> ' "$log" 2>/dev/null && break; sleep 0.05; done
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  local chosen
  chosen=$(grep -oE -- '-> [a-z]+' "$log" | head -1 | cut -c4-)
  local what="${*:-nothing set}"
  if [ "$chosen" = "$expected" ]; then
    echo "[$label] PASS detect $what: $chosen"
  else
    echo "[$label] FAIL detect $what: $chosen (expected $expected)"
  fi
}
run "$1"; shift
for pair in "$@"; do
  run "${pair##*=>}" ${pair%%=>*}
done
