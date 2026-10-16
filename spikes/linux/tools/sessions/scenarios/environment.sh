# The reader's environment from the settings-portal stand-in (started with PORTAL: light, then
# dark at 1.5 s, more contrast at 2.5 s, an accent and reduced motion at 3.5 s, light and no
# contrast at 4.5 s): the window's fill, the swatch and the accent follow. DX, DY: the
# content's offset in the frame; ACCENT_X, ACCENT_Y: a pixel of the switch's track, in the
# frame.
. "$(dirname "$0")/common.sh"
dx=${DX:-0}; dy=${DY:-0}
sleep 1; shot env-$KIND-light 0
pixel env-$KIND-light $((200 + dx)) $((300 + dy)) ffffff
pixel env-$KIND-light "$ACCENT_X" "$ACCENT_Y" 3584e4
sleep 1; shot env-$KIND-dark 0
pixel env-$KIND-dark $((200 + dx)) $((300 + dy)) 1c1c1e
sleep 1; shot env-$KIND-contrast 0
pixel env-$KIND-contrast $((200 + dx)) $((300 + dy)) ffff00
sleep 1; shot env-$KIND-accent 0
sleep 1.2; shot env-$KIND-back 0
pixel env-$KIND-back $((200 + dx)) $((300 + dy)) ffffff
pixel env-$KIND-back "$ACCENT_X" "$ACCENT_Y" e6334d
expect "[portal] changed org.freedesktop.appearance color-scheme = U32(1)" "the portal's scheme change reaches the app"
expect "[portal] changed org.freedesktop.appearance contrast = U32(1)" "the portal's contrast change reaches the app"
expect "[portal] changed org.freedesktop.appearance accent-color = Color(0.9, 0.2, 0.3)" "the portal's accent reaches the app"
expect "[portal] changed org.freedesktop.appearance reduced-motion = U32(1)" "the portal's reduced motion reaches the app"
