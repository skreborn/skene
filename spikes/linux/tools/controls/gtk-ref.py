#!/usr/bin/python3.12 -I
"""The GTK 4.14 reference: the drawn shell's controls as native GTK widgets, for comparing
what AT-SPI reports and what Orca says.

  gtk-ref.py [--announce]    --announce also announces the status through
                             gtk_accessible_announce (4.14), the closest GTK has to a live region
"""

import sys

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402

ANNOUNCE = any(a.startswith("--announce") for a in sys.argv)
# The widget gtk_accessible_announce is called on: the status label, or with --announce-window
# the window.
ANNOUNCER = [None]


def on_activate(app):
  window = Gtk.ApplicationWindow(application=app, title="GTK reference")
  window.set_default_size(480, 320)
  box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
  box.set_margin_start(24)
  box.set_margin_end(24)
  box.set_margin_top(20)
  heading = Gtk.Label(label="GTK reference", xalign=0, accessible_role=Gtk.AccessibleRole.HEADING)
  heading.add_css_class("title-1")
  name = Gtk.Label(label="Name", xalign=0)
  entry = Gtk.Entry()
  # GTK sets the labelled-by relation from a label's mnemonic widget.
  name.set_mnemonic_widget(entry)
  row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
  wifi = Gtk.Label(label="Wi-Fi", xalign=0)
  switch = Gtk.Switch()
  wifi.set_mnemonic_widget(switch)
  switch.update_property([Gtk.AccessibleProperty.DESCRIPTION], ["Connects to wireless networks"])
  row.append(switch)
  row.append(wifi)
  button = Gtk.Button(label="Submit")
  button.set_halign(Gtk.Align.START)
  status = Gtk.Label(label="", xalign=0)
  state = {"presses": 0}
  ANNOUNCER[0] = window if "--announce-window" in sys.argv else status

  def report(text):
    status.set_label(text)
    print(f"[gtk] status {text}", flush=True)

    if ANNOUNCE:
      ANNOUNCER[0].announce(text, Gtk.AccessibleAnnouncementPriority.MEDIUM)

  def on_switch(sw, _param):
    report(f"Wi-Fi {'on' if sw.get_active() else 'off'}")

  def on_click(_button):
    state["presses"] += 1
    report(f'Submitted "{entry.get_text()}" ({state["presses"]})')

  switch.connect("notify::active", on_switch)
  button.connect("clicked", on_click)
  entry.connect("activate", lambda _e: on_click(None))

  for child in [heading, name, entry, row, button, status]:
    box.append(child)

  window.set_child(box)
  window.present()
  entry.grab_focus()


app = Gtk.Application(application_id="org.skene.GtkReference")
app.connect("activate", on_activate)
app.run([sys.argv[0]])
