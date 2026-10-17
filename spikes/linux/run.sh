#!/usr/bin/env bash
# Rebuilds the drawn-Linux prototype and re-runs everything its reports call VERIFIED.
#
# Stage CORE (the text model and engine trait, taffy text leaves, the shared editing core,
# skene-parley over fontconfig and FreeType, the terminal's cells engine and field):
#   - the CLI and the display runner, installed into tools/bin (.cargo/config.toml names
#     tools/bin/skene-runner as the target runner: every test binary gets a private Xvfb and
#     private buses);
#   - formatting (stable rustfmt), clippy with warnings denied, rustdoc, on every member;
#   - every test of the workspace (foundation, skene-parley, skene-gui-linux, skene-terminal,
#     cli, the cores);
#   - the platform-text cases (scratchpad/platform-text/linux) laid out by Skene's engine and
#     compared with live Pango (pango-view, same fontconfig): lines, family per character,
#     emoji drawn from a colour glyph, PNGs by FreeType masks and by vello outlines;
#   - fontconfig rendering settings honoured: the user's own fonts.conf (XDG_CONFIG_HOME, as
#     ~/.config/fontconfig/fonts.conf) and the desktop's XSETTINGS-style values beneath it;
#   - measured size against drawn ink; 1000 labels: build, measure, taffy, draw.
#
# Stage SHELL (skene-gui-linux, the facade drawn by Skene on X11 and Wayland):
#   - the shell's tools (drive, clip, portal-stub, xsettings-manager) into tools/bin;
#   - hello (release) and the shell probes (states, environment, clipboard, runtime: debug;
#     rows: release);
#   - every scenario of tools/sessions/shell-checks.sh, each in a private session: Xvfb (with
#     and without a window manager), headless sway, headless mutter, each under its own
#     dbus-run-session, with a settings-portal stand-in where a scenario asks for one. Nothing
#     reaches the user's display: DISPLAY and WAYLAND_DISPLAY are unset here;
#   - the frame costs, five runs on each of Xvfb, headless sway and headless mutter
#     (tools/bench/frames.sh): the 1000-row probe's first frame and its parts, start to first
#     frame, hover, scroll and relayout frames, and hello's first frame, with the rows' first
#     frame held against its targets (under 50 ms, its layout under 16 ms).
#
# Stage CONTROLS (the built-in controls, each a foundation behavior over a composed tree of
# Skene's views styled by the style model, the picker's list an overlay on a popup surface of
# its own; the text field and editor over the shared editing core; input methods; AccessKit
# carried by Skene, vendor/accesskit_unix and vendor/accesskit_atspi_common):
#   - the controls probe (shell-probes/src/bin/controls.rs) driven by tools/controls/harness.py
#     with real input (tools/bin/drive) and read over AT-SPI, each in a private session
#     (tools/controls/session.sh: Xvfb, headless sway, headless mutter, headless gnome-shell):
#     every control by pointer, keys and a reader's actions; the fields' typing, handlers,
#     selection gestures, clipboard, primary selection, undo, secure entry and editor;
#   - Hangul through IBus, Fcitx5 and XIM on X11, gnome-shell's IBus over text-input-v3 and
#     Fcitx5 behind sway (input-method-v2); the input method choice;
#   - textprobe.py (report, edit) on the drawn field and on GtkEntry; Orca 46's speech over the
#     same keys for the field, switch and button against GtkEntry, GtkSwitch and GtkButton, on
#     X11 and on headless gnome-shell's Wayland with its Xwayland (which Orca 46 needs to start;
#     there it hears no key of a client window, so its own commands are left out).
#
# Stage TESTING (Option B on the drawn realization: skene_gui::testing reads through AT-SPI
# and acts through real input, XTest on X11, mutter's EIS and sway's virtual devices on
# Wayland; the display runner gives each test process a private session per leg):
#   - hello's build times, binary size and startup to first frame on Xvfb and headless mutter
#     (tools/testing/measure.sh, in a target directory of its own, removed after);
#   - every test of the workspace (the conformance suite, hello, notes, Simple, acme-rating,
#     the facade's own) by plain `cargo test` on the X11 leg (the default), the Wayland leg
#     (SKENE_TEST_SESSION=wayland: headless mutter, PipeWire) and the GUI crates on sway; and
#     by cargo-nextest (a process and a session per test) on X11 and Wayland;
#   - the GUI crates' tests five more times on each leg, failures counted;
#   - the runner never touches the user's session (tools/testing/isolation.sh: a stand-in
#     user Xvfb, headless sway and session bus set while hello's and notes' tests run on the
#     x11, wayland and sway legs, each watched for windows, views and bus connections, with
#     an app run on them as the positive control), and a test run outside the runner fails
#     before its body runs;
#   - the screenshots of hello and notes, light and dark, narrow and wide (screenshots/);
#   - an app made by `skene new` (its refusal without a runner beside it, plain cargo test,
#     skene check with an unrealized platform declared, skene test on both legs, skene build,
#     skene run on the runner's private Xvfb);
#   - the GTK prototype's numbers beside these ($GTK_PROTO/results/summary.txt).
#
# Stage PAINT (glyphs in the scene's order, gradients, motion on the frame clock, the GPU
# path): the scenarios and the facade's test run in every leg above; here, the GPU path
# (skene-gui-linux's `gpu` feature: vello_hybrid over wgpu's Vulkan, on Mesa's lavapipe here):
#   - clippy with warnings denied and rustdoc with the feature on;
#   - the realization's GPU tests (every conformance page painted on the GPU and then, the
#     device lost, on the CPU, compared; motion going on after the loss) on X11, mutter and
#     sway;
#   - the conformance suite and the facade's tests on the GPU path (SKENE_RENDERER=gpu) on X11
#     and mutter;
#   - full and partial repaints at 1920x1080 and 3840x2160 on both paths, on Xvfb and headless
#     mutter (tools/paint/repaint.sh).
#
# Stage SHELL GAPS (drag and drop, touch, kinetic scrolling, fractional scale, more
# compositors): the realization's driver tests (gestures::, dnd::, scale::) run in every leg
# above; here, the GUI crates and the realization's tests on the legs this stage adds:
#   - weston and KWin, each on its X11 backend inside a private Xvfb (XTest into weston, KWin's
#     fake input), and a window on that KWin's Xwayland, where the compositor starts (KWin from
#     SKENE_KWIN, or kwin_wayland on PATH);
#   - the scaled legs at 1.5 (SKENE_TEST_SCALE): sway's output scale and KWin's, and the
#     realization's own on Xvfb.
#
# Stage TESTING GAPS (every test through the driver, pixels as the screen shows them):
#   - the realization's tests are driver tests alone, run with the GUI crates on every leg
#     above, sway's included: nothing drives the window shell without its window;
#   - a screenshot and a pixel are read back from what the platform presented: the X server's
#     screen on X11, mutter's screen cast through PipeWire (WirePlumber beside it), wlroots'
#     screencopy on sway, the X screen weston and KWin show their output in;
#   - presented:: holds that a frame painted for a reader alone (a switch flipped through
#     AT-SPI) reaches the screen, and that a part of the window drawn again alone shows, pixel
#     for pixel, what the whole window drawn at once shows there; roles:: that each role a
#     reader finds is exactly Skene's (a switch, a row and an option told from a check box and
#     a list item by `xml-roles`).
#
# The GTK realization (gui-linux-gtk) and its probes (probes/) are kept, outside the
# workspace, for reference.
#
# Results go to results/ (summary.txt, logs, PNGs, JSON; results/shell/<scenario>/ for the
# shell, results/controls/<session>/ for the controls). The target directory is
# $CARGO_TARGET_DIR (target-testing by default), deleted at the end unless KEEP_TARGET=1.
# GTK_PROTO names the GTK prototype's directory (../gtk/proto unless set), whose cargo-nextest
# is borrowed where tools/bin has none and whose numbers are shown beside these.
# SKIP_CORE=1 skips the platform-text section, SKIP_SHELL=1 the shell's sessions,
# SKIP_FRAMES=1 the frame costs, SKIP_CONTROLS=1 the controls' sessions, SKIP_PAINT=1 the GPU
# path's section, SKIP_MEASURE=1 the measurements.
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
mkdir -p "$results" "$here/tools/bin" "$XDG_CACHE_HOME"
: > "$summary"

say() {
  echo "$*" | tee -a "$summary"
}

peak=0
measure_target() {
  local size
  size=$(du -sm "$CARGO_TARGET_DIR" 2>/dev/null | cut -f1 || echo 0)
  if [ "$size" -gt "$peak" ]; then peak=$size; fi
  echo "(target now ${size} MB)" >> "$summary"
}

say "== toolchain: $(cargo --version), $(rustc --version)"
say "== workspace members: $(cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(" ".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"])))')"

say "== the CLI and the runner"
cargo build -p skene --bins > "$results/cli-build.log" 2>&1
cp "$CARGO_TARGET_DIR/$triple/debug/skene" "$CARGO_TARGET_DIR/$triple/debug/skene-runner" tools/bin/
say "installed tools/bin/skene and tools/bin/skene-runner"
if [ ! -x tools/bin/cargo-nextest ] && [ -x "$gtk_proto/tools/bin/cargo-nextest" ]; then
  cp "$gtk_proto/tools/bin/cargo-nextest" tools/bin/
fi
export PATH="$here/tools/bin:$PATH"

if [ "${SKIP_MEASURE:-0}" != 1 ]; then
  say "== measurements: hello (its own target directory, removed after)"
  tools/testing/measure.sh "$here/target-measure" 2>&1 | tee "$results/measure.txt" | tee -a "$summary"
fi

say "== format, clippy, rustdoc"
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
measure_target

say "== tests: plain cargo test --workspace, every test binary through skene-runner"
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
started=$(date +%s%N)
cargo test --workspace --no-run > "$results/test-build.log" 2>&1
say "cargo test --workspace --no-run (building every test): $(( ($(date +%s%N) - started) / 1000000 )) ms"
measure_target
for leg in x11 wayland; do
  started=$(date +%s%N)
  if SKENE_TEST_SESSION=$leg cargo test --workspace --no-fail-fast > "$results/test-$leg.log" 2>&1; then
    outcome=passed
  else
    outcome=FAILED
  fi
  took=$(( ($(date +%s%N) - started) / 1000000 ))
  say "cargo test --workspace on the $leg leg: $outcome, $(grep -oE '[0-9]+ passed' "$results/test-$leg.log" | awk '{s+=$1} END {print s}') passed ($(untested "$results/test-$leg.log") of them testing nothing on the leg), $(grep -oE '[0-9]+ failed' "$results/test-$leg.log" | awk '{s+=$1} END {print s}') failed, ${took} ms wall (built before)"
  summarize "$results/test-$leg.log" | grep -vE ": ok\. 0 passed" | tee -a "$summary"
done
cp "$results/test-x11.log" "$results/test.log"
started=$(date +%s%N)
if SKENE_TEST_SESSION=sway cargo test --no-fail-fast -p skene-conformance -p hello -p notes-gui \
  -p skene-simple-gui -p skene-gui -p acme-rating -p skene-gui-linux > "$results/test-sway.log" 2>&1; then
  outcome=passed
else
  outcome=FAILED
fi
say "cargo test of the GUI crates and the realization on the sway leg: $outcome, $(grep -oE '[0-9]+ passed' "$results/test-sway.log" | awk '{s+=$1} END {print s}') passed ($(untested "$results/test-sway.log") of them testing nothing on the leg), $(grep -oE '[0-9]+ failed' "$results/test-sway.log" | awk '{s+=$1} END {print s}') failed, $(( ($(date +%s%N) - started) / 1000000 )) ms wall"
measure_target

say "== more legs: weston, KWin, KWin's Xwayland; the scaled legs (1.5)"
gui_crates="-p skene-conformance -p hello -p notes-gui -p skene-simple-gui -p skene-gui -p acme-rating -p skene-gui-linux"
legs_run() { # <log name> <leg> [scale]
  local log="$results/test-$1.log" outcome
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
  say "cargo test of the GUI crates and the realization on the $1 leg: $outcome, $(grep -oE '[0-9]+ passed' "$log" | awk '{s+=$1} END {print s}') passed ($(untested "$log") of them testing nothing on the leg), $(grep -oE '[0-9]+ failed' "$log" | awk '{s+=$1} END {print s}') failed, $(( ($(date +%s%N) - started) / 1000000 )) ms wall"
  grep -hE "^  .*: not tested here" "$log" | sed -E 's/^.*not tested here: /  not tested here: /' | sort | uniq -c | tee -a "$summary"
}
for leg in weston kwin kwin-x11; do
  legs_run "$leg" "$leg"
done
for leg in sway kwin x11; do
  legs_run "$leg-1.5" "$leg" 1.5
done
measure_target

say "== cargo nextest run --workspace: a process and a session per test"
say "$(cargo nextest --version 2>&1 | head -1)"
for leg in x11 wayland; do
  started=$(date +%s%N)
  SKENE_TEST_SESSION=$leg cargo nextest run --workspace --no-fail-fast > "$results/nextest-$leg.log" 2>&1 || true
  say "nextest on the $leg leg: $(grep -E 'Summary' "$results/nextest-$leg.log" | sed 's/^ *//') ($(( ($(date +%s%N) - started) / 1000000 )) ms wall incl. listing)"
  grep -E '^ +(PASS|FAIL) \[' "$results/nextest-$leg.log" | sort -t'[' -k2 -rn | sed -n "1,5s/^ */  slowest ($leg): /p" | tee -a "$summary"
done

say "== steadiness: the GUI crates' tests five more times on each leg"
for leg in x11 wayland; do
  rounds=""
  for round in 1 2 3 4 5; do
    SKENE_TEST_SESSION=$leg cargo test --no-fail-fast -p skene-conformance -p hello -p notes-gui \
      -p skene-simple-gui -p skene-gui -p acme-rating --tests > "$results/repeat-$leg-$round.log" 2>&1 || true
    rounds="$rounds $(grep -oE '[0-9]+ passed' "$results/repeat-$leg-$round.log" | awk '{s+=$1} END {print s}')/$(grep -oE '[0-9]+ failed' "$results/repeat-$leg-$round.log" | awk '{s+=$1} END {print s}')"
  done
  say "$leg: passed/failed per round:$rounds"
done
say "input the driver sent again, over every run above: clicks aimed again (a window not where it was found) $(cat "$results"/test-*.log "$results"/nextest-*.log "$results"/repeat-*.log | grep -c 'aiming again' || true), keys sent again (a keyboard not yet set up) $(cat "$results"/test-*.log "$results"/nextest-*.log "$results"/repeat-*.log | grep -c 'sending them again' || true)"

say "== the runner never touches the user's session: a stand-in user X display, Wayland compositor and session bus set, every leg"
tools/testing/isolation.sh "$results/isolation" hello notes-gui 2>&1 | tee "$results/isolation.txt" | tee -a "$summary"
direct=$(cargo test -p skene-gui --test it --no-run 2>&1 | sed -nE 's/^ *Executable tests\/it\/main.rs \((.*)\)$/\1/p' | head -1)
say "a test run outside the runner: $("$direct" 2>&1 | grep -oE 'a test runs in the display runner.s private session[^;]*' | head -1)"

say "== the screenshots (screenshots/)"
rm -rf screenshots
mkdir -p screenshots/mutter
SKENE_SCREENSHOTS="$here/screenshots" cargo test -p hello -p notes-gui --test screenshots > "$results/screenshots.log" 2>&1 || true
SKENE_TEST_SESSION=wayland SKENE_SCREENSHOTS="$here/screenshots/mutter" \
  cargo test -p hello -p notes-gui --test screenshots >> "$results/screenshots.log" 2>&1 || true
say "x11: $(cd screenshots && ls *.png | tr '\n' ' ')"
say "mutter: $(cd screenshots/mutter && ls *.png | tr '\n' ' ')"
measure_target

# The workspace's test executables are not run again: they go, to keep the target small.
find "$CARGO_TARGET_DIR/$triple/debug/deps" -maxdepth 1 -type f -perm -u+x -regex '.*-[0-9a-f]\{16\}$' -delete

say "== an app made by skene new"
apps="$(dirname "$here")/next-apps"
rm -rf "$apps"
mkdir -p "$apps/lonely"
cp tools/bin/skene "$apps/lonely/"
(cd "$apps" && say "without a runner beside skene: $(lonely/skene new nope 2>&1 | head -c 160 || true)")
(cd "$apps" && "$here/tools/bin/skene" new greeter | tee -a "$summary")
(
  cd "$apps/greeter" || exit 1
  # The app builds in a target of its own, removed after: its dependencies resolve apart from
  # the workspace's.
  export CARGO_TARGET_DIR="$here/target-greeter"
  started=$(date +%s%N)
  cargo test > "$results/new-test.log" 2>&1 || true
  say "plain cargo test, no DISPLAY: $(grep -oE '[0-9]+ passed' "$results/new-test.log" | awk '{s+=$1} END {print s}') passed, $(grep -oE '[0-9]+ failed' "$results/new-test.log" | awk '{s+=$1} END {print s}') failed ($(( ($(date +%s%N) - started) / 1000000 )) ms, its dependencies shared with the workspace's target)"
  code=0
  "$here/tools/bin/skene" check > "$results/new-check.log" 2>&1 || code=$?
  say "skene check (linux): exit $code: $(grep '^skene check' "$results/new-check.log" | tr '\n' ' ')"
  printf 'platforms = ["linux", "ios"]\n' > skene.toml
  code=0
  "$here/tools/bin/skene" check > "$results/new-check-ios.log" 2>&1 || code=$?
  say "skene check (linux, ios): exit $code: $(grep '^skene check' "$results/new-check-ios.log" | tr '\n' ' ')"
  printf 'platforms = ["linux"]\n' > skene.toml
  "$here/tools/bin/skene" test > "$results/new-skene-test.log" 2>&1 || true
  say "skene test (x11): $(grep -oE '[0-9]+ passed' "$results/new-skene-test.log" | awk '{s+=$1} END {print s}') passed"
  SKENE_TEST_SESSION=wayland "$here/tools/bin/skene" test > "$results/new-skene-test-wayland.log" 2>&1 || true
  say "skene test (wayland): $(grep -oE '[0-9]+ passed' "$results/new-skene-test-wayland.log" | awk '{s+=$1} END {print s}') passed"
  ("$here/tools/bin/skene" build linux 2>/dev/null || true) | grep '^skene build' | tee -a "$summary" || true
  rm -f "$results/greeter-run.png"
  SKENE_SHOT="$results/greeter-run.png" timeout 20 "$here/tools/bin/skene" run > /dev/null 2>&1 || true
  say "skene run, no DISPLAY (the runner's private Xvfb): $(python3 -I -c 'import sys; from PIL import Image; print("frame", Image.open(sys.argv[1]).size)' "$results/greeter-run.png" 2>&1)"
  say "the app's own target: $(du -sm "$CARGO_TARGET_DIR" | cut -f1) MB, removed"
  rm -rf "$CARGO_TARGET_DIR"
)
rm -rf "$apps"
measure_target

say "== beside the GTK prototype ($gtk_proto/results/summary.txt)"
gtk=$gtk_proto/results/summary.txt
if [ -f "$gtk" ]; then
  grep -E '^test totals|^nextest: Summary|^  GTK tests|^clean cargo|^the first cargo check|^a one-line change|^release hello|^startup, round' "$gtk" | sed 's/^/  GTK: /' | tee -a "$summary"
fi

if [ "${SKIP_CORE:-0}" != 1 ]; then
say "== the platform-text cases"
cargo build --release -p skene-parley --example verify > "$results/verify-build.log" 2>&1
verify="$CARGO_TARGET_DIR/$triple/release/examples/verify"
"$verify" shots "$results/shots" > "$results/shots.txt"
say "verify shots: $(grep -c '^==' "$results/shots.txt") cases (results/shots.txt, results/shots/*.png)"
python3 -I tools/pango-compare.py "$results/shots" "$results/pango" | tee "$results/pango-compare.txt" | tee -a "$summary"

say "== emoji in colour"
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
print(f"chromatic glyph groups: pango {pango[0]} (image {pango[1]}px wide), skene {skene[0]} ({skene[1]}px wide)")
EOF

say "== fontconfig rendering settings"
for config in none mono fullhint; do
  result=$(XDG_CONFIG_HOME="$here/tools/fc/$config" "$verify" settings "$results/settings-$config")
  say "user fonts.conf '$config': $result"
done
plain="$here/tools/fc/plain/fonts.conf"
say "plain configuration (no rendering rule), no desktop values: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" settings "$results/settings-plain")"
say "plain + desktop antialias=0 hinting=1 hintfull: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" settings "$results/settings-plain-desktop" 0,1,3)"
say "plain + desktop antialias=1 hinting=0 hintnone + user hintfull rule: $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/fullhint "$verify" settings "$results/settings-plain-user" 1,0,0)"
say "a desktop change after the first paint, one painter (plain configuration): $(FONTCONFIG_FILE=$plain XDG_CONFIG_HOME=$here/tools/fc/none "$verify" follow | head -1)"
say "rules keyed on the request's size and weight (tools/fc/keyed: no antialiasing below 12 px or bold): $(XDG_CONFIG_HOME=$here/tools/fc/keyed "$verify" follow | tail -3 | tr '\n' ';')"

say "== measured size against drawn ink"
"$verify" ink | tee "$results/ink.txt" | tail -1 | tee -a "$summary"

say "== 1000 labels"
"$verify" bench | tee "$results/bench.txt" | tee -a "$summary"
measure_target
fi

say "== the shell: tools and apps"
# The tests' build goes first: the shell's tools and probes build in a target of their own size.
rm -rf "${CARGO_TARGET_DIR:?}"
say "the tests' target removed before the shell's builds"
cargo build -p skene-shell-tools --bins > "$results/shell-tools-build.log" 2>&1
for tool in drive clip portal-stub xsettings-manager; do
  cp "$CARGO_TARGET_DIR/$triple/debug/$tool" tools/bin/
done
cargo build -p skene-shell-probes --bins > "$results/shell-probes-build.log" 2>&1
cargo build --release -p hello -p skene-shell-probes --bins > "$results/shell-release-build.log" 2>&1
say "built tools/bin/{drive,clip,portal-stub,xsettings-manager}, hello (release), the probes"
say "the input tools outside a private session (DISPLAY=:0 set, no session marker): drive: $(DISPLAY=:0 tools/bin/drive x11 key a 2>&1 | head -1); harness.py: $(DISPLAY=:0 OUT=/nonexistent python3.12 -I tools/controls/harness.py focus 2>&1 | head -1); orca.sh: $(DISPLAY=:0 tools/controls/orca/orca.sh 2>&1 | head -1)"
measure_target

if [ "${SKIP_SHELL:-0}" != 1 ]; then
say "== the shell: sessions"
rm -rf "$results/shell"
tools/sessions/shell-checks.sh "$CARGO_TARGET_DIR/$triple/debug" "$CARGO_TARGET_DIR/$triple/release" \
  "$results/shell" | tee "$results/shell-checks.txt" | tee -a "$summary"
say "shell checks: $(grep -c '\] PASS' "$results/shell-checks.txt" || true) passed, $(grep -c '\] FAIL' "$results/shell-checks.txt" || true) failed"
measure_target
fi

if [ "${SKIP_FRAMES:-0}" != 1 ]; then
say "== the frame costs (tools/bench/frames.sh: five runs a leg)"
rm -rf "$results/frames"
tools/bench/frames.sh "$CARGO_TARGET_DIR/$triple/release" "$results/frames" 5 \
  | tee "$results/frames.txt" | tee -a "$summary"
fi

if [ "${SKIP_CONTROLS:-0}" != 1 ]; then
say "== the controls: sessions"
rm -rf "$results/controls"
tools/controls/checks.sh "$CARGO_TARGET_DIR/$triple/debug/controls" "$results/controls" \
  | tee "$results/controls-checks.txt" | grep -E 'FAIL|DONE|detect|textprobe|\[orca\]' | tee -a "$summary"
say "controls checks: $(grep -c '\] PASS' "$results/controls-checks.txt" || true) passed, $(grep -c '\] FAIL' "$results/controls-checks.txt" || true) failed"
measure_target
fi

if [ "${SKIP_PAINT:-0}" != 1 ]; then
say "== the GPU path (skene-gui-linux's gpu feature)"
mkdir -p "$results/paint/cpu" "$results/paint/gpu"
cp "$CARGO_TARGET_DIR/$triple/release/rows" "$results/paint/cpu/"
if cargo clippy -p skene-gui-linux -p skene-gui -p skene-conformance --features skene-gui-linux/gpu \
  --all-targets -- -D warnings > "$results/clippy-gpu.log" 2>&1; then
  say "cargo clippy with the gpu feature, -D warnings: passed"
else
  say "cargo clippy with the gpu feature: FAILED (results/clippy-gpu.log)"
fi
cargo doc -p skene-gui-linux --features gpu --no-deps > "$results/doc-gpu.log" 2>&1
say "cargo doc -p skene-gui-linux --features gpu: $(grep -cE '^(warning|error)' "$results/doc-gpu.log" || true) warnings or errors"
for leg in x11 wayland sway; do
  SKENE_TEST_SESSION=$leg SKENE_TRACE=1 cargo test -p skene-gui-linux --features gpu --test it gpu:: -- --nocapture \
    > "$results/test-gpu-$leg.log" 2>&1 || true
  say "the GPU tests on the $leg leg: $(grep -E '^test result' "$results/test-gpu-$leg.log" | tail -1); $(grep -E '^\[gpu\] .*, Vulkan\)' "$results/test-gpu-$leg.log" | sort -u | head -1)"
  grep -E '^[a-z]+: most [0-9]+ apart' "$results/test-gpu-$leg.log" | awk '{ if ($3 > most) most = $3; apart += $5 } END { printf "  every page: a channel at most %d apart, %d pixels beyond 2 in all\n", most, apart }' | tee -a "$summary"
done
for leg in x11 wayland; do
  SKENE_TEST_SESSION=$leg SKENE_RENDERER=gpu cargo test --no-fail-fast -p skene-conformance \
    -p skene-gui-linux --features skene-gui-linux/gpu --test linux > "$results/test-on-gpu-$leg.log" 2>&1 || true
  SKENE_TEST_SESSION=$leg SKENE_RENDERER=gpu cargo test --no-fail-fast -p skene-gui \
    --features skene-gui-linux/gpu --test it >> "$results/test-on-gpu-$leg.log" 2>&1 || true
  say "the conformance suite and the facade's tests on the GPU path, $leg leg: $(grep -oE '[0-9]+ passed' "$results/test-on-gpu-$leg.log" | awk '{s+=$1} END {print s}') passed, $(grep -oE '[0-9]+ failed' "$results/test-on-gpu-$leg.log" | awk '{s+=$1} END {print s}') failed"
done
cargo build --release -p skene-shell-probes -p skene-gui-linux --features skene-gui-linux/gpu --bin rows \
  > "$results/paint/build.log" 2>&1
cp "$CARGO_TARGET_DIR/$triple/release/rows" "$results/paint/gpu/"
say "rows, release: $(du -k "$results/paint/cpu/rows" | cut -f1) KB without the GPU path, $(du -k "$results/paint/gpu/rows" | cut -f1) KB with it"
tools/paint/repaint.sh "$results/paint/cpu/rows" "$results/paint/gpu/rows" "$results/paint/repaint" 3 \
  | tee "$results/paint/repaint.txt" | tee -a "$summary"
rm -rf "$results/paint/cpu" "$results/paint/gpu"
measure_target
fi

say "== target peak: ${peak} MB"
if [ "${KEEP_TARGET:-0}" != 1 ]; then
  rm -rf "$CARGO_TARGET_DIR"
  say "deleted $CARGO_TARGET_DIR"
fi
rm -rf "$XDG_CACHE_HOME"
