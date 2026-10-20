# Hello on KWin (its virtual backend, its fake input): server-side decorations negotiated
# through xdg-decoration, so Skene draws no title bar, shadow or resize band, and its frame is
# the window's geometry alone; the window is activated, and real input reaches its content.
. "$(dirname "$0")/common.sh"
DRIVE="$TOOLS/drive kwin"
shot hello-kwin 0.6
expect "decorations Server" "xdg-decoration negotiates server-side decorations"
size hello-kwin 480 640
pixel hello-kwin 4 4 f2f2f7
pixel hello-kwin 30 100 ffffff
expect "states WindowState(ACTIVATED" "KWin activates the window"
$DRIVE calibrate click-at 240 120 sleep 400 >/dev/null 2>&1
if grep -q "^\[button\] 0 down" "$LOG"; then pass "$KIND: a press lands in the content (no title bar above it)"; else fail "$KIND: a press lands in the content (no title bar above it)"; fi
absent "request Move\|request Resize" "nothing asks the compositor to move or resize"
