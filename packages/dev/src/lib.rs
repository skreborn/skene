//! The dev loop's protocol between the `skene` command and the app it runs: the exit
//! status asking for a respawn, the variables the command sets, the lines it writes on
//! a terminal app's control pipe, and the names a served page carries.

use std::path::PathBuf;

/// The exit status an app ends a swap with where it cannot become the next binary in
/// place, asking the launcher to spawn it.
pub const SWAP_EXIT: i32 = 75;

/// The variable naming the descriptor of the file a terminal app records its snapshot
/// to across a swap.
pub const STATE_VARIABLE: &str = "SKENE_DEV_STATE";

/// The variable naming the descriptor of the pipe a terminal app reads the launcher's
/// [`Control`] lines from.
pub const CONTROL_VARIABLE: &str = "SKENE_DEV_CONTROL";

/// The variable that, set, makes a server render's pages join the dev loop.
pub const DEV_VARIABLE: &str = "SKENE_DEV";

/// The variable holding the client hints a server render answers with, one
/// `Name: value` per line.
pub const HINTS_VARIABLE: &str = "SKENE_HINTS";

/// The variable holding the nonce a server render's inline scripts carry.
pub const NONCE_VARIABLE: &str = "SKENE_SCRIPT_NONCE";

/// The first line and the last of a server binary's `--routes` output, the routes one
/// per line between.
pub const ROUTES_MARKERS: [&str; 2] = ["--skene-routes--", "--skene-routes-end--"];

/// The name of the meta element marking a served page as one in the dev loop.
pub const DEV_META: &str = "skene-dev";

/// The page global the dev script defines, resolving to the snapshot a reload kept.
pub const STATE_GLOBAL: &str = "skene_state";

/// The page global the app defines, returning its current snapshot.
pub const SNAPSHOT_GLOBAL: &str = "skene_snapshot";

/// A line the launcher writes on a terminal app's control pipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
  /// Becomes the binary at the path, carrying the state across.
  Swap(PathBuf),
  /// Shows the text, a failed build's output, or clears it with none.
  Notice(Option<String>),
}

impl Control {
  /// The line this control is written as, without its newline; its text is escaped so
  /// it stays one line and [`parse`](Self::parse) reads it back as itself.
  #[must_use]
  pub fn to_line(&self) -> String {
    match self {
      Self::Swap(path) => format!("swap {}", escape(&path.to_string_lossy())),
      Self::Notice(Some(text)) => format!("notice {}", escape(text)),
      Self::Notice(None) => "notice".to_owned(),
    }
  }

  /// The control `line` stands for; `None` for a line naming none.
  #[must_use]
  pub fn parse(line: &str) -> Option<Self> {
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));

    match word {
      "swap" => Some(Self::Swap(PathBuf::from(unescape(rest)))),
      "notice" => Some(Self::Notice((!rest.is_empty()).then(|| unescape(rest)))),
      _ => None,
    }
  }
}

/// `text` with its backslashes, carriage returns and line feeds escaped.
fn escape(text: &str) -> String {
  let mut escaped = String::with_capacity(text.len());

  for character in text.chars() {
    match character {
      '\\' => escaped.push_str("\\\\"),
      '\n' => escaped.push_str("\\n"),
      '\r' => escaped.push_str("\\r"),
      other => escaped.push(other),
    }
  }

  escaped
}

/// `text` with the escapes [`escape`] writes undone; any other backslash stands as it
/// is.
fn unescape(text: &str) -> String {
  let mut unescaped = String::with_capacity(text.len());
  let mut characters = text.chars();

  while let Some(character) = characters.next() {
    if character != '\\' {
      unescaped.push(character);

      continue;
    }

    match characters.next() {
      Some('n') => unescaped.push('\n'),
      Some('r') => unescaped.push('\r'),
      Some('\\') | None => unescaped.push('\\'),
      Some(other) => {
        unescaped.push('\\');
        unescaped.push(other);
      }
    }
  }

  unescaped
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::Control;

  /// Every control reads back as itself, whatever its text holds.
  #[test]
  fn a_control_round_trips() {
    for control in [
      Control::Swap(PathBuf::from("/tmp/app build/app")),
      Control::Swap(PathBuf::from("C:\\apps\\new\\app.exe")),
      Control::Notice(Some(
        "error[E0308]: mismatched types\n --> src/lib.rs:4:5".to_owned(),
      )),
      Control::Notice(Some("a literal \\n, a \\\\ and a trailing \\".to_owned())),
      Control::Notice(Some("\r\n".to_owned())),
      Control::Notice(None),
    ] {
      let line = control.to_line();

      assert!(!line.contains('\n'), "one line: {line:?}");
      assert_eq!(Control::parse(&line), Some(control), "{line:?}");
    }
  }

  /// A line naming no control is none, and an empty notice clears.
  #[test]
  fn a_line_names_its_control() {
    assert_eq!(Control::parse("reload"), None);
    assert_eq!(Control::parse(""), None);
    assert_eq!(Control::parse("notice"), Some(Control::Notice(None)));
  }
}
