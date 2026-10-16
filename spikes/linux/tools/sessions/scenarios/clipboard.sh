# The clipboard and the primary selection both ways: the app offers (Ctrl+C, Ctrl+Shift+C)
# and another client reads; another client offers and the app pastes (Ctrl+V, Ctrl+Shift+V).
# CLIP: the other client's protocol (x11 or wlr).
. "$(dirname "$0")/common.sh"
case $KIND in
  x11)
    W=$(x11_window clipboard)
    click() { xdotool mousemove --window "$W" 100 100 click 1; }
    chord() { xdotool key "$1"; }
    ;;
  *)
    click() { "$TOOLS/drive" "$DRIVER" calibrate click-at 100 100 >/dev/null 2>&1; }
    chord() {
      case $1 in
        ctrl+c) "$TOOLS/drive" "$DRIVER" chord Control_L+c >/dev/null 2>&1 ;;
        ctrl+shift+c) "$TOOLS/drive" "$DRIVER" chord Control_L+Shift_L+c >/dev/null 2>&1 ;;
        ctrl+v) "$TOOLS/drive" "$DRIVER" chord Control_L+v >/dev/null 2>&1 ;;
        ctrl+shift+v) "$TOOLS/drive" "$DRIVER" chord Control_L+Shift_L+v >/dev/null 2>&1 ;;
      esac
    }
    ;;
esac
click; sleep 0.3
chord ctrl+c; sleep 0.3
got=$("$TOOLS/clip" "$CLIP" read clipboard)
[ "$got" = "Skene offered Clipboard 1" ] && pass "$KIND: another client reads the clipboard the app offered" || fail "$KIND: another client reads the clipboard the app offered (got $got)"
chord ctrl+shift+c; sleep 0.3
got=$("$TOOLS/clip" "$CLIP" read primary)
[ "$got" = "Skene offered Primary 2" ] && pass "$KIND: another client reads the primary selection the app offered" || fail "$KIND: another client reads the primary selection the app offered (got $got)"
"$TOOLS/clip" "$CLIP" write clipboard "Written by another client" 3 >/dev/null & sleep 0.4
chord ctrl+v; sleep 0.4
expect 'pasted Clipboard Some("Written by another client")' "the app pastes another client's clipboard"
"$TOOLS/clip" "$CLIP" write primary "Primary from another client" 3 >/dev/null & sleep 0.4
chord ctrl+shift+v; sleep 0.4
expect 'pasted Primary Some("Primary from another client")' "the app pastes another client's primary selection"
wait
