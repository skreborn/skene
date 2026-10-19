#!/usr/bin/env bash
# Inside tools/controls/session.sh (APP=none): a private speech-dispatcher whose only output
# module writes each utterance to spoken.log, Orca 46 on it (default settings, its own prefs
# dir), then the app (ORCA_APP, its window named ORCA_WINDOW); the same real key sequence over
# a field, a switch and a button, each step marked in spoken.log: XTest on X11, the driver's
# keys (tools/bin/drive "$DRIVE") on Wayland, where Orca 46 needs Xwayland (it compiles the
# keymap of DISPLAY as it starts: session.sh's XWAYLAND=1).
set -u
# Real input goes only to a session tools/controls/session.sh made.
if [ -z "${SKENE_PRIVATE_SESSION:-}" ] || [ "$SKENE_PRIVATE_SESSION" != "${XDG_RUNTIME_DIR:-}" ] \
  || [ ! -d "$SKENE_PRIVATE_SESSION" ]; then
  echo "orca.sh: refusing to inject input outside a private session (run it through tools/controls/session.sh)" >&2
  exit 2
fi
B=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$B/../../.." && pwd)
. "$B/speech.sh"
start_speech

step "orca starts"
start_orca

if [ "${DRIVE:-x11}" = x11 ]; then
  key() { xdotool key --delay 150 "$@"; }
  typed() { xdotool type --delay 200 "$1"; }
else
  # The driver's keys at a hand's pace: a chord held as a hand holds it.
  key() {
    local steps=() name part chord
    for name in "$@"; do
      if [[ $name == *+* ]]; then
        chord=""
        IFS=+ read -ra parts <<<"$name"
        for part in "${parts[@]}"; do
          case $part in shift) part=Shift_L ;; ctrl) part=Control_L ;; alt) part=Alt_L ;; esac
          chord+="${chord:++}$part"
        done
        steps+=(chord "$chord")
      else
        steps+=(key "$name")
      fi
      steps+=(sleep 150)
    done
    "$root/tools/bin/drive" "$DRIVE" "${steps[@]}" >>"$OUT/drive.log" 2>&1
  }
  typed() {
    local steps=() index
    for ((index = 0; index < ${#1}; index++)); do
      steps+=(type "${1:index:1}" sleep 200)
    done
    "$root/tools/bin/drive" "$DRIVE" "${steps[@]}" >>"$OUT/drive.log" 2>&1
  }
fi
step "app starts; window shown, field focused"
env ${ORCA_APP_ENV:-} "$ORCA_APP" >"$OUT/app.log" 2>&1 &
APP_PID=$!
if [ "${DRIVE:-x11}" = x11 ]; then
  WIN=$(timeout 15 xdotool search --sync --name "^$ORCA_WINDOW\$" | head -1)
  echo "window $WIN"
fi
# Either app takes the X focus itself as it maps (there is no window manager), GTK 4.14 before
# its objects are on the accessibility bus, so Orca drops its first events as from dead objects:
# once a reader finds the window, the focus goes to the root window and back, a live activation
# for either. On Wayland the compositor activates the window as it maps.
ATSPI_WINDOW=$ORCA_WINDOW timeout 20 python3.12 -I "$root/tools/controls/atspi.py" dump >/dev/null 2>&1
sleep 1
step "window activated once its objects are on the bus"
if [ "${DRIVE:-x11}" = x11 ]; then
  xdotool windowfocus --sync "$(xwininfo -root | awk '/Window id:/ { print $4 }')"
  sleep 1
  xdotool windowfocus --sync "$WIN"
fi
sleep 1
# A click on the field, as a user would: the drawn app's field asks for focus as the window
# opens (set_autofocus) and GtkEntry has it as GtkWindow's first focusable, so for both it is
# a press on what holds focus already.
ATSPI_WINDOW=$ORCA_WINDOW python3.12 -I "$root/tools/controls/harness.py" focus >/dev/null 2>&1
sleep 1
step "type 'Hi there'"
typed "Hi there"
step "Home"; key Home
step "Right"; key Right
step "Right"; key Right
step "Ctrl+Right"; key ctrl+Right
step "Ctrl+Right"; key ctrl+Right
step "Ctrl+Left"; key ctrl+Left
step "Shift+Right"; key shift+Right
step "Shift+Right"; key shift+Right
step "Shift+Ctrl+Right"; key shift+ctrl+Right
step "Left (collapses the selection)"; key Left
step "End"; key End
step "BackSpace"; key BackSpace
if [ "${DRIVE:-x11}" = x11 ]; then
  step "Orca where-am-i (KP_Enter)"; key KP_Enter
  step "Orca flat review: current line (KP_Up)"; key KP_Up
  step "Orca flat review: current word (KP_Begin)"; key KP_Begin
  step "Orca flat review: current character (KP_Down)"; key KP_Down
  step "Orca flat review off (KP_Subtract)"; key KP_Subtract
else
  # Orca 46 hears keys on Wayland only from the registry's device event controller, which a
  # client window feeds none of (GTK 4's neither): its own commands would reach the app as keys.
  step "Orca's own commands: none on Wayland, where Orca 46 hears no key of a client window"
fi
step "Tab to the switch"; key Tab
step "Space: switch on"; key space
step "Space: switch off"; key space
step "Tab to the button"; key Tab
step "Return: press"; key Return
step "Space: press"; key space
[ "${DRIVE:-x11}" = x11 ] && import -window root -crop 560x340+0+0 "$OUT/screen.png" 2>/dev/null
step "Shift+Tab Shift+Tab back to the field"; key shift+Tab shift+Tab
step "end"
sleep 1
stop_speech
kill $APP_PID 2>/dev/null
wait $APP_PID 2>/dev/null
cat "$SPOKEN"
