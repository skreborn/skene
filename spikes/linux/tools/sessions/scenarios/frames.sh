# Hello's frame costs (release build): the first frame, then hover frames over its controls.
. "$(dirname "$0")/common.sh"
case $KIND in
  x11)
    W=$(x11_window hello)
    for p in "138 424" "342 424" "240 378" "418 194" "418 259" "240 316"; do set -- $p; xdotool mousemove --window "$W" "$1" "$2"; sleep 0.15; done
    ;;
  *)
    steps="calibrate"
    for p in "138 424" "342 424" "240 378" "418 194" "418 259" "240 316"; do set -- $p; steps="$steps move $1 $(($2 + ${HEADER:-0})) sleep 150"; done
    # shellcheck disable=SC2086
    "$TOOLS/drive" "$DRIVER" $steps >/dev/null 2>&1
    ;;
esac
sleep 0.3
$CHECK frames "$LOG" "hello-$KIND"
