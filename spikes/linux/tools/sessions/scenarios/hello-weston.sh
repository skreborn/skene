# Hello on weston (its X11 backend, XTest into weston's output window): weston offers no
# xdg-decoration, so Skene draws the title bar, the shadow and the resize band; real input moves
# the window by its title bar (up and left, where weston placed it low and right), resizes it
# from its corner, maximizes and restores it by double-clicks, and closes it by its button.
# Weston answers a restore with the maximized size and no maximized state, a hint Skene takes
# as GTK does (it uses a size of its own only where the hint is 0 by 0).
. "$(dirname "$0")/common.sh"
DRIVE="$TOOLS/drive x11"
shot hello-weston 0.6
size hello-weston 528 688
expect "decorations Client" "weston leaves the decorations to the client"
pixel hello-weston 30 34 ebebeb
first=$($DRIVE calibrate 2>/dev/null | grep -o "measured origin Some((.*))")
$DRIVE calibrate drag 150 18 -350 -232 sleep 400 >/dev/null 2>&1
moved=$($DRIVE calibrate 2>/dev/null | grep -o "measured origin Some((.*))")
echo "INFO $KIND: window origin $first then $moved"
if [ -n "$first" ] && [ -n "$moved" ] && [ "$first" != "$moved" ]; then pass "$KIND: a drag on the title bar moves the window"; else fail "$KIND: a drag on the title bar moves the window"; fi
expect "request Move" "the title-bar drag asks for xdg_toplevel.move"
$DRIVE calibrate drag 486 646 606 706 sleep 600 >/dev/null 2>&1
shot hello-weston-resized 0.4
expect "request Resize(BottomRight)" "a press in the corner's band asks for xdg_toplevel.resize"
expect "configure 600x700 decorations Client" "the resize lays the window out at 600x700"
size hello-weston-resized 648 748
$DRIVE calibrate double-click-at 200 18 sleep 700 >/dev/null 2>&1
shot hello-weston-maximized 0.3
expect "states WindowState(MAXIMIZED" "a double-click on the title bar maximizes"
maximized=$(grep -o "configure [0-9]*x[0-9]* decorations Client states WindowState(MAXIMIZED" "$LOG" | tail -1 | grep -o "[0-9]*x[0-9]*")
echo "INFO $KIND: maximized to $maximized (weston's panel keeps its strip)"
size hello-weston-maximized "${maximized%x*}" "${maximized#*x}"
pixel hello-weston-maximized 30 140 f2f2f7
pixel hello-weston-maximized 60 140 ffffff
$DRIVE calibrate double-click-at 200 18 sleep 700 >/dev/null 2>&1
shot hello-weston-restored 0.3
restored=$(grep -o "configure [0-9]*x[0-9]* decorations Client states WindowState(ACTIVATED)" "$LOG" | tail -1 | grep -o "[0-9]*x[0-9]*")
echo "INFO $KIND: restored to $restored"
if grep -q "request ToggleMaximize" "$LOG" && [ "$(grep -c "states WindowState(MAXIMIZED" "$LOG")" -ge 1 ] && \
  [ "$(grep -o "configure .* states WindowState([A-Z |]*)" "$LOG" | tail -1 | grep -c MAXIMIZED)" = 0 ]; then
  pass "$KIND: a second double-click restores"
else
  fail "$KIND: a second double-click restores"
fi
size hello-weston-restored $(( ${restored%x*} + 48 )) $(( ${restored#*x} + 48 ))
# Restored at the maximized size, the window reaches past the screen: maximized again, its
# close button shows.
$DRIVE calibrate double-click-at 200 18 sleep 700 >/dev/null 2>&1
$DRIVE calibrate click-at $(( ${maximized%x*} - 27 )) 19 sleep 600 >/dev/null 2>&1
expect "request Close" "the drawn close button closes"
quit "the app quit"
