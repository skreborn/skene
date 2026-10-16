# The keyboard on X11: the same physical key under the US and German layouts (XKB reloaded on
# the server's change), and a dead key composed through the locale's compose table.
. "$(dirname "$0")/common.sh"
W=$(x11_window states)
xdotool windowfocus "$W" 2>/dev/null
sleep 0.2
"$TOOLS/drive" x11 keycode 21 >/dev/null
setxkbmap de; sleep 0.4
"$TOOLS/drive" x11 keycode 21 >/dev/null
setxkbmap us; sleep 0.3
xdotool key --delay 60 dead_acute e
sleep 0.3
expect '[key] "y" "KeyY" down' "the Y key types y under the US layout"
expect '[key] "z" "KeyY" down' "the Y key types z under the German layout"
expect '[key] "é" "KeyE" down' "a dead acute and e compose to é"
