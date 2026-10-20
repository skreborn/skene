#!/usr/bin/python3.12 -I
"""A GTK 4.14 entry and password entry, for comparing which keys typed into each reach the
input method as key events: a window titled "GTK password" (GTK_REF_TITLE names another) holding
a GtkEntry named "Name" and a GtkPasswordEntry named "Password", each by its label.
"""

import os

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402


def on_activate(app):
  window = Gtk.ApplicationWindow(application=app, title=os.environ.get("GTK_REF_TITLE", "GTK password"))
  window.set_default_size(480, 240)
  box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
  box.set_margin_start(24)
  box.set_margin_end(24)
  box.set_margin_top(20)

  for label, entry in (("Name", Gtk.Entry()), ("Password", Gtk.PasswordEntry())):
    caption = Gtk.Label(label=label, xalign=0)
    caption.set_mnemonic_widget(entry)
    entry.connect("notify::text", lambda widget, _param, label=label: print(
      f"[app] {label.lower()}-length {len(widget.get_text())}", flush=True))
    box.append(caption)
    box.append(entry)

  window.set_child(box)
  window.present()
  # The session waits for the app's first frame.
  print("[frame] gtk", flush=True)


app = Gtk.Application(application_id="org.skene.GtkPassword")
app.connect("activate", on_activate)
app.run(None)
