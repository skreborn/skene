//! The terminal's text field over the shared editing core, its tests ported from the old
//! terminal's `text_entry`, `entry_selection` and `clusters` suites where they apply to a
//! field alone.

mod clusters;
mod composition;
mod entry;
mod selection;

use skene_foundation::seam::{
  content::Edit,
  report::{KeyEvent, Modifiers},
};
use skene_terminal::field::{KeyOutcome, TextField};

/// A field of `columns` holding `value`, focused by keyboard.
fn field(value: &str, columns: u16) -> TextField {
  let mut field = TextField::new(columns);

  field.editor_mut().write(&Edit::text(value));
  field.focus(true);

  field
}

/// An editor of `columns` by `rows` holding `value`, focused by keyboard.
fn editor(value: &str, columns: u16, rows: u16) -> TextField {
  let mut field = TextField::multiline(columns, rows);

  field.editor_mut().write(&Edit::text(value));
  field.focus(true);

  field
}

/// A key press named as the old tests name it (`"Ctrl+Shift+z"`), a paste reading `clipboard`.
fn key(field: &mut TextField, name: &str) -> KeyOutcome {
  let mut modifiers = Modifiers::default();
  let mut parts: Vec<&str> = name.split('+').collect();
  let key = parts.pop().expect("a key");

  for part in parts {
    match part {
      "Ctrl" => modifiers.control = true,
      "Shift" => modifiers.shift = true,
      "Alt" => modifiers.alt = true,
      _ => panic!("unknown modifier {part}"),
    }
  }

  field.key(&KeyEvent {
    key: key.to_owned(),
    code: String::new(),
    down: true,
    repeat: false,
    modifiers,
  })
}

fn keys(field: &mut TextField, names: &[&str]) {
  for name in names {
    let _ = key(field, name);
  }
}

fn value(field: &TextField) -> &str {
  field.editor().text()
}
