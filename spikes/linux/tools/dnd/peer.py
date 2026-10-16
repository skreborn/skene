#!/usr/bin/env python3
"""Another application for drag and drop: GTK 4, the toolkit whose behaviour Skene's follows.

  peer.py <entry text> <file to offer>

A window "Skene peer" holding a text field "Peer field" (a drag source and a drop target for
text, GTK's own), a box "Drop here" that takes text and files and shows what it took, and a label
"Drag file" that drags the file given as a URI list. It prints, a line each:

  [peer] ready <width> <height>    once shown (its size)
  [peer] place <name> <x> <y> <w> <h>  where each of field, drop and source sits in its window
  [peer] pointer <x> <y>           where it sees the pointer in its window, as it moves
  [peer] dropped text <text>       what the box took
  [peer] dropped uris <uri> ...
  [peer] field <text>              the field's text, as it changes

Where it runs on X11 with no window manager (a private Xvfb), it moves its window to
SKENE_PEER_X, SKENE_PEER_Y with xdotool; on sway, with swaymsg; elsewhere the compositor places
it. It needs PyGObject with GTK 4 (run it with the Python that has them, `-I`).
"""

import os
import subprocess
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, GObject, Gtk  # noqa: E402

TEXT = sys.argv[1] if len(sys.argv) > 1 else "peer text"
FILE = sys.argv[2] if len(sys.argv) > 2 else "/tmp/skene-peer.txt"
X = int(os.environ.get("SKENE_PEER_X", "900"))
Y = int(os.environ.get("SKENE_PEER_Y", "40"))


def say(*words):
  print("[peer]", *words, flush=True)


def place(window):
  if os.environ.get("SWAYSOCK"):
    subprocess.run(
      ["swaymsg", f"[app_id=dev.skene.peer] floating enable, move absolute position {X} {Y}"],
      stdout=subprocess.DEVNULL,
      stderr=subprocess.DEVNULL,
      check=False,
    )
  elif os.environ.get("DISPLAY") and not os.environ.get("WAYLAND_DISPLAY"):
    try:
      gi.require_version("GdkX11", "4.0")
      from gi.repository import GdkX11  # noqa: F401

      xid = window.get_surface().get_xid()
      subprocess.run(["xdotool", "windowmove", str(xid), str(X), str(Y)], check=False)
    except (ValueError, AttributeError):
      pass


def on_activate(app):
  window = Gtk.ApplicationWindow(application=app, title="Skene peer")
  window.set_default_size(360, 260)
  box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
  box.set_margin_start(16)
  box.set_margin_end(16)
  box.set_margin_top(16)
  box.set_margin_bottom(16)

  caption = Gtk.Label(label="Peer field", xalign=0)
  field = Gtk.Entry()
  field.set_text(TEXT)
  caption.set_mnemonic_widget(field)
  field.connect("notify::text", lambda entry, _: say("field", entry.get_text()))

  drop = Gtk.Label(label="Drop here")
  drop.set_size_request(-1, 60)
  drop.add_css_class("frame")
  target = Gtk.DropTarget.new(GObject.TYPE_NONE, Gdk.DragAction.COPY)
  target.set_gtypes([Gdk.FileList, GObject.TYPE_STRING])

  def dropped(_, value, x, y):
    if isinstance(value, Gdk.FileList):
      uris = [file.get_uri() for file in value.get_files()]
      say("dropped uris", *uris)
      drop.set_label("Took " + " ".join(uris))
    else:
      say("dropped text", value)
      drop.set_label("Took " + value)
    return True

  target.connect("drop", dropped)
  drop.add_controller(target)

  source_label = Gtk.Label(label="Drag file")
  source_label.set_size_request(-1, 40)
  source = Gtk.DragSource(actions=Gdk.DragAction.COPY)

  def prepare(_, x, y):
    files = Gdk.FileList.new_from_list([Gio.File.new_for_path(FILE)])
    return Gdk.ContentProvider.new_for_value(GObject.Value(Gdk.FileList, files))

  source.connect("prepare", prepare)
  source_label.add_controller(source)

  motion = Gtk.EventControllerMotion()
  motion.connect("motion", lambda _, x, y: say("pointer", f"{x:.1f}", f"{y:.1f}"))
  motion.connect("enter", lambda _, x, y: say("pointer", f"{x:.1f}", f"{y:.1f}"))
  window.add_controller(motion)

  box.append(caption)
  box.append(field)
  box.append(drop)
  box.append(source_label)
  window.set_child(box)
  window.present()

  def shown():
    place(window)
    field.grab_focus()
    field.select_region(0, -1)
    say("ready", window.get_width(), window.get_height())
    for name, widget in (("field", field), ("drop", drop), ("source", source_label)):
      ok, bounds = widget.compute_bounds(window)
      say(
        "place",
        name,
        f"{bounds.get_x():.1f}",
        f"{bounds.get_y():.1f}",
        f"{bounds.get_width():.1f}",
        f"{bounds.get_height():.1f}",
      )
    return False

  GLib.timeout_add(300, shown)


app = Gtk.Application(application_id="dev.skene.peer")
app.connect("activate", on_activate)
app.run([])
