#!/usr/bin/env bash
# Installs the spike's APKs on the running emulator, runs each test in a process of its own (one
# `am instrument` each, so every cold pass starts with Minikin's process caches empty) and
# appends the `key=value` lines the tests log to the results file.
set -uo pipefail

apks=$1
out=$2
: > "$out"

adb wait-for-device
echo "run=0 test=device emulator.sdk=$(adb shell getprop ro.build.version.sdk | tr -d '\r')" >> "$out"
echo "run=0 test=device emulator.model=$(adb shell getprop ro.product.model | tr -d '\r ')" >> "$out"
echo "run=0 test=device emulator.density=$(adb shell wm density | tr -d '\r ' | tr ':' '_')" >> "$out"
adb install -r -t "$apks/app-release.apk"
adb install -r -t "$apks/app-release-androidTest.apk"

# Compile both ahead of time, as a benchmark run does, so neither side is timed interpreted.
adb shell cmd package compile -m speed -f dev.skene.spike.text
adb shell cmd package compile -m speed -f dev.skene.spike.text.test

run() {
  local test=$1 rep=$2
  local log="$out.instrument.$test.$rep.txt"

  adb logcat -c
  adb shell am instrument -w -e class "dev.skene.spike.text.SpikeTest#$test" \
    dev.skene.spike.text.test/androidx.test.runner.AndroidJUnitRunner > "$log" 2>&1
  local ok
  ok=$(grep -c "OK (1 test)" "$log")
  echo "run=$rep test=$test instrument.ok=$ok" >> "$out"
  adb logcat -d -v raw -s SKENE_SPIKE:I | grep '=' | tr -d '\r' | sed "s/^/run=$rep test=$test /" >> "$out"

  if [ "$ok" = 0 ]; then
    echo "::group::$test run $rep failed"
    cat "$log"
    adb logcat -d -v brief '*:E' | tail -80
    echo "::endgroup::"
  fi
}

for rep in 1 2 3; do
  run taffy "$rep"
  run compose "$rep"
done

run offMain 1
run ink 1
exit 0
