# Hello on sway: server-side decorations negotiated, sway draws the title bar; the window is
# tiled wide (the wider padding), then floated and resized narrow; then the output's scale goes
# to 1.5 and 1.25 (fractional-scale-v1) and the frame follows in device pixels.
. "$(dirname "$0")/common.sh"
shot hello-sway 0.6
expect "decorations Server" "xdg-decoration negotiates server-side decorations"
grim "$OUT/compositor.png"
$CHECK pixel "$OUT/compositor.png" 10 6 285577 3 | sed "s/^\(PASS\|FAIL\) /\1 $KIND: sway's own title bar: /"
size hello-sway 1276 997
pixel hello-sway 30 100 f2f2f7
pixel hello-sway 60 100 ffffff
swaymsg "[app_id=dev.skene.hello] floating enable" >/dev/null
sleep 0.3
# The container is 600 by 500; its border and title bar leave the window 596 by 473.
swaymsg "[app_id=dev.skene.hello] resize set width 600 px height 500 px" >/dev/null
shot hello-sway-narrow 0.6
size hello-sway-narrow 596 473
pixel hello-sway-narrow 30 100 ffffff
swaymsg output HEADLESS-1 scale 1.5 >/dev/null
shot hello-sway-150 0.8
expect "fractional scale 180/120" "fractional-scale-v1 reports 1.5"
size hello-sway-150 894 710
swaymsg output HEADLESS-1 scale 1.25 >/dev/null
shot hello-sway-125 0.8
expect "fractional scale 150/120" "fractional-scale-v1 reports 1.25"
size hello-sway-125 745 591
