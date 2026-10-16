#!/usr/bin/python3.12 -I
"""What a screen reader reads of the app over AT-SPI, through libatspi (the library Orca's
pyatspi wraps), in a separate process.

  atspi.py dump [--app NAME]                      print the tree: role, name, states, text
  atspi.py wait-text VALUE [--timeout S]          wait until the text field's text is VALUE
  atspi.py do ROLE NAME ACTION                    run an action ("click") on a node
  atspi.py focus ROLE NAME                        grab focus through Component
  atspi.py set-text VALUE                         EditableText.SetTextContents on the field
  atspi.py set-caret OFFSET                       Text.SetCaretOffset on the field
  atspi.py events SECONDS                         print the events a reader would hear
"""

import os
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

APP = "drawn-base"


def apps():
  desktop = Atspi.get_desktop(0)
  return [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]


def find_app(timeout=10.0, window=os.environ.get("ATSPI_WINDOW", "Drawn base")):
  end = time.time() + timeout
  while time.time() < end:
    for app in apps():
      if app is None:
        continue
      try:
        for i in range(app.get_child_count()):
          child = app.get_child_at_index(i)
          if child is not None and child.get_name() == window:
            return app
      except GLib.Error:
        pass
    time.sleep(0.1)
  names = [a.get_name() if a else None for a in apps()]
  raise SystemExit(f"app with window {window!r} not found; apps: {names}")


def states(node):
  state_set = node.get_state_set()
  wanted = [
    "focused", "focusable", "checked", "pressed", "editable", "single_line", "multi_line",
    "enabled", "sensitive", "showing", "visible", "active", "selectable_text",
  ]
  return [s for s in wanted if state_set.contains(getattr(Atspi.StateType, s.upper()))]


def interfaces(node):
  try:
    return node.get_interfaces()
  except GLib.Error:
    return []


def describe(node):
  role = node.get_role_name()
  line = f"{role} {node.get_name()!r} [{', '.join(states(node))}]"
  ifaces = interfaces(node)
  if "Text" in ifaces:
    text = node.get_text_iface() if hasattr(node, "get_text_iface") else node
    count = Atspi.Text.get_character_count(node)
    content = Atspi.Text.get_text(node, 0, count)
    caret = Atspi.Text.get_caret_offset(node)
    selections = []
    for i in range(Atspi.Text.get_n_selections(node)):
      r = Atspi.Text.get_selection(node, i)
      selections.append((r.start_offset, r.end_offset))
    line += f" text={content!r} chars={count} caret={caret} selections={selections}"
  if "Value" in ifaces:
    line += f" value={Atspi.Value.get_current_value(node)}"
  if "Action" in ifaces:
    names = [Atspi.Action.get_action_name(node, i) for i in range(Atspi.Action.get_n_actions(node))]
    line += f" actions={names}"
  if "Component" in ifaces:
    e = Atspi.Component.get_extents(node, Atspi.CoordType.WINDOW)
    line += f" extents=({e.x},{e.y},{e.width}x{e.height})"
  line += f" ifaces={ifaces}"
  return line


def walk(node, depth=0, out=None):
  if out is None:
    out = []
  out.append((depth, node))
  for i in range(node.get_child_count()):
    child = node.get_child_at_index(i)
    if child is not None:
      walk(child, depth + 1, out)
  return out


def find(app, role, name):
  for _, node in walk(app):
    if node.get_role_name() == role and (name is None or node.get_name() == name):
      return node
  raise SystemExit(f"no {role} named {name!r}")


def find_field(app):
  for _, node in walk(app):
    if "EditableText" in interfaces(node):
      return node
  raise SystemExit("no node with EditableText")


def main(argv):
  command = argv[1] if len(argv) > 1 else "dump"
  app = find_app()
  if command == "dump":
    print(f"application {app.get_name()!r} toolkit={app.get_toolkit_name()!r} pid={app.get_process_id()}")
    for depth, node in walk(app):
      print("  " * depth + describe(node))
  elif command == "wait-text":
    value = argv[2]
    timeout = float(argv[4]) if len(argv) > 4 else 10.0
    field = find_field(app)
    end = time.time() + timeout
    while time.time() < end:
      count = Atspi.Text.get_character_count(field)
      text = Atspi.Text.get_text(field, 0, count)
      if text == value:
        print(f"field text is {text!r}")
        return
      time.sleep(0.1)
    raise SystemExit(f"field text is {text!r}, wanted {value!r}")
  elif command == "do":
    node = find(app, argv[2], argv[3])
    for i in range(Atspi.Action.get_n_actions(node)):
      if Atspi.Action.get_action_name(node, i) == argv[4]:
        print(f"{argv[4]} -> {Atspi.Action.do_action(node, i)}")
        return
    raise SystemExit(f"no action {argv[4]}")
  elif command == "focus":
    node = find(app, argv[2], argv[3])
    print(f"grab_focus -> {Atspi.Component.grab_focus(node)}")
  elif command == "set-text":
    field = find_field(app)
    print(f"set_text_contents -> {Atspi.EditableText.set_text_contents(field, argv[2])}")
  elif command == "set-caret":
    field = find_field(app)
    print(f"set_caret_offset -> {Atspi.Text.set_caret_offset(field, int(argv[2]))}")
  elif command == "events":
    seconds = float(argv[2])

    def on_event(event):
      try:
        source = f"{event.source.get_role_name()} {event.source.get_name()!r}"
      except GLib.Error:
        source = "?"
      print(f"event {event.type} {source} detail=({event.detail1},{event.detail2}) any={event.any_data!r}", flush=True)

    listener = Atspi.EventListener.new(on_event)
    for kind in ["object:", "focus:", "window:"]:
      listener.register(kind)
    GLib.timeout_add(int(seconds * 1000), lambda: Atspi.event_quit())
    print("listening", flush=True)
    Atspi.event_main()
  else:
    raise SystemExit(__doc__)


if __name__ == "__main__":
  main(sys.argv)
