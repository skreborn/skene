# Hello on mutter: no xdg-decoration, so Skene draws the title bar, the shadow and the resize
# band; real input through mutter's RemoteDesktop moves the window by its title bar, resizes it
# from its corner, maximizes and restores it by double-clicks, and closes it by its button.
. "$(dirname "$0")/common.sh"
DRIVE="$TOOLS/drive mutter-dbus"
shot hello-mutter 0.6
size hello-mutter 528 688
expect "decorations Client" "mutter leaves the decorations to the client"
pixel hello-mutter 30 34 ebebeb
first=$($DRIVE calibrate 2>/dev/null | grep -o "measured origin Some((.*))")
$DRIVE calibrate drag 150 18 250 98 sleep 400 >/dev/null 2>&1
moved=$($DRIVE calibrate 2>/dev/null | grep -o "measured origin Some((.*))")
echo "INFO $KIND: window origin $first then $moved"
if [ -n "$first" ] && [ -n "$moved" ] && [ "$first" != "$moved" ]; then pass "$KIND: a drag on the title bar moves the window"; else fail "$KIND: a drag on the title bar moves the window"; fi
expect "request Move" "the title-bar drag asks for xdg_toplevel.move"
$DRIVE calibrate drag 486 646 606 706 sleep 600 >/dev/null 2>&1
shot hello-mutter-resized 0.4
expect "request Resize(BottomRight)" "a press in the corner's band asks for xdg_toplevel.resize"
expect "configure 600x700 decorations Client" "the resize lays the window out at 600x700"
size hello-mutter-resized 648 748
$DRIVE calibrate double-click-at 200 18 sleep 700 >/dev/null 2>&1
shot hello-mutter-maximized 0.3
expect "states WindowState(MAXIMIZED" "a double-click on the title bar maximizes"
size hello-mutter-maximized 1280 1024
pixel hello-mutter-maximized 30 140 f2f2f7
pixel hello-mutter-maximized 60 140 ffffff
$DRIVE calibrate double-click-at 200 18 sleep 700 >/dev/null 2>&1
shot hello-mutter-restored 0.3
size hello-mutter-restored 648 748
$DRIVE calibrate click-at 573 19 sleep 600 >/dev/null 2>&1
expect "request Close" "the drawn close button closes"
if kill -0 "$APP_PID" 2>/dev/null; then fail "$KIND: the app quit"; else pass "$KIND: the app quit"; fi
