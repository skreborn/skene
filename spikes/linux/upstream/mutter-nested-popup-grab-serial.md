# mutter: a nested xdg_popup grabbed with the press that opened it is dismissed at once

Against: mutter 46.2 (Ubuntu 24.04's, headless, `mutter --headless --wayland`). Not filed;
ready to file.

## Summary

A client with an xdg_popup open (and grabbed) opens a second popup, its parent the first,
from a click inside the first popup, and grabs it with the serial of that click's button
press, as xdg-shell asks ("the serial of the user event"). mutter answers the grab with
`xdg_popup.popup_done` before the popup is ever configured. Grabbed with the serial that
opened the *first* popup, the same nested popup is accepted, configured and keeps the grab.
On headless sway the same test passes.

## Reproduction

Skene's Linux prototype (`spikes/linux`), the driver test
`popups::a_picker_inside_a_popup_opens_its_list_and_chooses_by_a_click` (in
`conformance-linux/tests/it`) on the runner's Wayland leg (headless mutter), with the
workaround below taken out, `WAYLAND_DEBUG=client`:

```text
-> xdg_surface@31.get_popup(xdg_popup@35, xdg_surface@12, xdg_positioner@29)
-> xdg_popup@35.grab(wl_seat@15, 5)                     # the first popup, from a press (serial 5)
<- xdg_popup@35.configure, (16, 90, 448, 83)
<- wl_pointer@34.button, (11, 107348097, 272, 1)        # a press inside the first popup
<- wl_pointer@34.button, (12, 107348097, 272, 0)        # its release
-> xdg_surface@41.get_popup(xdg_popup@42, xdg_surface@31, xdg_positioner@29)
-> xdg_popup@42.grab(wl_seat@15, 11)                    # the nested popup, the press's serial
<- xdg_popup@42.popup_done, ()                          # refused, never configured
```

With `xdg_popup@42.grab(wl_seat@15, 5)` instead (the first popup's serial) the nested popup
is configured and stays open, and the test passes.

## Where

`meta-wayland-xdg-shell.c` (`xdg_popup_apply_state` → `meta_wayland_seat_can_popup`) accepts a
grab whose serial is the pointer's `grab_serial` (`meta_wayland_pointer_can_popup`), which
`handle_button_event` (`meta-wayland-pointer.c`) sets only for an implicit grab, a press with
`button_count == 1`. Under an xdg_popup grab the press inside the popup evidently does not
update it, so the serial of the press that opened the first popup stays the only one accepted
(JUDGMENT: read from the 46.2 source; not stepped through).

## Expected

A grab with the serial of the latest press the client received (inside its own grabbed popup)
is accepted for a popup whose parent is the topmost popup, as the protocol's nesting rules and
the other compositors allow.

## Workaround in Skene

`gui-linux/src/wayland.rs`: a nested popup dismissed before its first configure is opened once
more, grabbed with the serial that opened the first popup.
