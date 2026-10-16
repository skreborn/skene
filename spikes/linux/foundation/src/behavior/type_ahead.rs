//! Finding an option by typing the start of its name.

use std::time::Duration;

/// What a list's typed letters have spelled: a buffer of the characters typed within a pause
/// of one another ([`TypeAhead::PAUSE`], its owner resetting it on the platform's clock),
/// matched against the options' names as Chromium's lists match them.
///
/// The same character typed again moves on to the next name starting with it; a longer
/// buffer finds the first name, from the current one on, that starts with it (ignoring case).
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct TypeAhead {
  buffer: String,
}

impl TypeAhead {
  /// How long after a character the next one continues the same search: its owner resets
  /// the buffer once this passes with nothing typed.
  pub const PAUSE: Duration = Duration::from_secs(1);

  /// An empty buffer.
  pub const fn new() -> Self {
    Self {
      buffer: String::new(),
    }
  }

  /// What has been typed so far.
  #[must_use]
  pub fn buffer(&self) -> &str {
    &self.buffer
  }

  /// Whether a search is under way: a `Space` continues it rather than choosing.
  #[must_use]
  pub const fn is_active(&self) -> bool {
    !self.buffer.is_empty()
  }

  /// Types `text` into a list of `names` whose current option is `current`: the option it
  /// finds, where one matches.
  pub fn type_text(&mut self, text: &str, names: &[&str], current: Option<usize>) -> Option<usize> {
    self.buffer.push_str(&text.to_lowercase());

    if names.is_empty() {
      return None;
    }

    let mut chars = self.buffer.chars();
    let first = chars.next()?;
    let repeated = chars.all(|other| other == first);
    let (prefix, start) = if repeated {
      (first.to_string(), current.map_or(0, |current| current + 1))
    } else {
      (self.buffer.clone(), current.unwrap_or(0))
    };

    (0..names.len())
      .map(|offset| (start + offset) % names.len())
      .find(|&index| names[index].to_lowercase().starts_with(&prefix))
  }

  /// Forgets what was typed.
  pub fn reset(&mut self) {
    self.buffer.clear();
  }
}
