# The keyboard on X11: the same physical key under the US and German layouts (XKB reloaded on
# the server's change), and a dead key composed through the locale's compose table. Each layout
# changes once the app has taken the key before it, as a user's would: an X11 client reads the
# keymap in force when it handles a change, not the one a key queued before it was pressed under.
. "$(dirname "$0")/common.sh"
W=$(x11_window states)
xdotool windowfocus --sync "$W" 2>/dev/null
"$TOOLS/drive" x11 keycode 21 >/dev/null; await '"KeyY" up'
setxkbmap de; await '[x11] keymap layouts ["German"]'
"$TOOLS/drive" x11 keycode 21 >/dev/null; await '"KeyY" up' '[x11] keymap layouts ["German"]'
setxkbmap us; await '[x11] keymap layouts ["English (US)"]' '[x11] keymap layouts ["German"]'
xdotool key --delay 60 dead_acute e
await '[key] "é" "KeyE" down'
expect '[key] "y" "KeyY" down' "the Y key types y under the US layout"
expect '[key] "z" "KeyY" down' "the Y key types z under the German layout"
expect '[key] "é" "KeyE" down' "a dead acute and e compose to é"
