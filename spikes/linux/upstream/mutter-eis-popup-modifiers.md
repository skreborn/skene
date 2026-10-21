# A modifier held on an EIS keyboard is reported released while an xdg_popup holds the grab

Project: GNOME/mutter (https://gitlab.gnome.org/GNOME/mutter/-/issues)
Component: Wayland / input (EIS)
Version: mutter 46.2 (Ubuntu 24.04, 46.2-1ubuntu0.24.04.18), headless, `--wayland --no-x11 --virtual-monitor 1280x1024`

## Summary

A RemoteDesktop session's EIS keyboard pressing Control and then R, while a client's
`xdg_popup` holds an explicit grab (`xdg_popup.grab`), sends the client a
`wl_keyboard.modifiers` with Control *before* the Control key event, then a
`wl_keyboard.modifiers` with nothing depressed before the R key. The client sees R with no
modifier held, so a Ctrl+R shortcut does not fire while a popover or a menu is open.

The same keys through the same session's D-Bus request (`NotifyKeyboardKeycode`) are
reported correctly under the grab, and EIS keys are reported correctly when no popup has
the grab.

## Steps to reproduce

1. Build the reproducer next to this report (GTK 4.14, plain C):
   `gcc -o mods mutter-eis-popup-modifiers.c $(pkg-config --cflags --libs gtk4)`.
   It shows a window with a button; F2 opens an autohide popover (an `xdg_popup` with a grab),
   and a key controller prints each key-pressed's keyval and modifier state. A global
   `<Control>r` shortcut prints `SHORTCUT`.
2. Run it on a headless mutter with `GDK_BACKEND=wayland WAYLAND_DEBUG=client`.
3. Open a RemoteDesktop session, `ConnectToEIS`, bind a keyboard and press, in order:
   F2 (the popover opens), Control_L down, R down, R up, Control_L up (evdev codes 60, 29, 19).
4. Close the popover with Escape and press Control_L+R again.

## What happens

Under the popup's grab (keyboard focus on the popup's surface `wl_surface@42`):

```
wl_keyboard@43.enter(4, wl_surface@42, array[0])
wl_keyboard@43.modifiers(4, 0, 0, 0, 0)
wl_keyboard@43.modifiers(5, 4, 0, 0, 0)      <- Control reported before its key
wl_keyboard@43.key(6, 12311385, 29, 1)
wl_keyboard@43.modifiers(7, 0, 0, 0, 0)      <- Control reported released while held
wl_keyboard@43.key(8, 12311385, 19, 1)
wl_keyboard@43.key(9, 12311385, 19, 0)
wl_keyboard@43.key(10, 12311385, 29, 0)
KEY popover keyval=Control_L state=0x4 control=1
KEY popover keyval=r state=0 control=0
```

No `SHORTCUT` line follows.

## What is expected

What the same keys give without the popup (the window's own surface has the keyboard), and
what they give under the popup when sent through `NotifyKeyboardKeycode` on the session's
D-Bus:

```
wl_keyboard@48.key(7, 12330473, 29, 1)
wl_keyboard@48.modifiers(8, 4, 0, 0, 0)
wl_keyboard@48.key(9, 12330473, 19, 1)
KEY popover keyval=r state=0x4 control=1
wl_keyboard@48.key(10, 12330474, 19, 0)
wl_keyboard@48.key(11, 12330475, 29, 0)
wl_keyboard@48.modifiers(12, 0, 0, 0, 0)
```

The modifiers follow the key that changed them, and R arrives with Control held.

## Notes

- It reproduces on every run: 5 of 5 with GTK 4, and 3 of 3 with a client of our own that
  reads `wl_keyboard.modifiers` into an xkb state the same way.
- Only the EIS path is affected, and only while a popup holds the grab. The input-driving
  test harness this was found in now sends keys through the RemoteDesktop session's D-Bus
  requests, and keeps EIS for the pointer.
