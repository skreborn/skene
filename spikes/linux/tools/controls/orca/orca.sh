#!/usr/bin/env bash
# Inside tools/controls/session.sh x11 (APP=none): a private speech-dispatcher whose only
# output module writes each utterance to spoken.log, Orca 46 on it (default settings, its own
# prefs dir), then the app (ORCA_APP, its window named ORCA_WINDOW); the same real XTest key
# sequence over a field, a switch and a button, each step marked in spoken.log.
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

step "app starts; window shown, field focused"
env ${ORCA_APP_ENV:-} "$ORCA_APP" >"$OUT/app.log" 2>&1 &
APP_PID=$!
WIN=$(timeout 15 xdotool search --sync --name "^$ORCA_WINDOW\$" | head -1)
echo "window $WIN"
xdotool windowfocus --sync "$WIN"
sleep 1
if [ "$ORCA_WINDOW" = controls ]; then
  # The drawn app focuses nothing on its own: a click on the field, as a user would.
  ATSPI_WINDOW=controls python3.12 -I "$root/tools/controls/harness.py" focus >/dev/null 2>&1
fi
sleep 1
key() { xdotool key --delay 150 "$@"; }
step "type 'Hi there'"
xdotool type --delay 200 "Hi there"
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
step "Orca where-am-i (KP_Enter)"; key KP_Enter
step "Orca flat review: current line (KP_Up)"; key KP_Up
step "Orca flat review: current word (KP_Begin)"; key KP_Begin
step "Orca flat review: current character (KP_Down)"; key KP_Down
step "Orca flat review off (KP_Subtract)"; key KP_Subtract
step "Tab to the switch"; key Tab
step "Space: switch on"; key space
step "Space: switch off"; key space
step "Tab to the button"; key Tab
step "Return: press"; key Return
step "Space: press"; key space
import -window root -crop 560x340+0+0 "$OUT/screen.png" 2>/dev/null
step "Shift+Tab Shift+Tab back to the field"; key shift+Tab shift+Tab
step "end"
sleep 1
stop_speech
kill $APP_PID 2>/dev/null
wait $APP_PID 2>/dev/null
cat "$SPOKEN"
