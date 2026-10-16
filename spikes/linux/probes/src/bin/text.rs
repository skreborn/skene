//! Text entries on real GTK, typed into with real X keys: each edit is reported before GTK
//! applies it; an edit the truth equals is applied natively (caret, undo and the input method
//! intact), one it does not is stopped and the truth spliced in with the caret mapped; writes
//! from the signal are smallest splices, never `set_text`, held while an input method
//! composes; the selection crosses in grapheme clusters.

use probes::{key, result, run, settle, show, type_text, wait, widget};
use skene_gui::{
  platform::linux::gtk::{self, prelude::*},
  prelude::*,
};

fn caret(entry: &gtk::Entry) -> i32 {
  entry.position()
}

/// Whether an input method composes in the node `handle` captured: GtkText's preedit, as the
/// realization tracks it from `preedit-changed`.
fn preedit(handle: &NodeHandle) -> bool {
  skene_gui::__private::composing(handle.node().expect("built"))
}

fn body() {
  // A handler that transforms (upper case) and rejects (digits).
  let code = Signal::new(String::new());
  let [code_field, name_field] = [NodeHandle::new(), NodeHandle::new()];
  let name = Signal::new(String::new());
  let shown = show(400, -1, || {
    Column::new()
      .set_padding(10)
      .set_spacing(10)
      .add_child(
        TextField::new("Code")
          .derive_text(move || code.get())
          .handle_change(move |text| {
            code.set(
              text
                .chars()
                .filter(|character| !character.is_ascii_digit())
                .collect::<String>()
                .to_uppercase(),
            );
          })
          .set_handle(&code_field),
      )
      .add_child(
        TextField::new("Name")
          .bind_text(name)
          .set_handle(&name_field),
      )
  });
  let entry = widget(&code_field)
    .downcast::<gtk::Entry>()
    .expect("a GtkEntry");

  shown.click(entry.upcast_ref());
  type_text("ab1c");

  result(
    "typed \"ab1c\": transformed and rejected in place, the caret after it",
    entry.text() == "ABC" && code.get() == "ABC" && caret(&entry) == 3,
    format!(
      "entry {:?}, signal {:?}, caret {}",
      entry.text(),
      code.get(),
      caret(&entry)
    ),
  );

  key("Left");
  type_text("x");

  result(
    "typed \"x\" mid-text: the caret stays mapped",
    entry.text() == "ABXC" && caret(&entry) == 3,
    format!("entry {:?}, caret {}", entry.text(), caret(&entry)),
  );

  // An accepted edit is GTK's own: its undo works, and the signal follows the undo.
  let field = widget(&name_field)
    .downcast::<gtk::Entry>()
    .expect("a GtkEntry");

  shown.click(field.upcast_ref());
  type_text("hello");

  let typed = (field.text().to_string(), name.get());

  key("ctrl+z");

  let undone = (field.text().to_string(), name.get());

  key("ctrl+shift+z");

  let redone = (field.text().to_string(), name.get());

  result(
    "accepted typing is GTK's own: Ctrl+Z undoes it, Ctrl+Shift+Z redoes it, the signal follows",
    typed == ("hello".to_owned(), "hello".to_owned())
      && undone.0 != "hello"
      && undone.0 == undone.1
      && redone == typed,
    format!("typed {typed:?}, undone {undone:?}, redone {redone:?}"),
  );

  // A write from the signal is the smallest splice: the caret before it stays put.
  key("Home");
  key("Right");
  key("Right");
  name.set("hello world".to_owned());
  settle();

  result(
    "a write from the signal splices, the caret before it unmoved",
    field.text() == "hello world" && caret(&field) == 2,
    format!("entry {:?}, caret {}", field.text(), caret(&field)),
  );

  type_text("!");

  result(
    "typing after the write goes where the caret was",
    field.text() == "he!llo world" && name.get() == "he!llo world",
    format!("entry {:?}, signal {:?}", field.text(), name.get()),
  );

  // An input method composing: GTK's own Ctrl+Shift+U entry. A write during the preedit
  // waits and applies when the composition ends, so the commit is the reader's edit of the
  // written text; a write while composing applies when the composition ends without one.
  name.set("hello".to_owned());
  settle();
  key("End");
  key("ctrl+shift+u");
  type_text("e9");

  let composing = preedit(&name_field);

  name.set("zz".to_owned());
  settle();

  let held = field.text().to_string();

  key("space");
  wait(100);
  settle();

  let ended = !preedit(&name_field);

  result(
    "a write during a preedit waits; the commit is the reader's edit of it",
    composing && ended && held == "hello" && field.text() == "zzé" && name.get() == "zzé",
    format!(
      "composing {composing}, then {}; during the preedit the write of \"zz\" left {held:?}; after the commit entry {:?}, signal {:?}",
      !ended,
      field.text(),
      name.get()
    ),
  );

  key("ctrl+shift+u");
  type_text("41");
  name.set("written".to_owned());
  settle();

  let during = field.text().to_string();

  key("Escape");
  wait(100);
  settle();

  result(
    "a write during a preedit the reader abandons applies when it ends",
    during == "zzé" && field.text() == "written" && name.get() == "written",
    format!("during {during:?}, after Escape {:?}", field.text()),
  );

  shown.close();

  // The selection, controlled, in grapheme clusters: "a", "é" (e and a combining acute, two
  // code points) and "b" are three clusters over four code points.
  let text = Signal::new("ae\u{301}b".to_owned());
  let selection = Signal::new(TextSelection { start: 0, end: 0 });
  let selected = NodeHandle::new();
  let shown = show(400, -1, || {
    TextField::new("Letters")
      .bind_text(text)
      .bind_selection(selection)
      .set_handle(&selected)
  });
  let entry = widget(&selected)
    .downcast::<gtk::Entry>()
    .expect("a GtkEntry");

  entry.grab_focus();
  settle();
  selection.set(TextSelection { start: 1, end: 2 });
  settle();

  let bounds = entry.selection_bounds();

  result(
    "a selection of cluster 1..2 selects code points 1..3 (the whole é)",
    bounds == Some((1, 3)),
    format!("{bounds:?}"),
  );

  key("End");
  key("shift+Left");
  key("shift+Left");

  let moved = selection.get();
  let bounds = entry.selection_bounds();

  result(
    "Shift+Left twice from the end reports a backward selection in clusters",
    moved == TextSelection { start: 3, end: 1 } && bounds == Some((1, 4)),
    format!("signal {moved:?}, GTK code points {bounds:?}"),
  );

  shown.close();

  // A text editor: each edit reported before its buffer applies it, and a truth it does not
  // equal shown as the smallest splice instead.
  let notes = Signal::new(String::new());
  let editor = NodeHandle::new();
  let shown = show(400, -1, || {
    TextEditor::new("Notes")
      .derive_text(move || notes.get())
      .handle_change(move |text| notes.set(text.replace(char::is_numeric, "").to_uppercase()))
      .set_handle(&editor)
  });
  let view = widget(&editor)
    .downcast::<gtk::TextView>()
    .expect("a GtkTextView");

  shown.click(view.upcast_ref());
  type_text("ab");
  key("Return");
  type_text("c1d");

  let buffer = view.buffer();
  let shown_text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
  let cursor = buffer.iter_at_mark(&buffer.get_insert()).offset();

  result(
    "a text editor's edits are reported before they apply: \"ab⏎c1d\" shows \"AB⏎CD\", caret at the end",
    shown_text == "AB\nCD" && notes.get() == "AB\nCD" && cursor == 5,
    format!(
      "buffer {shown_text:?}, signal {:?}, caret {cursor}",
      notes.get()
    ),
  );

  notes.set("AB\nCD\nEF".to_owned());
  settle();

  let written = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);

  result(
    "a write to the editor from the signal shows",
    written == "AB\nCD\nEF",
    format!("{written:?}"),
  );

  shown.close();
}

fn main() -> std::process::ExitCode {
  run("text", body)
}
