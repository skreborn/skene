#!/usr/bin/env bash
# The display runner never touches the user's session, on any leg: a stand-in user session (an
# Xvfb on a watched display, a headless sway as the user's Wayland compositor with its socket
# and SWAYSOCK, a session bus watched by dbus-monitor) is set in the environment while the GUI
# tests run on the x11, wayland and sway legs; no window may appear on the watched display, no
# view in the sway, no connection on the bus. Positive controls: hello run as an app (not a
# test) reaches each of them.
#
#   tools/testing/isolation.sh <out dir> [package...]
#
# Prints one line per leg and one per check; run from the workspace, its CARGO_TARGET_DIR set.
set -uo pipefail
. "$(cd "$(dirname "$0")" && pwd)/../children.sh"
out=$1
shift
packages=("$@")
[ ${#packages[@]} -gt 0 ] || packages=(hello notes-gui)
mkdir -p "$out"
out=$(cd "$out" && pwd)
unset DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS SWAYSOCK
# Short: a Wayland socket's path is limited to 108 bytes.
user=$(mktemp -d /tmp/skiso-XXXX)
mkdir -p "$user/runtime" && chmod 700 "$user/runtime"

xnum=$((700 + RANDOM % 200))
Xvfb ":$xnum" -screen 0 1024x768x24 -nolisten tcp > "$out/user-xvfb.log" 2>&1 &
xvfb=$!
printf 'output HEADLESS-1 resolution 1024x768\n' > "$user/sway.conf"
XDG_RUNTIME_DIR=$user/runtime WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
  WLR_RENDERER=pixman sway --unsupported-gpu -c "$user/sway.conf" > "$out/user-sway.log" 2>&1 &
sway=$!
dbus-daemon --session --nofork --nopidfile --print-address=1 > "$user/bus" 2> "$out/user-bus.log" &
bus=$!
for _ in $(seq 100); do
  [ -s "$user/bus" ] && ls "$user/runtime"/sway-ipc.* > /dev/null 2>&1 && break
  sleep 0.1
done
busaddr=$(head -1 "$user/bus")
wsock=$(ls "$user/runtime" | grep -E '^wayland-[0-9]+$' | head -1)
swaysock=$(ls "$user/runtime"/sway-ipc.* | head -1)
echo "stand-in user session: DISPLAY=:$xnum, WAYLAND_DISPLAY=$wsock (headless sway), a session bus"

# Every connection to the user's bus, as the bus announces it.
dbus-monitor --address "$busaddr" "type='signal',member='NameOwnerChanged'" \
  > "$out/user-bus-monitor.log" 2>&1 &
monitor=$!
sleep 0.5
connections() {
  grep -cE '^ +string ":1\.[0-9]+"$' "$out/user-bus-monitor.log" || true
}
baseline=$(connections)

views() {
  SWAYSOCK=$swaysock swaymsg -t get_tree 2>/dev/null | python3 -I -c 'import json, sys
def walk(node):
  count = 1 if node.get("pid") else 0
  for key in ("nodes", "floating_nodes"):
    for child in node.get(key, []):
      count += walk(child)
  return count
print(walk(json.load(sys.stdin)))' 2>/dev/null || echo err
}

# One line per poll: the X windows on the watched display, the views in the sway.
flag=$(mktemp)
polls="$out/polls.txt"
: > "$polls"
( while [ -f "$flag" ]; do
    echo "$( (xwininfo -display ":$xnum" -root -children 2>/dev/null || true) | grep -c '"' || true) $(views)" >> "$polls"
    sleep 0.1
  done ) &
watcher=$!

args=()
for package in "${packages[@]}"; do args+=(-p "$package"); done
export XDG_RUNTIME_DIR=$user/runtime DISPLAY=:$xnum WAYLAND_DISPLAY=$wsock \
  DBUS_SESSION_BUS_ADDRESS=$busaddr SWAYSOCK=$swaysock
for leg in x11 wayland sway; do
  SKENE_TEST_SESSION=$leg cargo test --no-fail-fast "${args[@]}" --tests > "$out/tests-$leg.log" 2>&1
  echo "leg $leg (${packages[*]}): $(grep -oE '[0-9]+ passed' "$out/tests-$leg.log" | awk '{s+=$1} END {print s+0}') passed, $(grep -oE '[0-9]+ failed' "$out/tests-$leg.log" | awk '{s+=$1} END {print s+0}') failed"
done
rm -f "$flag"
wait $watcher
during=$(( $(connections) - baseline ))
echo "during the tests: $(wc -l < "$polls") polls; with an X window on :$xnum: $(awk '$1 > 0' "$polls" | wc -l); with a sway view: $(awk '$2 != "0"' "$polls" | wc -l); connections to the user's bus: $during"

# Positive controls: an app is run on the user's session.
SKENE_TITLE=positive-x11 SKENE_WINDOW_SYSTEM=x11 timeout 8 cargo run -q -p hello > "$out/positive-x11.log" 2>&1 &
app=$!
sleep 6
x11=$(xwininfo -display ":$xnum" -root -tree 2>/dev/null | grep -c 'positive-x11' || true)
wait $app
SKENE_TITLE=positive-wayland SKENE_WINDOW_SYSTEM=wayland timeout 8 cargo run -q -p hello > "$out/positive-wayland.log" 2>&1 &
app=$!
sleep 6
wayland=$(views)
wait $app
echo "positive controls, hello run as an app: windows on :$xnum $x11, sway views $wayland, connections to the user's bus $(( $(connections) - baseline - during ))"

signal_children TERM "$monitor" "$sway" "$xvfb" "$bus"
wait 2> /dev/null
rm -rf "$user"
