#!/usr/bin/env bash
# Installs the spike's APKs on the running emulator, runs each test in a process of its own (one
# `am instrument` each, so every cold pass starts with Minikin's process caches empty), appends
# the `key=value` lines the tests log to the results file, and pulls the text-parity run's files
# (records, images, probes) into the parity directory. The suite is `full` (everything, the
# advance-rule sweep last) or `sweep` (the sweep alone).
set -uo pipefail

apks=$1
out=$2
parity=${3:-parity}
suite=${4:-full}
package=dev.skene.spike.text
files=/sdcard/Android/data/$package/files/parity
: > "$out"

adb wait-for-device
echo "run=0 test=device emulator.sdk=$(adb shell getprop ro.build.version.sdk | tr -d '\r')" >> "$out"
echo "run=0 test=device emulator.model=$(adb shell getprop ro.product.model | tr -d '\r ')" >> "$out"
echo "run=0 test=device emulator.density=$(adb shell wm density | tr -d '\r ' | tr ':' '_')" >> "$out"
for prop in ro.build.type ro.debuggable ro.kernel.android.checkjni dalvik.vm.checkjni; do
  echo "run=0 test=device emulator.$prop=$(adb shell getprop $prop | tr -d '\r ')" >> "$out"
done
adb install -r -t "$apks/app-release.apk"
adb install -r -t "$apks/app-release-androidTest.apk"

# Compile both ahead of time, as a benchmark run does, so neither side is timed interpreted.
adb shell cmd package compile -m speed -f $package
adb shell cmd package compile -m speed -f $package.test

# run TEST REP [CLASS] [instrument arguments...]
run() {
  local test=$1 rep=$2 class=${3:-SpikeTest}
  shift 3 2>/dev/null || shift $#
  local log="$out.instrument.$test.$rep.txt"

  adb logcat -c
  adb shell am instrument -w -e class "$package.$class#$test" -e runId "$test.$rep" "$@" \
    $package.test/androidx.test.runner.AndroidJUnitRunner > "$log" 2>&1
  local ok
  ok=$(grep -c "OK (1 test)" "$log")
  echo "run=$rep test=$test instrument.ok=$ok" >> "$out"
  # The emulator's logcat may keep earlier runs' lines after a clear; keep this run's alone.
  adb logcat -d -v raw -s SKENE_SPIKE:I | tr -d '\r' | grep "^RID:$test.$rep " \
    | sed "s/^RID:[^ ]* /run=$rep test=$test /" >> "$out"

  if [ "$ok" = 0 ]; then
    echo "::group::$test run $rep failed"
    cat "$log"
    adb logcat -d -v brief '*:E' | tail -80
    echo "::endgroup::"
  fi
}

# The advance-rule sweep, at each density (dpi): the probe of Minikin's per-glyph advances
# (which rule they follow), A, B with Parley's own advances (`linear`), with the probe's rule and
# `chws` as Minikin sets it (`hinted`) and with the rule alone (`rule`, records only), and C drawn
# from the linear and the hinted sets, each in `d<dpi>/`.
sweep() {
  for dpi in 320 420 480 560; do
    adb shell wm density "$dpi"
    sleep 3
    local dir=d$dpi
    run advances "$dpi" ParityTest -e dir "$dir"
    run nativeSimple "$dpi" ParityTest -e dir "$dir"

    for set in linear hinted rule; do
      run parleyRule "$set$dpi" ParityTest -e dir "$dir" -e set "$set"
    done

    for set in linear hinted; do
      run raster "$set$dpi" ParityTest -e dir "$dir" -e set "$set"
    done
  done

  adb shell wm density reset
}

# Let the freshly booted emulator's background work settle before timing anything.
sleep 45
adb shell svc power stayon true
adb shell settings put system screen_off_timeout 1800000
adb shell input keyevent KEYCODE_WAKEUP
adb shell wm dismiss-keyguard

if [ "$suite" = sweep ]; then
  adb shell rm -rf "$files"
  run fonts 1 ParityTest
  sweep
  mkdir -p "$parity"
  adb pull "$files/." "$parity" > /dev/null
  echo "pulled $(find "$parity" -type f | wc -l) files into $parity"
  exit 0
fi

for rep in 1 2 3 4 5; do
  run taffy "$rep"
  run compose "$rep"
  run taffyPlain "$rep"
  run taffyParley "$rep" ParityTest
done

run offMain 1
run ink 1
run jniCost 1

# The text-parity run: A (StaticLayout), B (Parley through the harness), C (Parley's glyphs drawn
# by Canvas), then the probes.
adb shell rm -rf "$files"
run fonts 1 ParityTest
run nativeSimple 1 ParityTest
run nativeQuality 1 ParityTest
run nativeSubpixel 1 ParityTest
run parleyFed 1 ParityTest
run parleyStock 1 ParityTest
run raster 1 ParityTest
run probes 1 ParityTest

# The platform's accessibility text settings, each set from the shell and read in a fresh process.
run fontScale 100 ParityTest
for scale in 1.3 2.0; do
  adb shell settings put system font_scale "$scale"
  sleep 3
  run fontScale "${scale/./}" ParityTest
done
adb shell settings put system font_scale 1.0

run fontWeight 0 ParityTest
adb shell settings put secure font_weight_adjustment 300
sleep 3
run fontWeight 300 ParityTest
adb shell settings put secure font_weight_adjustment 0

adb shell input keyevent KEYCODE_WAKEUP
adb shell wm dismiss-keyguard
run contrast off ParityTest -e state off
adb shell settings put secure high_text_contrast_enabled 1
sleep 3
run contrast on ParityTest -e state on
adb shell settings put secure high_text_contrast_enabled 0

sweep

mkdir -p "$parity"
adb pull "$files/." "$parity" > /dev/null
echo "pulled $(find "$parity" -type f | wc -l) files into $parity"

exit 0
