# A thousand rows (release build): the first frame, hover frames (each restyling two rows),
# and wheel frames (the scroll repainted).
. "$(dirname "$0")/common.sh"
case $KIND in
  x11)
    W=$(x11_window rows)
    for y in 100 130 160 190 220 250 280 310; do xdotool mousemove --window "$W" 300 $y; sleep 0.15; done
    for _ in 1 2 3 4 5 6 7 8; do xdotool click 5; sleep 0.15; done
    ;;
  *)
    steps="calibrate"
    for y in 100 130 160 190 220 250 280 310; do steps="$steps move 300 $((y + ${HEADER:-0})) sleep 150"; done
    for _ in 1 2 3 4 5 6 7 8; do steps="$steps wheel 300 $((310 + ${HEADER:-0})) 0 1 sleep 150"; done
    # shellcheck disable=SC2086
    "$TOOLS/drive" "$DRIVER" $steps >/dev/null 2>&1
    ;;
esac
sleep 0.3
shot rows-$KIND 0
expect "[scroll]" "the wheel scrolls the list"
$CHECK frames "$LOG" "rows-$KIND"
