#!/usr/bin/env bash
# Rebuilds the drawn-Linux prototype and re-runs everything its reports call VERIFIED.
#
# The run is four phases, so what measures time runs alone and what only checks runs side by
# side, every test process in a private session of its own (the display runner's), sharing no
# display, bus, compositor or input with another:
#   1. alone: hello's build times, size and startup (tools/testing/measure.sh, a target of its
#      own);
#   2. the builds: formatting, clippy and rustdoc (with skene-gui-linux's `gpu` feature too), every
#      test binary on both paths, the CLI, the shell's tools and probes, the release apps, and
#      beside them an app made by `skene new` (built, checked, tested and run, in a target of its
#      own);
#   3. side by side, POOL jobs at once (4 unless set), the longest first: every test leg, nextest,
#      the steadiness rounds, the isolation check, the screenshots, the controls' sessions, the
#      platform-text cases and the GPU path's tests;
#   4. alone: the timed parts: the shell's sessions (scenarios paced by the clock, and the
#      sessions' start-up times), 1000 labels and shaping (verify), the frame costs
#      (tools/bench/frames.sh, six rounds and more until each judged case's first frame is judged
#      against its targets, or MAX_ROUNDS), and the repaint costs (tools/paint/repaint.sh).
# Each section's wall time is in the summary.
#
# What runs (the stages that added it in brackets):
#   - [core] formatting (stable rustfmt), clippy with warnings denied, rustdoc, on every member;
#     every test of the workspace (foundation, skene-parley, skene-terminal, cli, the cores); the
#     platform-text cases (scratchpad/platform-text/linux) and the cases of spikes/text-parity's
#     corpus that found Parley's gaps (soft hyphens, bidi, Thai, Khmer, Lao, Burmese, Devanagari,
#     tabs, stacked marks) laid out by Skene's engine and compared with live Pango (pango-view,
#     same fontconfig): lines, their widths, the clusters' visual order, family per character,
#     emoji drawn from a colour glyph, PNGs by FreeType masks and by vello outlines; fontconfig
#     rendering settings honoured (the user's own fonts.conf and the desktop's XSETTINGS-style
#     values beneath it); measured size against drawn ink; 1000 labels: build, measure, taffy,
#     draw; the 1000-row probe's texts shaped one at a time and as a batch, with the shape cache
#     and without.
#   - [shell] the shell's tools (drive, clip, portal-stub, xsettings-manager, KWin's stand-in
#     input method kwin-input-method, the helper KWin grants its fake input to,
#     kwin-fake-input) into tools/bin;
#     hello (release) and the shell probes (states, environment, clipboard, runtime: debug;
#     rows: release); every scenario of tools/sessions/shell-checks.sh, each in a private
#     session (Xvfb with and without a window manager, headless sway, headless mutter, weston,
#     KWin), a scenario failing where its compositor or its app never came up; the frame costs on
#     Xvfb, headless sway and headless mutter: the 1000-row probe's first frame and its parts,
#     start to first frame, hover, scroll and relayout frames, hello's first frame, the same rows
#     in a LazyColumn (a thousand, ten thousand, a hundred thousand) and relaid out, with the rows'
#     first frame judged against its targets (under 50 ms, its layout under 16 ms) by its
#     median's 95% interval.
#   - [controls] the controls probe driven by tools/controls/harness.py with real input and read
#     over AT-SPI in private sessions (Xvfb, headless sway, headless mutter, headless
#     gnome-shell); Hangul through IBus, Fcitx5 and XIM on X11, gnome-shell's IBus over
#     text-input-v3 and Fcitx5 behind sway; the input method choice; a password field's keys to
#     the input method against GTK 4's GtkPasswordEntry; textprobe.py on the drawn field and on
#     GtkEntry; Orca 46's speech against GtkEntry, GtkSwitch and GtkButton on X11 and on headless
#     gnome-shell's Wayland.
#   - [testing] every test of the workspace by plain `cargo test` on the X11 leg and the Wayland
#     leg (headless mutter), the GUI crates (skene-conformance-linux: the conformance suite and
#     the realization's driver tests; hello, notes, Simple, acme-rating, the facade) on sway,
#     weston, KWin and KWin's Xwayland, and on sway, mutter, KWin, KWin's Xwayland and X11 at
#     scale 1.5; by cargo-nextest (a process and a session per test) on X11 and Wayland; the GUI
#     crates' tests five more times on each of X11 and Wayland; the runner never touching the
#     user's session (tools/testing/isolation.sh), and a test run outside the runner failing
#     before its body runs; the screenshots of hello and notes (screenshots/); an app made by
#     `skene new`; the GTK prototype's numbers beside these ($GTK_PROTO/results/summary.txt).
#     The X11 leg runs on Xorg's dummy driver with an inputtest touchscreen where both are
#     installed (xserver-xorg-core, xserver-xorg-video-dummy), so touch is tested there, and on
#     Xvfb otherwise; the touch tests (gestures::, handles::) run once more, at 1 and 1.5, on
#     sway nested in such an Xorg on its X11 backend (SKENE_SWAY_BACKEND=x11), whose touches
#     headless sway has no way to take. KWin's legs start KWin with its permission checks on,
#     its fake input granted to kwin-fake-input alone, which the tests act through, and a
#     stand-in input method (kwin-input-method), which a field's caret is read through from
#     outside the app.
#   - [paint] the GPU path (skene-gui-linux's `gpu` feature: vello_gpu over wgpu's Vulkan, on
#     Mesa's lavapipe here): its tests (skene-conformance-linux's `gpu` target) on X11, mutter and
#     sway, and on sway and X11 at 1.5 with scale:: and presented:: on the GPU path there; the
#     conformance suite and the facade's tests on the GPU path on X11 and mutter, the suite on X11
#     GPU_SUITE_ROUNDS more times (3 unless set), its aborts counted; full and partial repaints
#     at 1920x1080 and 3840x2160 on both paths, on Xvfb and headless mutter.
#
# The GTK realization (gui-linux-gtk) and its probes (probes/) are kept, outside the
# workspace, for reference.
#
# Results go to results/ (summary.txt, logs, PNGs, JSON; results/jobs/ for each job of phase 3).
# The target directory is $CARGO_TARGET_DIR (target-testing by default), deleted at the end
# unless KEEP_TARGET=1. GTK_PROTO names the GTK prototype's directory (../gtk/proto unless set),
# whose cargo-nextest is borrowed where tools/bin has none and whose numbers are shown beside
# these; SKENE_KWIN names the kwin_wayland to run (as for skene-runner). SKIP_MEASURE=1,
# SKIP_CORE=1, SKIP_SHELL=1, SKIP_FRAMES=1, SKIP_CONTROLS=1 and SKIP_PAINT=1 skip those parts.
set -euo pipefail

cd "$(dirname "$0")"
here=$PWD
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$here/target-testing}" CARGO_INCREMENTAL=0
export RUST_BACKTRACE=0 LANG=C.UTF-8
unset DISPLAY WAYLAND_DISPLAY RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTDOCFLAGS FONTCONFIG_FILE SKENE_PRIVATE_SESSION
export XDG_CACHE_HOME="$here/results/cache"
triple=x86_64-unknown-linux-gnu
results="$here/results"
gtk_proto="${GTK_PROTO:-../gtk/proto}"
summary="$results/summary.txt"
jobs_dir="$results/jobs"
debug="$CARGO_TARGET_DIR/$triple/debug"
release="$CARGO_TARGET_DIR/$triple/release"
rm -rf "$jobs_dir"
mkdir -p "$results" "$here/tools/bin" "$XDG_CACHE_HOME" "$jobs_dir"
: > "$summary"
run_started=$(date +%s)

say() {
  echo "$*" | tee -a "$summary"
}

ms_since() {
  echo $(( ($(date +%s%N) - $1) / 1000000 ))
}

# A section of the summary: its heading, and the wall time of the one before it.
section_started=0
section_name=""
section() {
  if [ -n "$section_name" ]; then
    echo "  ($section_name: $(( $(date +%s) - section_started )) s)" | tee -a "$summary"
  fi
  section_name=$1
  section_started=$(date +%s)
  if [ -n "$1" ]; then say "== $1"; fi
}

peak=0
measure_target() {
  local size
  size=$(du -sm "$CARGO_TARGET_DIR" 2>/dev/null | cut -f1 || echo 0)
  if [ "$size" -gt "$peak" ]; then peak=$size; fi
  echo "(target now ${size} MB)" >> "$summary"
}

# Jobs (phase 3's, and the app phase 2 builds beside the workspace's builds): each runs in the
# background with its output in results/jobs/<name>.log and its summary lines in
# results/jobs/<name>.summary, at most POOL at once; `gather` waits for them all and adds their
# summaries in the order the jobs were started.
pool_names=()
pool_pids=()
job() { # job <name> <command...>
  local name=$1 running pid
  shift
  while :; do
    running=0
    for pid in "${pool_pids[@]}"; do
      if kill -0 "$pid" 2>/dev/null; then running=$((running + 1)); fi
    done
    if [ "$running" -lt "${POOL:-4}" ]; then break; fi
    wait -n 2>/dev/null || sleep 1
  done
  pool_names+=("$name")
  (
    summary="$jobs_dir/$name.summary"
    : > "$summary"
    started=$(date +%s)
    set +e
    "$@" > "$jobs_dir/$name.log" 2>&1
    echo "  ($name: $(( $(date +%s) - started )) s, alongside the other jobs)" >> "$summary"
  ) &
  pool_pids+=("$!")
}
gather() {
  wait || true
  local name
  for name in "${pool_names[@]}"; do
    tee -a "$summary" < "$jobs_dir/$name.summary"
  done
  pool_names=()
  pool_pids=()
}

summarize() {
  awk '
    /^ *Running / { match($0, /deps\/[A-Za-z0-9_]+-/); name = substr($0, RSTART + 5, RLENGTH - 6) " " $2 }
    /^ *Doc-tests / { name = $2 " doctests" }
    /^test result:/ { if (name != "") printf "  %s: %s %s passed, %s failed in %s\n", name, $3, $4, $6, $NF; name = "" }
  ' "$1"
}
# How many of a log's passing tests had nothing to test on their leg, as skene-runner lists them.
untested() {
  grep -oE 'skene-runner: tested nothing on the [^ ]+ leg, though counted as passed: [0-9]+' "$1" | awk '{s+=$NF} END {print s+0}'
}
passed() { grep -hoE '[0-9]+ passed' "$@" | awk '{s+=$1} END {print s+0}'; }
failed() { grep -hoE '[0-9]+ failed' "$@" | awk '{s+=$1} END {print s+0}'; }

gui_crates="-p skene-conformance-linux -p hello -p notes-gui -p skene-simple-gui -p skene-gui -p acme-rating"

say "== toolchain: $(cargo --version), $(rustc --version)"
say "== workspace members: $(cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(" ".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"])))')"
say "== the X11 leg's server: $( [ -e /usr/lib/xorg/modules/drivers/dummy_drv.so ] && [ -e /usr/lib/xorg/modules/input/inputtest_drv.so ] && command -v Xorg > /dev/null && echo "Xorg with its dummy driver and an inputtest touchscreen" || echo "Xvfb (no Xorg with the dummy and inputtest drivers)")"

section "the CLI and the runner"
cargo build -p skene --bins > "$results/cli-build.log" 2>&1
cp "$debug/skene" "$debug/skene-runner" tools/bin/
say "installed tools/bin/skene and tools/bin/skene-runner"
if [ ! -x tools/bin/cargo-nextest ] && [ -x "$gtk_proto/tools/bin/cargo-nextest" ]; then
  cp "$gtk_proto/tools/bin/cargo-nextest" tools/bin/
fi
export PATH="$here/tools/bin:$PATH"

# Phase 1, alone.
if [ "${SKIP_MEASURE:-0}" != 1 ]; then
  section "measurements: hello (its own target directory, removed after)"
  tools/testing/measure.sh "$here/target-measure" 2>&1 | tee "$results/measure.txt" | tee -a "$summary"
fi

# Phase 2, the builds.
section "format, clippy, rustdoc"
if cargo +stable fmt --all --check > "$results/fmt.log" 2>&1; then
  say "cargo +stable fmt --all --check: clean"
else
  say "cargo +stable fmt --all --check: DIFFERS (results/fmt.log)"
fi
if cargo clippy --workspace --all-targets -- -D warnings > "$results/clippy.log" 2>&1; then
  say "cargo clippy --workspace --all-targets -- -D warnings: passed"
else
  say "cargo clippy: FAILED (results/clippy.log)"
fi
cargo doc --workspace --no-deps > "$results/doc.log" 2>&1
say "cargo doc --workspace --no-deps: $(grep -cE '^(warning|error)' "$results/doc.log" || true) warnings or errors"
if [ "${SKIP_PAINT:-0}" != 1 ]; then
  if cargo clippy -p skene-gui-linux -p skene-gui -p skene-conformance -p skene-conformance-linux \
    --features skene-gui-linux/gpu --all-targets -- -D warnings > "$results/clippy-gpu.log" 2>&1; then
    say "cargo clippy with the gpu feature, -D warnings: passed"
  else
    say "cargo clippy with the gpu feature: FAILED (results/clippy-gpu.log)"
  fi
  cargo doc -p skene-gui-linux --features gpu --no-deps > "$results/doc-gpu.log" 2>&1
  say "cargo doc -p skene-gui-linux --features gpu: $(grep -cE '^(warning|error)' "$results/doc-gpu.log" || true) warnings or errors"
fi
measure_target

# An app made by `skene new`, built, checked, tested and run.
new_app() {
  local apps code started
  # Outside the repository, where no configuration of the workspace's reaches the app.
  apps=$(mktemp -d "${TMPDIR:-/tmp}/skene-new-apps-XXXXXX")
  mkdir -p "$apps/lonely"
  cp tools/bin/skene "$apps/lonely/"
  (cd "$apps" && say "skene new without a runner beside skene: $(lonely/skene new nope 2>&1 | head -c 160 || true)")
  (cd "$apps" && "$here/tools/bin/skene" new greeter | tee -a "$summary")
  (
    cd "$apps/greeter" || exit 1
    # The app builds in a target of its own, removed after: its dependencies resolve apart from
    # the workspace's.
    export CARGO_TARGET_DIR="$here/target-greeter"
    started=$(date +%s%N)
    cargo test > "$results/new-test.log" 2>&1 || true
    say "plain cargo test, no DISPLAY: $(passed "$results/new-test.log") passed, $(failed "$results/new-test.log") failed ($(ms_since "$started") ms, building the app and its dependencies)"
    code=0
    "$here/tools/bin/skene" check > "$results/new-check.log" 2>&1 || code=$?
    say "skene check (linux): exit $code: $(grep '^skene check' "$results/new-check.log" | tr '\n' ' ')"
    printf 'platforms = ["linux", "ios"]\n' > skene.toml
    code=0
    "$here/tools/bin/skene" check > "$results/new-check-ios.log" 2>&1 || code=$?
    say "skene check (linux, ios): exit $code: $(grep '^skene check' "$results/new-check-ios.log" | tr '\n' ' ')"
    printf 'platforms = ["linux"]\n' > skene.toml
    "$here/tools/bin/skene" test > "$results/new-skene-test.log" 2>&1 || true
    say "skene test (x11): $(passed "$results/new-skene-test.log") passed"
    SKENE_TEST_SESSION=wayland "$here/tools/bin/skene" test > "$results/new-skene-test-wayland.log" 2>&1 || true
    say "skene test (wayland): $(passed "$results/new-skene-test-wayland.log") passed"
    ("$here/tools/bin/skene" build linux 2>/dev/null || true) | grep '^skene build' | tee -a "$summary" || true
    rm -f "$results/greeter-run.png"
    SKENE_SHOT="$results/greeter-run.png" timeout 20 "$here/tools/bin/skene" run > /dev/null 2>&1 || true
    say "skene run, no DISPLAY (the runner's private Xvfb): $(python3 -I -c 'import sys; from PIL import Image; print("frame", Image.open(sys.argv[1]).size)' "$results/greeter-run.png" 2>&1)"
    say "the app's own target: $(du -sm "$CARGO_TARGET_DIR" | cut -f1) MB, removed"
    rm -rf "$CARGO_TARGET_DIR"
  )
  rm -rf "$apps"
}

section "the builds (every test, the CLI, the shell's tools and probes, the release apps) and, beside them, an app made by skene new"
# The app builds in a target of its own, beside the workspace's builds.
job new-app new_app
started=$(date +%s%N)
cargo test --workspace --no-run > "$results/test-build.log" 2>&1
# The sets phase 3 tests by themselves, whose features unify apart from the workspace's.
# shellcheck disable=SC2086
cargo test --no-run $gui_crates >> "$results/test-build.log" 2>&1
cargo test --no-run -p hello -p notes-gui -p skene-simple-gui -p skene-gui -p acme-rating --tests \
  >> "$results/test-build.log" 2>&1
say "cargo test --workspace --no-run and the GUI crates' sets (building every test): $(ms_since "$started") ms"
if [ "${SKIP_PAINT:-0}" != 1 ]; then
  started=$(date +%s%N)
  cargo test -p skene-conformance-linux -p skene-gui --features skene-gui-linux/gpu --no-run \
    > "$results/test-build-gpu.log" 2>&1
  say "the GPU path's tests built (skene-gui-linux's gpu feature): $(ms_since "$started") ms"
fi
# Every binary of the workspace (the CLI, the shell's tools, the probes), on the dependencies the
# tests were built on.
started=$(date +%s%N)
cargo build --workspace --bins > "$results/bins-build.log" 2>&1
for tool in drive clip portal-stub xsettings-manager kwin-input-method kwin-fake-input; do
  cp "$debug/$tool" tools/bin/
done
say "built every binary (the CLI, tools/bin/{drive,clip,portal-stub,xsettings-manager,kwin-input-method,kwin-fake-input}, the probes): $(ms_since "$started") ms"
started=$(date +%s%N)
cargo build --release -p hello -p skene-shell-probes --bins > "$results/shell-release-build.log" 2>&1
if [ "${SKIP_CORE:-0}" != 1 ]; then
  cargo build --release -p skene-parley --example verify > "$results/verify-build.log" 2>&1
fi
if [ "${SKIP_PAINT:-0}" != 1 ]; then
  mkdir -p "$results/paint/cpu" "$results/paint/gpu"
  cp "$release/rows" "$results/paint/cpu/"
  cargo build --release -p skene-shell-probes -p skene-gui-linux --features skene-gui-linux/gpu --bin rows \
    > "$results/paint/build.log" 2>&1
  cp "$release/rows" "$results/paint/gpu/"
  # The CPU-only rows goes back for the frame costs.
  cargo build --release -p skene-shell-probes --bin rows >> "$results/paint/build.log" 2>&1
  say "rows, release: $(du -k "$results/paint/cpu/rows" | cut -f1) KB without the GPU path, $(du -k "$results/paint/gpu/rows" | cut -f1) KB with it"
fi
say "the release builds (hello, the probes, verify, rows with the GPU path and without): $(ms_since "$started") ms"
say "the input tools outside a private session (DISPLAY=:0 set, no session marker): drive: $(DISPLAY=:0 tools/bin/drive x11 key a 2>&1 | head -1); harness.py: $(DISPLAY=:0 OUT=/nonexistent python3.12 -I tools/controls/harness.py focus 2>&1 | head -1); orca.sh: $(DISPLAY=:0 tools/controls/orca/orca.sh 2>&1 | head -1)"
gather
measure_target

# Phase 3's jobs.
workspace_leg() { # workspace_leg <leg>
  local log="$results/test-$1.log" outcome started
  started=$(date +%s%N)
  if SKENE_TEST_SESSION=$1 cargo test --workspace --no-fail-fast > "$log" 2>&1; then
    outcome=passed
  else
    outcome=FAILED
  fi
  say "cargo test --workspace on the $1 leg: $outcome, $(passed "$log") passed ($(untested "$log") of them testing nothing on the leg), $(failed "$log") failed, $(ms_since "$started") ms wall (built before)"
  summarize "$log" | grep -vE ": ok\. 0 passed" | tee -a "$summary"
}
gui_leg() { # gui_leg <log name> <leg> [scale]
  local log="$results/test-$1.log" outcome started
  started=$(date +%s%N)
  # shellcheck disable=SC2086
  if SKENE_TEST_SESSION=$2 SKENE_TEST_SCALE=${3:-1} cargo test --no-fail-fast $gui_crates > "$log" 2>&1; then
    outcome=passed
  elif grep -q "could not start" "$log"; then
    say "the $1 leg: not run: $(grep -m1 -o 'could not start[^)]*)' "$log")"
    return
  else
    outcome=FAILED
  fi
  say "cargo test of the GUI crates and the realization on the $1 leg: $outcome, $(passed "$log") passed ($(untested "$log") of them testing nothing on the leg), $(failed "$log") failed, $(ms_since "$started") ms wall"
  grep -hE "^  .*: not tested here" "$log" | sed -E 's/^.*not tested here: /  not tested here: /' | sort | uniq -c | tee -a "$summary"
}
nextest_leg() { # nextest_leg <leg>
  local started
  started=$(date +%s%N)
  SKENE_TEST_SESSION=$1 cargo nextest run --workspace --no-fail-fast -j 2 > "$results/nextest-$1.log" 2>&1 || true
  say "nextest on the $1 leg ($(cargo nextest --version 2>&1 | head -1), two at once): $(grep -E 'Summary' "$results/nextest-$1.log" | sed 's/^ *//') ($(ms_since "$started") ms wall incl. listing)"
  grep -E '^ +(PASS|FAIL) \[' "$results/nextest-$1.log" | sort -t'[' -k2 -rn | sed -n "1,5s/^ */  slowest ($1): /p" | tee -a "$summary"
}
sway_touch() { # the touch tests on sway nested in Xorg (its X11 backend), at 1 and 1.5
  local log="$results/test-sway-touch.log" scale started
  started=$(date +%s%N)
  : > "$log"
  for scale in 1 1.5; do
    SKENE_SWAY_BACKEND=x11 SKENE_TEST_SESSION=sway SKENE_TEST_SCALE=$scale cargo test --no-fail-fast \
      -p skene-conformance-linux --test it -- gestures:: handles:: >> "$log" 2>&1 || true
  done
  say "the touch tests (gestures::, handles::) on sway nested in Xorg, at 1 and 1.5: $(passed "$log") passed ($(untested "$log") of them testing nothing on the leg), $(failed "$log") failed, $(ms_since "$started") ms wall"
  grep -hE "^  .*: not tested here" "$log" | sed -E 's/^.*not tested here: /  not tested here: /' | sort | uniq -c | tee -a "$summary"
}
steadiness() { # steadiness <leg>: the GUI crates' tests (the suite, the apps, Simple, the facade) five more times
  local rounds="" round
  for round in 1 2 3 4 5; do
    SKENE_TEST_SESSION=$1 cargo test --no-fail-fast -p hello -p notes-gui -p skene-simple-gui \
      -p skene-gui -p acme-rating --tests > "$results/repeat-$1-$round.log" 2>&1 || true
    SKENE_TEST_SESSION=$1 cargo test --no-fail-fast -p skene-conformance-linux --test it suite:: \
      >> "$results/repeat-$1-$round.log" 2>&1 || true
    rounds="$rounds $(passed "$results/repeat-$1-$round.log")/$(failed "$results/repeat-$1-$round.log")"
  done
  say "steadiness, the GUI crates' tests five more times on the $1 leg, passed/failed per round:$rounds"
}
isolation() {
  tools/testing/isolation.sh "$results/isolation" hello notes-gui 2>&1 | tee "$results/isolation.txt" | tee -a "$summary"
  local direct
  direct=$(cargo test -p skene-gui --test it --no-run 2>&1 | sed -nE 's/^ *Executable tests\/it\/main.rs \((.*)\)$/\1/p' | head -1)
  say "a test run outside the runner: $("$direct" 2>&1 | grep -oE 'a test runs in the display runner.s private session[^;]*' | head -1)"
}
screenshots() {
  rm -rf screenshots
  mkdir -p screenshots/mutter
  SKENE_SCREENSHOTS="$here/screenshots" cargo test -p hello -p notes-gui --test screenshots > "$results/screenshots.log" 2>&1 || true
  SKENE_TEST_SESSION=wayland SKENE_SCREENSHOTS="$here/screenshots/mutter" \
    cargo test -p hello -p notes-gui --test screenshots >> "$results/screenshots.log" 2>&1 || true
  say "screenshots, x11: $(cd screenshots && ls ./*.png | tr '\n' ' ')"
  say "screenshots, mutter: $(cd screenshots/mutter && ls ./*.png | tr '\n' ' ')"
}
shell_sessions() {
  rm -rf "$results/shell"
  tools/sessions/shell-checks.sh "$debug" "$release" "$results/shell" | tee "$results/shell-checks.txt" | tee -a "$summary"
  say "shell checks: $(grep -c '\] PASS' "$results/shell-checks.txt" || true) passed, $(grep -c '\] FAIL' "$results/shell-checks.txt" || true) failed"
}
controls_sessions() {
  rm -rf "$results/controls"
  tools/controls/checks.sh "$debug/controls" "$results/controls" \
    | tee "$results/controls-checks.txt" | grep -E 'FAIL|DONE|detect|textprobe|\[orca\]' | tee -a "$summary"
  say "controls checks: $(grep -c '\] PASS' "$results/controls-checks.txt" || true) passed, $(grep -c '\] FAIL' "$results/controls-checks.txt" || true) failed"
}
platform_text() {
  local verify="$release/examples/verify" result config plain
  "$verify" shots "$results/shots" > "$results/shots.txt"
  say "verify shots: $(grep -c '^==' "$results/shots.txt") cases (results/shots.txt, results/shots/*.png)"
  python3 -I tools/pango-compare.py "$results/shots" "$results/pango" | tee "$results/pango-compare.txt" | tee -a "$summary"
  "$verify" emoji "$results/emoji-skene.png" | tee -a "$summary"
  python3 -I - "$results/pango/emoji-pango.png" "$results/emoji-skene.png" <<'EOF' | tee -a "$summary"
import sys
from PIL import Image

def groups(path):
  image = Image.open(path).convert("RGB")
  width, height = image.size
  columns = [any(max(image.getpixel((x, y))) - min(image.getpixel((x, y))) > 60 for y in range(height))
             for x in range(width)]
  count, previous = 0, False
  for column in columns:
    count += column and not previous
    previous = column
  return count, width

pango, skene = groups(sys.argv[1]), groups(sys.argv[2])
print(f"emoji in colour: chromatic glyph groups: pango {pango[0]} (image {pango[1]}px wide), skene {skene[0]} ({skene[1]}px wide)")
EOF
  for config in none mono fullhint; do
    result=$(XDG_CONFIG_HOME="$here/tools/fc/$config" "$verify" settings "$results/settings-$config")
    say "fontconfig: user fonts.conf '$config': $result"
  done
  plain="$here/tools/fc/plain/fonts.conf"
  say "fontconfig: plain configuration (no rendering rule), no desktop values: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" settings "$results/settings-plain")"
  say "fontconfig: plain + desktop antialias=0 hinting=1 hintfull: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" settings "$results/settings-plain-desktop" 0,1,3)"
  say "fontconfig: plain + desktop antialias=1 hinting=0 hintnone + user hintfull rule: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/fullhint "$verify" settings "$results/settings-plain-user" 1,0,0)"
  say "fontconfig: a desktop change after the first paint, one painter (plain configuration): $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" follow | sed -n 1p)"
  say "fontconfig: rules keyed on the request's size and weight (tools/fc/keyed: no antialiasing below 12 px or bold): $(XDG_CONFIG_HOME=$here/tools/fc/keyed "$verify" follow | tail -3 | tr '\n' ';')"
  "$verify" ink | tee "$results/ink.txt" | tail -1 | sed 's/^/measured size against drawn ink: /' | tee -a "$summary"
}
gpu_tests() { # gpu_tests <leg> <scale>
  local leg=$1 scale=$2 name=$1
  [ "$scale" = 1 ] || name=$1-$2
  SKENE_TEST_SESSION=$leg SKENE_TEST_SCALE=$scale SKENE_TRACE=1 cargo test -p skene-conformance-linux \
    --features skene-gui-linux/gpu --test gpu -- --nocapture > "$results/test-gpu-$name.log" 2>&1 || true
  say "the GPU tests on the $leg leg at scale $scale: $(grep -E '^test result' "$results/test-gpu-$name.log" | tail -1); $(grep -E '^\[gpu\] .*, Vulkan\)' "$results/test-gpu-$name.log" | sort -u | head -1)"
  grep -E '^[a-z]+: most [0-9]+ apart' "$results/test-gpu-$name.log" | awk '{ if ($3 > most) most = $3; apart += $5 } END { printf "  every page: a channel at most %d apart, %d pixels beyond 2 in all\n", most, apart }' | tee -a "$summary"
  if [ "$scale" != 1 ]; then
    SKENE_TEST_SESSION=$leg SKENE_TEST_SCALE=$scale SKENE_RENDERER=gpu cargo test -p skene-conformance-linux \
      --features skene-gui-linux/gpu --test it -- scale:: presented:: > "$results/test-gpu-scale-$name.log" 2>&1 || true
    say "  scale:: and presented:: on the GPU path there: $(grep -E '^test result' "$results/test-gpu-scale-$name.log" | tail -1)"
  fi
}
on_gpu() { # on_gpu <leg>: the conformance suite and the facade's tests on the GPU path
  SKENE_TEST_SESSION=$1 SKENE_RENDERER=gpu cargo test --no-fail-fast -p skene-conformance-linux \
    --features skene-gui-linux/gpu --test it suite:: > "$results/test-on-gpu-$1.log" 2>&1 || true
  SKENE_TEST_SESSION=$1 SKENE_RENDERER=gpu cargo test --no-fail-fast -p skene-gui \
    --features skene-gui-linux/gpu --test it >> "$results/test-on-gpu-$1.log" 2>&1 || true
  say "the conformance suite and the facade's tests on the GPU path, $1 leg: $(passed "$results/test-on-gpu-$1.log") passed, $(failed "$results/test-on-gpu-$1.log") failed"
}
on_gpu_rounds() {
  local aborts=0 round
  for round in $(seq "${GPU_SUITE_ROUNDS:-3}"); do
    SKENE_TEST_SESSION=x11 SKENE_RENDERER=gpu cargo test --no-fail-fast -p skene-conformance-linux \
      --features skene-gui-linux/gpu --test it suite:: > "$results/test-on-gpu-x11-$round.log" 2>&1 || true
    aborts=$((aborts + $(grep -c 'buffer overflow detected' "$results/test-on-gpu-x11-$round.log" || true)))
  done
  say "the conformance suite on the GPU path on X11, ${GPU_SUITE_ROUNDS:-3} more rounds: $(passed "$results"/test-on-gpu-x11-*.log) passed, $(failed "$results"/test-on-gpu-x11-*.log) failed, $aborts aborts"
}

# Phase 3: side by side, the longest jobs first (by their wall times alongside the others),
# mutter's legs (the most a busy machine slows) apart from one another.
section "side by side, ${POOL:-4} at once: the tests on every leg and the sessions"
job steadiness-wayland steadiness wayland
if [ "${SKIP_CONTROLS:-0}" != 1 ]; then job controls controls_sessions; fi
job steadiness-x11 steadiness x11
job weston gui_leg weston weston
job wayland-1.5 gui_leg wayland-1.5 wayland 1.5
job kwin-x11-1.5 gui_leg kwin-x11-1.5 kwin-x11 1.5
job nextest-wayland nextest_leg wayland
job sway-1.5 gui_leg sway-1.5 sway 1.5
job workspace-wayland workspace_leg wayland
job kwin-x11 gui_leg kwin-x11 kwin-x11
if [ "${SKIP_PAINT:-0}" != 1 ]; then job on-gpu-rounds on_gpu_rounds; fi
job workspace-x11 workspace_leg x11
job sway gui_leg sway sway
job kwin-1.5 gui_leg kwin-1.5 kwin 1.5
job kwin gui_leg kwin kwin
job x11-1.5 gui_leg x11-1.5 x11 1.5
if [ "${SKIP_PAINT:-0}" != 1 ]; then job on-gpu-wayland on_gpu wayland; fi
job nextest-x11 nextest_leg x11
if [ "${SKIP_PAINT:-0}" != 1 ]; then job on-gpu-x11 on_gpu x11; fi
if [ "${SKIP_PAINT:-0}" != 1 ]; then
  for spec in wayland:1 sway:1.5 x11:1.5 x11:1 sway:1; do
    job "gpu-${spec%:*}-${spec#*:}" gpu_tests "${spec%:*}" "${spec#*:}"
  done
fi
job isolation isolation
job sway-touch sway_touch
if [ "${SKIP_CORE:-0}" != 1 ]; then job platform-text platform_text; fi
job screenshots screenshots
gather
cp "$results/test-x11.log" "$results/test.log" 2>/dev/null || true
say "input the driver sent again, over every run above: clicks aimed again (a window not where it was found) $(cat "$results"/test-*.log "$results"/nextest-*.log "$results"/repeat-*.log | grep -c 'aiming again' || true), keys sent again (a keyboard not yet set up) $(cat "$results"/test-*.log "$results"/nextest-*.log "$results"/repeat-*.log | grep -c 'sending them again' || true)"
measure_target

section "beside the GTK prototype ($gtk_proto/results/summary.txt)"
gtk=$gtk_proto/results/summary.txt
if [ -f "$gtk" ]; then
  grep -E '^test totals|^nextest: Summary|^  GTK tests|^clean cargo|^the first cargo check|^a one-line change|^release hello|^startup, round' "$gtk" | sed 's/^/  GTK: /' | tee -a "$summary"
fi

# Phase 4, alone: what measures time.
if [ "${SKIP_SHELL:-0}" != 1 ]; then
  section "the shell's sessions (tools/sessions/shell-checks.sh)"
  shell_sessions
fi

if [ "${SKIP_CORE:-0}" != 1 ]; then
  section "the platform text's timed parts (verify, release)"
  "$release/examples/verify" bench | sed 's/^/1000 labels: /' | tee "$results/bench.txt" | tee -a "$summary"
  "$release/examples/verify" shaping | tee "$results/shaping.txt" | tee -a "$summary"
fi

if [ "${SKIP_FRAMES:-0}" != 1 ]; then
  section "the frame costs (tools/bench/frames.sh: six rounds a leg, a judged case's first frame until judged, at most MAX_ROUNDS)"
  rm -rf "$results/frames"
  tools/bench/frames.sh "$release" "$results/frames" 6 | tee "$results/frames.txt" | tee -a "$summary"
fi

if [ "${SKIP_PAINT:-0}" != 1 ]; then
  section "the repaint costs on both paths (tools/paint/repaint.sh)"
  tools/paint/repaint.sh "$results/paint/cpu/rows" "$results/paint/gpu/rows" "$results/paint/repaint" 3 \
    | tee "$results/paint/repaint.txt" | tee -a "$summary"
  rm -rf "$results/paint/cpu" "$results/paint/gpu"
fi
measure_target

section ""
say "== target peak: ${peak} MB; the whole run: $(( $(date +%s) - run_started )) s"
if [ "${KEEP_TARGET:-0}" != 1 ]; then
  rm -rf "$CARGO_TARGET_DIR"
  say "deleted $CARGO_TARGET_DIR"
fi
rm -rf "$XDG_CACHE_HOME"
