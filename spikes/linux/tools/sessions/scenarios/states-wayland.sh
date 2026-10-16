# The same rows on Wayland through the compositor's real input (DRIVER: sway's virtual
# pointer and keyboard, or mutter's RemoteDesktop), the window found by calibration. DX, DY:
# the content's offset in the frame (the shadow's margin and the title bar under CSD), HEADER
# the title bar's height in the window's geometry. One driver run acts the whole scenario, a
# frame copied between its steps.
. "$(dirname "$0")/common.sh"
dx=${DX:-0}; dy=${DY:-0}; header=${HEADER:-0}
copy() { echo "cp $OUT/frame.png $OUT/states-$KIND-$1.png"; }
"$TOOLS/drive" "$DRIVER" calibrate \
  move 150 $((65 + header)) sleep 400 run "$(copy hover)" \
  press sleep 400 run "$(copy pressed)" \
  release sleep 300 \
  click-at 430 $((65 + header)) sleep 300 run "echo clicked switch" \
  move 150 $((400 + header)) sleep 200 key Tab sleep 400 run "$(copy focus)" \
  key Tab sleep 100 key Tab sleep 400 run "$(copy focus3)" \
  key Return sleep 200 key space sleep 400 >"$OUT/drive.log" 2>&1
grep -E "^(FAIL|calibrate)" "$OUT/drive.log" | sed "s/^/INFO $KIND: driver: /"
pixel states-$KIND-hover $((150 + dx)) $((70 + dy)) dbeafe
pixel states-$KIND-pressed $((150 + dx)) $((70 + dy)) 93c5fd
expect "[change] Control(Switch)" "a click on the row's switch flips the switch"
count "[activate] Layout(Box)" 3 "a click on a row, Enter and Space activate it; a click on its switch does not"
pixel states-$KIND-focus $((150 + dx)) $((126 + dy)) fde68a
pixel states-$KIND-focus3 $((150 + dx)) $((182 + dy)) fde68a
