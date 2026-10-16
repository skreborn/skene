#!/usr/bin/python3.12 -I
"""Everything a reader can ask of a window's text field and controls over AT-SPI, through
libatspi in a separate process, and the D-Bus introspection of the field's object.

  textprobe.py report        roles, states, relations, attributes, interfaces of every node;
                             the field's Text methods; the D-Bus introspection
  textprobe.py edit          every EditableText method on the field, each followed by GetText
Set ATSPI_WINDOW to the window's name; the field's text should be "Hello brave world".
"""

import os
import sys
import warnings
import xml.etree.ElementTree as ET

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, Gio, GLib  # noqa: E402

warnings.simplefilter("ignore", DeprecationWarning)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi  # noqa: E402


def all_states(node):
  state_set = node.get_state_set()
  return sorted(
    s.value_nick for s in Atspi.StateType.__enum_values__.values()
    if s != Atspi.StateType.LAST_DEFINED and state_set.contains(s)
  )


def relations(node):
  out = []
  for relation in node.get_relation_set():
    targets = [relation.get_target(i) for i in range(relation.get_n_targets())]
    names = [f"{t.get_role_name()} {t.get_name()!r}" for t in targets]
    out.append(f"{relation.get_relation_type().value_nick}->{names}")
  return out


def call(label, function, *args):
  try:
    result = function(*args)
  except GLib.Error as error:
    result = f"ERROR {error.message}"
  except Exception as error:  # noqa: BLE001
    result = f"ERROR {error}"
  if hasattr(result, "content"):
    result = (result.content, result.start_offset, result.end_offset)
  elif hasattr(result, "start_offset"):
    result = (result.start_offset, result.end_offset)
  elif hasattr(result, "x") and hasattr(result, "width"):
    result = (result.x, result.y, result.width, result.height)
  print(f"  {label} -> {result!r}", flush=True)
  return result


def a11y_bus():
  session = Gio.bus_get_sync(Gio.BusType.SESSION, None)
  reply = session.call_sync(
    "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress", None,
    GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, -1, None)
  address = reply.unpack()[0]
  return Gio.DBusConnection.new_for_address_sync(
    address,
    Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
    None, None)


def introspect(node):
  bus = a11y_bus()
  reply = bus.call_sync(
    node.app.bus_name, node.path, "org.freedesktop.DBus.Introspectable", "Introspect",
    None, GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, -1, None)
  root = ET.fromstring(reply.unpack()[0])
  for interface in root.findall("interface"):
    name = interface.get("name")
    if not name.startswith("org.a11y"):
      continue
    methods = [m.get("name") for m in interface.findall("method")]
    props = [p.get("name") for p in interface.findall("property")]
    signals = [s.get("name") for s in interface.findall("signal")]
    print(f"  {name}: methods={methods} properties={props} signals={signals}")


def report(app):
  print(f"application {app.get_name()!r} toolkit={app.get_toolkit_name()!r} "
        f"version={app.get_toolkit_version()!r} atspi={app.get_atspi_version()!r}")
  for depth, node in atspi.walk(app):
    pad = "  " * depth
    print(f"{pad}{node.get_role_name()} {node.get_name()!r} desc={node.get_description()!r}")
    print(f"{pad}  states={all_states(node)}")
    print(f"{pad}  relations={relations(node)}")
    print(f"{pad}  attributes={dict(node.get_attributes() or {})}")
    print(f"{pad}  interfaces={atspi.interfaces(node)}")
    if "Action" in atspi.interfaces(node):
      actions = [
        (Atspi.Action.get_action_name(node, i), Atspi.Action.get_localized_name(node, i),
         Atspi.Action.get_key_binding(node, i))
        for i in range(Atspi.Action.get_n_actions(node))
      ]
      print(f"{pad}  actions={actions}")
  field = atspi.find_field(app)
  print(f"--- Text on the field ({field.get_role_name()} {field.get_name()!r})")
  count = call("CharacterCount", Atspi.Text.get_character_count, field)
  call("GetText(0,-1)", Atspi.Text.get_text, field, 0, -1)
  call("GetText(0,count)", Atspi.Text.get_text, field, 0, count)
  call("CaretOffset", Atspi.Text.get_caret_offset, field)
  call("NSelections", Atspi.Text.get_n_selections, field)
  call("GetSelection(0)", Atspi.Text.get_selection, field, 0)
  for offset in [0, 7, 8]:
    for granularity in ["CHAR", "WORD", "SENTENCE", "LINE", "PARAGRAPH"]:
      call(f"GetStringAtOffset({offset},{granularity})", Atspi.Text.get_string_at_offset, field,
           offset, getattr(Atspi.TextGranularity, granularity))
    for boundary in ["CHAR", "WORD_START", "WORD_END", "LINE_START"]:
      call(f"GetTextAtOffset({offset},{boundary})", Atspi.Text.get_text_at_offset, field, offset,
           getattr(Atspi.TextBoundaryType, boundary))
  call("GetTextBeforeOffset(8,WORD_START)", Atspi.Text.get_text_before_offset, field, 8,
       Atspi.TextBoundaryType.WORD_START)
  call("GetTextAfterOffset(8,WORD_START)", Atspi.Text.get_text_after_offset, field, 8,
       Atspi.TextBoundaryType.WORD_START)
  call("GetCharacterAtOffset(1)", Atspi.Text.get_character_at_offset, field, 1)
  call("GetCharacterExtents(1,WINDOW)", Atspi.Text.get_character_extents, field, 1,
       Atspi.CoordType.WINDOW)
  call("GetRangeExtents(0,5,WINDOW)", Atspi.Text.get_range_extents, field, 0, 5,
       Atspi.CoordType.WINDOW)
  extents = call("GetCharacterExtents(7,WINDOW)", Atspi.Text.get_character_extents, field, 7,
                 Atspi.CoordType.WINDOW)
  if isinstance(extents, tuple):
    call("GetOffsetAtPoint(char 7 center)", Atspi.Text.get_offset_at_point, field,
         extents[0] + extents[2] // 2, extents[1] + extents[3] // 2, Atspi.CoordType.WINDOW)
  call("GetAttributeRun(1)", Atspi.Text.get_attribute_run, field, 1, True)
  call("GetDefaultAttributes", Atspi.Text.get_default_attributes, field)
  call("GetAttributes(1)", Atspi.Text.get_text_attributes, field, 1)
  call("SetSelection(0,0,5)", Atspi.Text.set_selection, field, 0, 0, 5)
  call("GetSelection(0)", Atspi.Text.get_selection, field, 0)
  call("AddSelection(6,11)", Atspi.Text.add_selection, field, 6, 11)
  call("NSelections", Atspi.Text.get_n_selections, field)
  call("RemoveSelection(0)", Atspi.Text.remove_selection, field, 0)
  call("NSelections", Atspi.Text.get_n_selections, field)
  call("SetCaretOffset(count)", Atspi.Text.set_caret_offset, field, count)
  print("--- D-Bus introspection of the field's object")
  introspect(field)


def edit(app):
  field = atspi.find_field(app)
  text = lambda: Atspi.Text.get_text(field, 0, Atspi.Text.get_character_count(field))  # noqa: E731
  print(f"--- EditableText on the field ({field.get_role_name()} {field.get_name()!r})")
  steps = [
    ("SetTextContents('Hello brave world')", Atspi.EditableText.set_text_contents,
     "Hello brave world"),
    ("InsertText(5, ',', 1)", Atspi.EditableText.insert_text, 5, ",", 1),
    ("DeleteText(0, 1)", Atspi.EditableText.delete_text, 0, 1),
    ("CopyText(0, 5)", Atspi.EditableText.copy_text, 0, 5),
    ("CutText(0, 5)", Atspi.EditableText.cut_text, 0, 5),
    ("PasteText(0)", Atspi.EditableText.paste_text, 0),
  ]
  print("--- does a write read back at once? (set, read immediately, read after 300 ms)")
  call("SetCaretOffset(3)", Atspi.Text.set_caret_offset, field, 3)
  call("  CaretOffset immediately", Atspi.Text.get_caret_offset, field)
  GLib.usleep(300000)
  call("  CaretOffset after 300 ms", Atspi.Text.get_caret_offset, field)
  call("SetTextContents('Synchronous?')", Atspi.EditableText.set_text_contents, field, "Synchronous?")
  print(f"    text immediately={text()!r}", flush=True)
  GLib.usleep(300000)
  print(f"    text after 300 ms={text()!r}", flush=True)
  call("SetSelection/AddSelection(0,5)", Atspi.Text.add_selection, field, 0, 5)
  call("  GetSelection(0) immediately", Atspi.Text.get_selection, field, 0)
  GLib.usleep(300000)
  call("  GetSelection(0) after 300 ms", Atspi.Text.get_selection, field, 0)
  print("--- EditableText methods")
  for label, function, *args in steps:
    call(label, function, field, *args)
    GLib.usleep(200000)
    print(f"    text={text()!r}", flush=True)


def main(argv):
  app = atspi.find_app()
  {"report": report, "edit": edit}[argv[1]](app)


if __name__ == "__main__":
  main(sys.argv)
