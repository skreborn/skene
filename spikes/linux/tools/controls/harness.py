#!/usr/bin/python3.12 -I
"""The controls checks, run inside tools/controls/session.sh: what a reader finds over AT-SPI
(libatspi, as Orca reads it) and what the app reports (`[app]` lines of its log), after real
input through tools/bin/drive ($DRIVE: x11, mutter-dbus or sway).

  harness.py controls     every control by pointer, keys and a reader's actions
  harness.py fields       text entry: typing, handlers, selection gestures, clipboard, primary
                          selection, undo, the secure field, the editor
  harness.py hangul       Hangul through the session's input method: composition, commit,
                          candidates, surrounding text, a rejecting field, undo
  harness.py reader       textprobe.py report and edit on the first field (X11)
  harness.py focus        a click on the Name field
  harness.py dump         the tree, for a look

Each check prints `[<label>] PASS|FAIL <what>: <detail>`; LABEL names the session.
"""

import os
import re
import subprocess
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.environ["OUT"]
DRIVE = os.environ.get("DRIVE", "x11")
LABEL = os.environ.get("LABEL", os.environ.get("KIND", "x11"))
WINDOW = os.environ.get("ATSPI_WINDOW", "controls")
WAYLAND = DRIVE != "x11"
GNOME = os.environ.get("KIND") == "gnome"
# Another client reads and offers the clipboard through wlr-data-control (sway) or X11
# selections; mutter has no such protocol.
CLIP = DRIVE != "mutter-dbus"
FAILED = []

# Real input goes only to a session tools/controls/session.sh made.
_PRIVATE = os.environ.get("SKENE_PRIVATE_SESSION", "")
if not _PRIVATE or _PRIVATE != os.environ.get("XDG_RUNTIME_DIR") or not os.path.isdir(_PRIVATE):
  sys.exit("harness.py: refusing to inject input outside a private session "
           "(run it through tools/controls/session.sh)")


def check(what, ok, detail=""):
  status = "PASS" if ok else "FAIL"
  if not ok:
    FAILED.append(what)
  suffix = f": {detail}" if detail != "" else ""
  print(f"[{LABEL}] {status} {what}{suffix}", flush=True)
  return ok


def note(text):
  print(f"[{LABEL}] NOTE {text}", flush=True)


# --- the app over AT-SPI ---

_app = [None]


def app():
  if _app[0] is not None:
    return _app[0]
  end = time.time() + 15
  while time.time() < end:
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
      candidate = desktop.get_child_at_index(index)
      if candidate is None:
        continue
      try:
        for child_index in range(candidate.get_child_count()):
          child = candidate.get_child_at_index(child_index)
          if child is not None and child.get_name() == WINDOW:
            _app[0] = candidate
            return candidate
      except GLib.Error:
        pass
    time.sleep(0.2)
  raise SystemExit(f"no app with a window named {WINDOW!r}")


def walk(node, depth=0, out=None):
  if out is None:
    out = []
  out.append((depth, node))
  try:
    count = node.get_child_count()
  except GLib.Error:
    return out
  for index in range(count):
    child = node.get_child_at_index(index)
    if child is not None:
      walk(child, depth + 1, out)
  return out


def find(role, name=None, timeout=3.0):
  end = time.time() + timeout
  while True:
    for _, node in walk(app()):
      try:
        if node.get_role_name() == role and (name is None or node.get_name() == name):
          return node
      except GLib.Error:
        pass
    if time.time() > end:
      raise SystemExit(f"no {role} named {name!r}")
    time.sleep(0.1)


def states(node):
  state_set = node.get_state_set()
  return {
    state.value_nick
    for state in Atspi.StateType.__enum_values__.values()
    if state != Atspi.StateType.LAST_DEFINED and state_set.contains(state)
  }


def extents(node):
  box = Atspi.Component.get_extents(node, Atspi.CoordType.WINDOW)
  return (box.x, box.y, box.width, box.height)


def center(node, fx=0.5, fy=0.5):
  x, y, width, height = extents(node)
  return (round(x + width * fx), round(y + height * fy))


def text(node):
  return Atspi.Text.get_text(node, 0, Atspi.Text.get_character_count(node))


def caret(node):
  return Atspi.Text.get_caret_offset(node)


def selection(node):
  if Atspi.Text.get_n_selections(node) == 0:
    return None
  span = Atspi.Text.get_selection(node, 0)
  return (span.start_offset, span.end_offset)


def action(node, name):
  for index in range(Atspi.Action.get_n_actions(node)):
    if Atspi.Action.get_action_name(node, index) == name:
      return Atspi.Action.do_action(node, index)
  raise SystemExit(f"no action {name}")


def field(name):
  for role in ["entry", "password text", "text"]:
    for _, node in walk(app()):
      try:
        if node.get_role_name() == role and node.get_name() == name:
          return node
      except GLib.Error:
        pass
  raise SystemExit(f"no field named {name!r}")


# --- the app's log ---


def log_text():
  try:
    with open(os.path.join(OUT, "app.log"), encoding="utf-8", errors="replace") as file:
      return file.read()
  except FileNotFoundError:
    return ""


def mark():
  return len(log_text())


def app_lines(since):
  return [line for line in log_text()[since:].splitlines() if line.startswith("[app] ")]


def wait_app(pattern, since, timeout=3.0):
  end = time.time() + timeout
  while True:
    for line in app_lines(since):
      if re.search(pattern, line):
        return line
    if time.time() > end:
      return None
    time.sleep(0.05)


def last_app(what, since):
  found = None
  for line in app_lines(since):
    if line.startswith(f"[app] {what} "):
      found = line[len(f"[app] {what} "):]
  return found


# --- input ---

_calibrated = [False]


def drive(*steps):
  args = [os.path.join(ROOT, "tools/bin/drive"), DRIVE]
  needs_pointer = any(str(step).endswith("-at") or step in ("drag", "move", "wheel") for step in steps)
  if WAYLAND and needs_pointer and not _calibrated[0]:
    args.append("calibrate")
    _calibrated[0] = True
  elif WAYLAND and needs_pointer:
    args.append("calibrate")
  args += [str(step) for step in steps]
  result = subprocess.run(args, capture_output=True, text=True, timeout=60)
  with open(os.path.join(OUT, "drive.log"), "a", encoding="utf-8") as log:
    log.write(" ".join(args) + "\n" + result.stdout + result.stderr)
  if result.returncode != 0:
    note(f"drive failed: {result.stderr.strip()[-300:]}")
  time.sleep(0.15)


def screen_extents(node):
  """The node's rectangle on the screen: AT-SPI's on X11; on Wayland, where a client does not
  know its place, the window's rectangle moved by the origin the driver measured."""
  box = Atspi.Component.get_extents(node, Atspi.CoordType.SCREEN)
  if not WAYLAND:
    return box
  x, y, _, _ = extents(node)
  try:
    with open(os.path.join(OUT, "drive.log"), encoding="utf-8") as log:
      found = re.findall(r"measured origin Some\(\((-?[\d.]+), (-?[\d.]+)\)\)", log.read())
  except FileNotFoundError:
    found = []
  if found:
    box.x = x + round(float(found[-1][0]))
    box.y = y + round(float(found[-1][1]))
  return box


def drive_background(*steps):
  """Starts the driver on `steps` without waiting for it (no pointer steps)."""
  args = [os.path.join(ROOT, "tools/bin/drive"), DRIVE, *[str(step) for step in steps]]
  log = open(os.path.join(OUT, "drive.log"), "a", encoding="utf-8")  # noqa: SIM115
  log.write(" ".join(args) + "\n")
  log.flush()
  return subprocess.Popen(args, stdout=log, stderr=log)


def click(node, fx=0.5, fy=0.5):
  x, y = center(node, fx, fy)
  drive("click-at", x, y)


MODIFIERS = {"shift": "Shift_L", "ctrl": "Control_L", "alt": "Alt_L"}


def keys(*names):
  steps = []
  for name in names:
    if "+" in name and len(name) > 1 and WAYLAND:
      # Held as a hand holds them: mutter honours a popup's grab only while the key that
      # opened it is held or was the last released, checked when the popup maps.
      parts = [MODIFIERS.get(part, part) for part in name.split("+")]
      for part in parts:
        steps += ["keydown", part]
      steps += ["sleep", "150"]
      for part in reversed(parts):
        steps += ["keyup", part]
    elif "+" in name and len(name) > 1:
      parts = [MODIFIERS.get(part, part) for part in name.split("+")]
      steps += ["chord", "+".join(parts)]
    else:
      steps += ["key", name]
    steps += ["sleep", "60"]
  drive(*steps)


def settle(seconds=0.3):
  time.sleep(seconds)


def shot(name):
  frame = os.path.join(OUT, "frame.png")
  if os.path.exists(frame):
    subprocess.run(["cp", frame, os.path.join(OUT, f"{name}.png")], check=False)


def clip(*args):
  backend = "x11" if DRIVE == "x11" else "wlr"
  result = subprocess.run(
    [os.path.join(ROOT, "tools/bin/clip"), backend, *args],
    capture_output=True, text=True, timeout=20)
  return result.stdout.strip()


def clip_offer(selection_name, value, seconds=8):
  backend = "x11" if DRIVE == "x11" else "wlr"
  return subprocess.Popen(
    [os.path.join(ROOT, "tools/bin/clip"), backend, "write", selection_name, value, str(seconds)],
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


# --- scenarios ---


def scenario_controls():
  # The switch: a click flips it; Space flips it on its release, Enter does not; a reader's
  # click action flips it and the state reads back at once.
  switch = find("check box", "Wi-Fi")
  since = mark()
  click(switch)
  check("switch: a click turns it on", wait_app(r"^\[app\] wifi true", since) is not None)
  check("switch: the reader reads it checked, as GtkSwitch", "checked" in states(switch),
        sorted(states(switch)))
  since = mark()
  keys("space")
  check("switch: Space turns it off", wait_app(r"^\[app\] wifi false", since) is not None)
  since = mark()
  keys("Return")
  settle()
  check("switch: Enter leaves it", last_app("wifi", since) is None)
  since = mark()
  action(switch, "click")
  check("switch: a reader's click turns it on, read back at once",
        "checked" in states(switch) and wait_app(r"^\[app\] wifi true", since, 1) is not None,
        sorted(states(switch)))
  check("switch: described by its description",
        switch.get_description() == "Connects to wireless networks", repr(switch.get_description()))

  # The checkbox: a click on its caption flips it, as a browser's label does; Space flips it.
  checkbox = find("check box", "Remember me")
  caption = find("label", "Remember me")
  since = mark()
  click(caption)
  check("checkbox: a click on its caption checks it",
        wait_app(r"^\[app\] remember true", since) is not None)
  check("checkbox: the reader reads it checked", "checked" in states(checkbox))
  check("checkbox: focus moved to it", "focused" in states(checkbox), sorted(states(checkbox)))
  since = mark()
  keys("space")
  check("checkbox: Space unchecks it", wait_app(r"^\[app\] remember false", since) is not None)
  relations = {
    relation.get_relation_type().value_nick: [
      relation.get_target(index).get_name() for index in range(relation.get_n_targets())
    ]
    for relation in checkbox.get_relation_set()
  }
  check("checkbox: labelled by its caption", relations.get("labelled-by") == ["Remember me"],
        relations)

  # The radio group: a click chooses; the arrows move focus and choose, wrapping; the group
  # is one Tab stop.
  small = find("radio button", "Small")
  medium = find("radio button", "Medium")
  large = find("radio button", "Large")
  since = mark()
  click(small)
  check("radio: a click chooses Small", wait_app(r"^\[app\] size Some\(0\)", since) is not None)
  since = mark()
  keys("Down")
  check("radio: Down chooses Medium", wait_app(r"^\[app\] size Some\(1\)", since) is not None)
  check("radio: focus moved to Medium", "focused" in states(medium))
  since = mark()
  keys("Down", "Down")
  check("radio: Down past the last wraps to Small",
        wait_app(r"^\[app\] size Some\(0\)", since) is not None)
  since = mark()
  keys("Up")
  check("radio: Up past the first wraps to Large",
        wait_app(r"^\[app\] size Some\(2\)", since) is not None)
  check("radio: the reader reads Large checked", "checked" in states(large))
  keys("Tab")
  slider = find("slider", "Volume")
  check("radio: Tab leaves the group for the slider", "focused" in states(slider),
        sorted(states(slider)))
  since = mark()
  click(find("label", "Medium"))
  check("radio: a click on a caption chooses its radio",
        wait_app(r"^\[app\] size Some\(1\)", since) is not None)

  # The slider: a press jumps, a drag follows, the keys step; a reader sets its value.
  since = mark()
  click(slider, 0.75)
  value = Atspi.Value.get_current_value(slider)
  check("slider: a press at three quarters jumps there",
        wait_app(r"^\[app\] volume 0\.[78]", since) is not None and 0.65 <= value <= 0.85, value)
  since = mark()
  keys("Home")
  check("slider: Home goes to 0", wait_app(r"^\[app\] volume 0\.0", since) is not None)
  since = mark()
  keys("End")
  check("slider: End goes to 1", wait_app(r"^\[app\] volume 1\.0", since) is not None)
  since = mark()
  keys("Left")
  check("slider: Left steps down by 0.1", wait_app(r"^\[app\] volume 0\.9", since) is not None)
  since = mark()
  keys("Page_Down")
  check("slider: PageDown steps by a tenth", wait_app(r"^\[app\] volume 0\.8", since) is not None)
  since = mark()
  Atspi.Value.set_current_value(slider, 0.3)
  now = Atspi.Value.get_current_value(slider)
  check("slider: a reader sets 0.3, read back at once",
        abs(now - 0.3) < 1e-6 and wait_app(r"^\[app\] volume 0\.3", since, 1) is not None, now)
  x, y, width, height = extents(slider)
  since = mark()
  drive("drag", round(x + width * 0.3), y + height // 2, round(x + width * 0.1), y + height // 2)
  settle()
  moves = [line for line in app_lines(since) if line.startswith("[app] volume ")]
  check("slider: a drag follows the pointer", len(moves) >= 1 and moves[-1].endswith("0.1"), moves)

  # The picker: a click opens its list; the arrows highlight, Enter chooses; Escape closes;
  # the closed face's arrows and type-ahead choose; a click on an option chooses it; a click
  # outside closes.
  picker = find("combo box", "Fruit")
  since = mark()
  click(picker)
  settle(0.5)
  check("picker: a click opens the list", "expanded" in states(picker), sorted(states(picker)))
  listed = [node for _, node in walk(picker) if node.get_role_name() == "list item"]
  check("picker: the reader finds its five options", len(listed) == 5,
        [node.get_name() for node in listed])
  check("picker: the list is drawn", os.path.exists(os.path.join(OUT, "popup.png")))
  if os.path.exists(os.path.join(OUT, "popup.png")):
    subprocess.run(["cp", os.path.join(OUT, "popup.png"), os.path.join(OUT, "picker-open.png")])
  keys("Down", "Down")
  settle()
  focused = [node.get_name() for node in listed if "focused" in states(node)]
  check("picker: Down Down highlights Blueberry, the reader's focus on it",
        focused == ["Blueberry"], focused)
  keys("Return")
  check("picker: Enter chooses Blueberry", wait_app(r"^\[app\] fruit Some\(2\)", since) is not None)
  settle()
  check("picker: Enter closes the list", "expanded" not in states(picker))
  since = mark()
  keys("Alt_L+Down")
  settle(0.4)
  check("picker: Alt+Down opens the list", "expanded" in states(picker))
  keys("Escape")
  settle()
  check("picker: Escape closes it choosing nothing",
        "expanded" not in states(picker) and last_app("fruit", since) is None)
  since = mark()
  keys("c")
  check("picker: typing c on the closed face chooses Cherry",
        wait_app(r"^\[app\] fruit Some\(3\)", since) is not None)
  since = mark()
  keys("Up")
  check("picker: Up on the closed face chooses Blueberry",
        wait_app(r"^\[app\] fruit Some\(2\)", since) is not None)
  click(picker)
  settle(0.5)
  grape = find("list item", "Grape")
  since = mark()
  click(grape)
  check("picker: a click on Grape chooses it", wait_app(r"^\[app\] fruit Some\(4\)", since) is not None)
  settle()
  check("picker: the click closes the list", "expanded" not in states(picker))
  click(picker)
  settle(0.5)
  since = mark()
  x, y, width, height = extents(find("label", "Controls"))
  drive("click-at", x + 5, y + 5)
  settle(0.4)
  check("picker: a click outside closes the list choosing nothing",
        "expanded" not in states(picker) and last_app("fruit", since) is None)

  # The button: a click, Enter, Space on its release and a reader's click activate it.
  button = find("push button", "Submit")
  since = mark()
  click(button)
  check("button: a click activates", wait_app(r"^\[app\] submit 1", since) is not None)
  keys("Return")
  check("button: Enter activates", wait_app(r"^\[app\] submit 2", since) is not None)
  # One driver holds the key (on Wayland a virtual keyboard's keys go up as it goes away).
  holding = drive_background("keydown", "space", "sleep", "1200", "keyup", "space")
  time.sleep(0.8)
  pressed_early = last_app("submit", since) == "3"
  holding.wait(timeout=30)
  check("button: Space activates on its release, not its press",
        not pressed_early and wait_app(r"^\[app\] submit 3", since) is not None)
  action(button, "click")
  status = find("label", "Submitted 4 times", timeout=0.5)
  check("button: a reader's click activates, its effect read back at once", status is not None)
  check("button: described by the status text",
        button.get_description() == "Submitted 4 times", repr(button.get_description()))


def scenario_fields():
  name = field("Name")
  since = mark()
  click(name)
  drive("type", "Hello brave world")
  check("field: typed text reaches the app",
        wait_app(r'^\[app\] name "Hello brave world"', since) is not None)
  check("field: the reader reads it", text(name) == "Hello brave world", repr(text(name)))
  check("field: the caret at its end", caret(name) == 17, caret(name))

  # Selection gestures.
  x, y, width, height = extents(name)
  sx, sy, sw, sh = Atspi.Text.get_range_extents(name, 6, 11, Atspi.CoordType.WINDOW).x, 0, 0, 0
  brave = Atspi.Text.get_range_extents(name, 6, 11, Atspi.CoordType.WINDOW)
  hello = Atspi.Text.get_range_extents(name, 0, 5, Atspi.CoordType.WINDOW)
  mid_y = y + height // 2
  drive("double-click-at", brave.x + brave.width // 2, mid_y)
  settle()
  check("field: a double click selects the word", selection(name) == (6, 11), selection(name))
  drive("sleep", "500", "triple-click-at", brave.x + brave.width // 2, mid_y)
  settle()
  check("field: a triple click selects the line", selection(name) == (0, 17), selection(name))
  # A press inside the selection drags its text, as a browser's does: the drag that selects
  # starts from a collapsed one.
  keys("End")
  drive("sleep", "500", "drag", hello.x + 1, mid_y, brave.x + brave.width - 1, mid_y)
  settle()
  check("field: a drag selects what it covers", selection(name) == (0, 11), selection(name))
  keys("End", "shift+Left", "shift+Left")
  check("field: Shift+Left extends from the caret", selection(name) == (15, 17), selection(name))

  # The clipboard and the primary selection.
  drive("sleep", "500", "double-click-at", brave.x + brave.width // 2, mid_y)
  settle()
  if CLIP:
    check("primary: a selection is offered as the primary selection",
          clip("read", "primary") == "brave", clip("read", "primary"))
  keys("ctrl+c")
  settle()
  if CLIP:
    check("clipboard: Ctrl+C copies the selection", clip("read", "clipboard") == "brave",
          clip("read", "clipboard"))
  else:
    note("mutter offers no data-control protocol: another client's clipboard and primary "
         "selection are checked on X11 and sway; here Ctrl+V reads back what Ctrl+C wrote")
  since = mark()
  keys("End", "ctrl+v")
  check("clipboard: Ctrl+V pastes it", wait_app(r'^\[app\] name "Hello brave worldbrave"', since)
        is not None)
  since = mark()
  keys("ctrl+z")
  check("undo: Ctrl+Z takes the paste back", wait_app(r'^\[app\] name "Hello brave world"', since)
        is not None and text(name) == "Hello brave world", repr(text(name)))
  since = mark()
  keys("ctrl+shift+z")
  check("redo: Ctrl+Shift+Z brings it again",
        wait_app(r'^\[app\] name "Hello brave worldbrave"', since) is not None)
  if CLIP:
    offer = clip_offer("clipboard", "Outside")
    settle(0.6)
    since = mark()
    keys("ctrl+a", "ctrl+v")
    check("clipboard: another app's text pastes over the selection",
          wait_app(r'^\[app\] name "Outside"', since) is not None, repr(text(name)))
    offer.terminate()
    offer = clip_offer("primary", " PRIMARY")
    settle(0.6)
    since = mark()
    end = Atspi.Text.get_character_extents(name, 6, Atspi.CoordType.WINDOW)
    drive("middle-click-at", end.x + end.width + 8, mid_y)
    check("primary: a middle click pastes another app's primary selection there",
          wait_app(r'^\[app\] name "Outside PRIMARY"', since) is not None, repr(text(name)))
    offer.terminate()

  # A transforming handler and a rejecting one.
  shout = field("Shout")
  since = mark()
  click(shout)
  drive("type", "abc")
  check("transform: the handler's capitals show", wait_app(r'^\[app\] shout "ABC"', since)
        is not None and text(shout) == "ABC", repr(text(shout)))
  check("transform: the caret stays after the text", caret(shout) == 3, caret(shout))
  digits = field("Digits")
  since = mark()
  click(digits)
  drive("type", "12a3")
  settle()
  check("reject: a letter is refused", text(digits) == "123"
        and wait_app(r'^\[app\] digits-rejected "12a"', since) is not None, repr(text(digits)))
  check("reject: the caret stays where the refused letter went", caret(digits) == 3, caret(digits))

  # The secure field: bullets to the reader, the last character shown for a moment, no copy.
  password = field("Password")
  click(password)
  drive("type", "secret")
  shot("password-echo")
  check("secure: the reader reads bullets", text(password) == "••••••", repr(text(password)))
  check("secure: the reader's role is password text", password.get_role_name() == "password text")
  settle(1.3)
  shot("password-masked")
  if CLIP:
    before = clip("read", "clipboard")
    keys("ctrl+a", "ctrl+c")
    settle()
    check("secure: Ctrl+C copies nothing", clip("read", "clipboard") == before,
          f"clipboard {clip('read', 'clipboard')!r}")
  since = mark()
  keys("ctrl+x")
  settle()
  check("secure: Ctrl+X cuts nothing", last_app("password-length", since) is None)

  # The editor: Enter breaks lines.
  notes = field("Notes")
  since = mark()
  click(notes)
  drive("type", "one", "key", "Return", "type", "two")
  check("editor: Enter breaks the line", wait_app(r'^\[app\] notes "one\\ntwo"', since) is not None,
        repr(text(notes)))
  check("editor: the reader reads two lines",
        Atspi.Text.get_string_at_offset(notes, 5, Atspi.TextGranularity.LINE).content == "two",
        repr(Atspi.Text.get_string_at_offset(notes, 5, Atspi.TextGranularity.LINE).content))
  # Compose without an input method: a dead key, then the letter it accents (X11: a layout
  # with dead keys and a Compose key, switched to at run time).
  if DRIVE == "x11":
    subprocess.run(["setxkbmap", "-layout", "us", "-variant", "intl", "-option", "compose:ralt"],
                   check=False)
    settle(0.5)
    since = mark()
    drive("key", "dead_acute", "sleep", "60", "key", "e", "sleep", "60", "key", "Multi_key",
          "sleep", "60", "key", "o", "sleep", "60", "key", "c")
    check("compose: dead acute then e types é, Compose o c types ©",
          wait_app(r'^\[app\] notes "one\\ntwoé©"', since) is not None, repr(text(notes)))
    subprocess.run(["setxkbmap", "-layout", "us", "-option", ""], check=False)
  shot("fields")


def scenario_hangul():
  name = field("Name")
  click(name)
  settle(0.5)
  since = mark()
  keys("g", "k", "s")
  settle(0.4)
  shot("hangul-preedit")
  check("hangul: g k s composes 한", last_app("name", since) == '"한"', app_lines(since)[-3:])
  keys("r", "m", "f", "space")
  settle(0.5)
  check("hangul: r m f space commits 한글 and a space", text(name) == "한글 ", repr(text(name)))
  check("hangul: the app saw the commit", last_app("name", since) == '"한글 "',
        last_app("name", since))
  keys("BackSpace")
  settle(0.3)
  check("hangul: BackSpace deletes the space", text(name) == "한글", repr(text(name)))
  xim = LABEL.endswith("xim")
  if xim:
    note("XIM carries no surrounding text: the conversion of the text before the caret is "
         "an input method's over D-Bus or text-input-v3 alone")
  im_log = os.path.join(OUT, "im-bus.log")
  if WAYLAND and not GNOME:
    # Fcitx5 behind sway: input-method-v2 carries what text-input-v3 told the compositor.
    told = re.search(r'^\[ime\] commit #\d+.*surrounding "한글"', log_text(), re.M) is not None
    check("surrounding: the app told text-input-v3 the text around the caret", told)
    rect = re.findall(r'^\[ime\] commit #\d+.*rect \((-?\d+), (-?\d+), (\d+), (\d+)\)',
                      log_text(), re.M)
    x, y, _, _ = (int(value) for value in rect[-1]) if rect else (-1, -1, 0, 0)
    fx, fy, fw, fh = extents(name)
    check("candidates: the caret's rectangle told lies in the field (surface pixels)",
          fx <= x <= fx + fw and fy <= y <= fy + fh, f"{x},{y} in {fx},{fy} {fw}x{fh}")
  if os.path.exists(im_log):
    with open(im_log, encoding="utf-8", errors="replace") as file:
      bus = file.read()
    told = "SetSurroundingText" in bus and "한글" in bus
    if GNOME:
      # gnome-shell's IBus client forwards the surrounding text only once the engine asked
      # for it (RequireSurroundingText, at its first F9): what the app sends is the check.
      told = re.search(r'^\[ime\] commit #\d+.*surrounding "한글"', log_text(), re.M) is not None
      check("surrounding: the app told text-input-v3 the text around the caret", told)
    elif not xim:
      check("surrounding: the input method was told the text around the caret", told)
    locations = []
    for block in re.split(r"\n(?=\S)", bus):
      if re.search(r"member=(SetCursorLocation|SetCursorRect)\b", block) and "IBus.Engine" not in block:
        values = [int(value) for value in re.findall(r"int32 (-?\d+)", block)[:4]]
        if len(values) == 4:
          locations.append(values)
    origin = screen_extents(name)
    if locations:
      x, y, width, height = locations[-1]
      note(f"cursor rectangle told {x},{y} {width}x{height}; the field on screen at "
           f"{origin.x},{origin.y} {origin.width}x{origin.height}")
      check("candidates: the caret's rectangle told lies in the field",
            origin.x <= x <= origin.x + origin.width and origin.y <= y <= origin.y + origin.height,
            f"{x},{y}")
    else:
      check("candidates: the caret's rectangle is told", False, "no SetCursorLocation")
  # A Hanja conversion of the text before the caret: the input method reads the surrounding
  # text and deletes around the caret.
  keys("F9")
  settle(0.8)
  shot("hangul-hanja")
  if DRIVE == "x11":
    windows = subprocess.run(["xwininfo", "-root", "-children"], capture_output=True, text=True).stdout
    with open(os.path.join(OUT, "windows.txt"), "w", encoding="utf-8") as file:
      file.write(windows)
  keys("1")
  settle(0.6)
  converted = text(name)
  if GNOME and converted == "한글1":
    # The engine asked for the surrounding text at the first F9, which therefore converted
    # nothing (GtkEntry under gnome-shell 46 alike): the second F9 has it.
    note("the first F9 converted nothing: gnome-shell's IBus client had not forwarded the "
         "surrounding text before the engine asked for it")
    keys("BackSpace")
    settle(0.3)
    keys("F9")
    settle(0.8)
    shot("hangul-hanja")
    keys("1")
    settle(0.6)
    converted = text(name)
  if xim:
    note(f"after F9 1 the field reads {converted!r}")
    if converted != "한글":
      keys("BackSpace")
  else:
    check("surrounding: F9 converts the syllable before the caret (delete-surrounding)",
          converted not in ("한글", "") and len(converted) == 2, repr(converted))
    keys("ctrl+z")
    settle(0.4)
    check("undo: Ctrl+Z takes the conversion back as one step", text(name) == "한글",
          repr(text(name)))

  # A rejecting field: the composition shows while it lasts, the truth returns at its end.
  digits = field("Digits")
  click(digits)
  settle(0.4)
  since = mark()
  keys("g", "k")
  settle(0.4)
  composing = text(digits)
  keys("space")
  settle(0.5)
  check("reject: a composition in a digits-only field is undone at its commit",
        text(digits) == "" and any("digits-rejected" in line for line in app_lines(since)),
        f"while composing {composing!r}, after {text(digits)!r}")
  shot("hangul-final")

  # The keyboard the app asked for (TextField::set_keyboard): the email field's purpose told.
  click(field("Email"))
  settle(0.8)
  if WAYLAND and not GNOME:
    told = re.search(r'^\[ime\] commit #\d+.*purpose Email', log_text(), re.M) is not None
    check("keyboard: the email field's purpose told through text-input-v3 (Email)", told)
  elif os.path.exists(im_log) and not xim:
    with open(im_log, encoding="utf-8", errors="replace") as file:
      bus = file.read()
    if LABEL.endswith("fcitx"):
      told = any(int(value) & (1 << 7)
                 for value in re.findall(r"member=SetCapability[^\n]*\n\s+uint64 (\d+)", bus))
      check("keyboard: the email field's purpose told to Fcitx5 (capability bit 7)", told)
    else:
      told = re.search(r'string "ContentType"\s+variant\s+struct \{\s+uint32 6', bus) is not None \
        or re.search(r'member=SetContentType[^\n]*\n\s+uint32 6', bus) is not None
      check("keyboard: the email field's purpose told to IBus (purpose 6, email)", told)


def scenario_secure():
  # A secure field through a real input method: the input method is told the field holds a
  # password and its text as one mark per cluster, the field takes what is typed, and neither
  # the app's trace nor what crossed the input method's bus shows it.
  secret = "qz7jvw"
  password = field("Password")
  bus_since = 0
  im_log = os.path.join(OUT, "im-bus.log")
  if os.path.exists(im_log):
    bus_since = os.path.getsize(im_log)
  click(password)
  settle(0.5)
  since = mark()
  keys(*secret)
  settle(0.6)
  length = last_app("password-length", since)
  check("secure: the field took what was typed", length not in (None, "0"), repr(length))
  traced = log_text()[since:]
  trigrams = [secret[index:index + 3] for index in range(len(secret) - 2)]
  quoted = [f'"{character}"' for character in secret] + [f"'{character}'" for character in secret]
  shown = [line for line in traced.splitlines() if not line.startswith("[app] ")
           and (any(run in line for run in trigrams) or any(quote in line for quote in quoted)
                or re.search("[\uac00-\ud7a3\u3131-\u318e]", line))]
  check("secure: no trace line shows what was typed (nor a syllable composed of it)",
        not shown, shown[:3])
  xim = LABEL.endswith("xim")
  if WAYLAND:
    told = re.findall(r'^\[ime\] commit #\d+.*surrounding "([^"]*)".*purpose (\w+)', traced, re.M)
    check("secure: text-input-v3 was told marks and the password purpose",
          bool(told) and all(set(text) <= {"•"} and purpose == "Password" for text, purpose in told),
          told[-2:])
  elif os.path.exists(im_log):
    with open(im_log, encoding="utf-8", errors="replace") as file:
      bus = file.read()[bus_since:]
    leaked = [run for run in trigrams if run in bus]
    check("secure: nothing that crossed the input method's bus shows the text", not leaked,
          leaked)
    if not xim:
      check("secure: the input method was told marks around the caret",
            re.search(r'member=SetSurroundingText.*\n(?:.*\n){0,12}?\s+string "•+"', bus) is not None)
      if LABEL.endswith("fcitx"):
        told = any(int(value) & (1 << 3)
                   for value in re.findall(r"member=SetCapability[^\n]*\n\s+uint64 (\d+)", bus))
        check("secure: Fcitx5 was told the field holds a password (capability bit 3)", told)
      else:
        told = re.search(r'string "ContentType"\s+variant\s+struct \{\s+uint32 8', bus) is not None
        check("secure: IBus was told the field holds a password (purpose 8)", told)
    else:
      note("XIM carries no text of the field, and no purpose: it is handed the keys alone")


def scenario_reader():
  # What a reader can ask of the field (textprobe.py report) and every EditableText method
  # (textprobe.py edit), after typing "Hello brave world" for real; the same against GtkEntry
  # with ATSPI_WINDOW="GTK reference". X11 only (xdotool gives the window the keyboard).
  subprocess.run(["xdotool", "search", "--sync", "--name", f"^{WINDOW}$", "windowfocus", "--sync"],
                 capture_output=True, timeout=20, check=False)
  name = None
  for _, node in walk(app()):
    try:
      if node.get_role_name() in ("entry", "text") and "editable" in states(node):
        name = node
        break
    except GLib.Error:
      pass
  if WINDOW == "controls":
    click(name)
  settle(0.5)
  drive("type", "Hello brave world")
  settle(0.5)
  check("reader: the field reads what was typed", text(name) == "Hello brave world",
        repr(text(name)))
  probe = os.path.join(ROOT, "tools/controls/textprobe.py")
  for mode in ["report", "edit"]:
    with open(os.path.join(OUT, f"{mode}.txt"), "w", encoding="utf-8") as file:
      subprocess.run(["python3.12", "-I", probe, mode], stdout=file, stderr=subprocess.STDOUT,
                     timeout=120, check=False, env={**os.environ, "ATSPI_WINDOW": WINDOW})
    with open(os.path.join(OUT, f"{mode}.txt"), encoding="utf-8") as file:
      errors = [line.strip() for line in file if "ERROR" in line]
    note(f"textprobe {mode}: {len(errors)} errors {errors}")


def scenario_focus():
  # A click on the first field (for the screen reader's run, which starts on the field).
  click(field("Name"))


def scenario_dump():
  for depth, node in walk(app()):
    try:
      line = f"{'  ' * depth}{node.get_role_name()} {node.get_name()!r} {sorted(states(node))}"
      try:
        line += f" {extents(node)}"
      except Exception:  # noqa: BLE001
        pass
    except GLib.Error as error:
      line = f"{'  ' * depth}? {error}"
    print(line)


def scenario_inspect():
  # Everything a reader is told of each object: its role, name, description, every state, its
  # interfaces, attributes, relations, actions and, for a text, its default attributes, so the
  # drawn window and the GTK reference compare line by line.
  every = [name for name in dir(Atspi.StateType) if name.isupper() and name != "LAST_DEFINED"]
  for depth, node in walk(app()):
    pad = "  " * depth
    try:
      role, name = node.get_role_name(), node.get_name()
      state_set = node.get_state_set()
      held = sorted(name.lower() for name in every if state_set.contains(getattr(Atspi.StateType, name)))
      print(f"{pad}{role} {name!r} description={node.get_description()!r}")
      print(f"{pad}  states {held}")
      print(f"{pad}  interfaces {sorted(node.get_interfaces())}")
      print(f"{pad}  attributes {dict(sorted((node.get_attributes() or {}).items()))}")
      for relation in node.get_relation_set() or []:
        targets = [relation.get_target(index) for index in range(relation.get_n_targets())]
        print(f"{pad}  relation {relation.get_relation_type().value_nick} "
              f"{[(target.get_role_name(), target.get_name()) for target in targets]}")
      if "Action" in node.get_interfaces():
        actions = [(Atspi.Action.get_action_name(node, index), Atspi.Action.get_key_binding(node, index))
                   for index in range(Atspi.Action.get_n_actions(node))]
        print(f"{pad}  actions {actions}")
      if "Text" in node.get_interfaces():
        count = Atspi.Text.get_character_count(node)
        print(f"{pad}  text {Atspi.Text.get_text(node, 0, -1)!r} count {count} "
              f"caret {Atspi.Text.get_caret_offset(node)}")
        print(f"{pad}  default attributes {dict(sorted(Atspi.Text.get_default_attributes(node).items()))}")
    except GLib.Error as error:
      print(f"{pad}? {error}")


def warm_up():
  # gnome-shell starts in its overview with the search entry focused: Escape leaves it and the
  # window takes focus. The first key is lost as the seat gains its keyboard.
  if os.environ.get("KIND") == "gnome":
    drive("key", "Shift_L", "sleep", "300", "key", "Escape", "sleep", "1000")


def main(argv):
  scenarios = argv[1:] or ["dump"]
  warm_up()
  for scenario in scenarios:
    {
      "controls": scenario_controls,
      "fields": scenario_fields,
      "hangul": scenario_hangul,
      "secure": scenario_secure,
      "dump": scenario_dump,
      "reader": scenario_reader,
      "focus": scenario_focus,
      "inspect": scenario_inspect,
    }[scenario]()
  print(f"[{LABEL}] DONE {' '.join(scenarios)}: {len(FAILED)} failed", flush=True)
  return 1 if FAILED else 0


if __name__ == "__main__":
  sys.exit(main(sys.argv))
