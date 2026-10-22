//! The window an app draws in: its title, the latest one a live owner sets in force.

use std::cell::RefCell;

use skene_reactive::{Effect, on_cleanup, untrack};

use crate::spine::Realization;

thread_local! {
  /// The titles live owners set.
  static TITLES: RefCell<Titles> = RefCell::new(Titles::default());
}

/// The titles live owners set, in the order they were set, the latest in force.
#[derive(Debug, Default)]
struct Titles {
  entries: Vec<(u64, String)>,
  next: u64,
}

impl Titles {
  /// Sets `title`, in force until a later one is set; returns its id.
  fn add(&mut self, title: String) -> u64 {
    let id = self.next;

    self.next += 1;
    self.entries.push((id, title));

    id
  }

  /// Rewrites the title `id` in its place.
  fn rewrite(&mut self, id: u64, title: String) {
    if let Some((_, current)) = self.entries.iter_mut().find(|(at, _)| *at == id) {
      *current = title;
    }
  }

  /// Removes the title `id`.
  fn remove(&mut self, id: u64) {
    self.entries.retain(|(at, _)| *at != id);
  }

  /// The title in force.
  fn in_force(&self) -> Option<&str> {
    self.entries.last().map(|(_, title)| title.as_str())
  }

  /// Runs `change` on this thread's titles, then shows `P` the title in force where it
  /// changed.
  fn change<P: Realization, R>(change: impl FnOnce(&mut Self) -> R) -> R {
    let (result, shown) = TITLES.with(|titles| {
      let mut titles = titles.borrow_mut();
      let before = titles.in_force().map(ToOwned::to_owned);
      let result = change(&mut titles);
      let after = titles.in_force().map(ToOwned::to_owned);

      (result, (after != before).then_some(after))
    });

    if let Some(title) = shown {
      P::set_window_title(title.as_deref());
    }

    result
  }
}

/// Sets the window's title while the current owner lives; the latest title a live owner
/// set is in force.
pub fn set_title<P: Realization>(title: impl Into<String>) {
  let title = title.into();
  let id = Titles::change::<P, _>(|titles| titles.add(title));

  on_cleanup(move || Titles::change::<P, _>(|titles| titles.remove(id)));
}

/// Derives the window's title from the signals `title` reads, while the current owner
/// lives; the latest title a live owner set is in force.
pub fn derive_title<P: Realization>(title: impl Fn() -> String + 'static) {
  let first = untrack(&title);
  let id = Titles::change::<P, _>(|titles| titles.add(first));

  Effect::new(move |_: Option<()>| {
    let title = title();

    Titles::change::<P, _>(|titles| titles.rewrite(id, title));
  });
  on_cleanup(move || Titles::change::<P, _>(|titles| titles.remove(id)));
}

/// Forgets every title, as a fresh thread's window has none.
pub(in crate::spine) fn clear() {
  TITLES.with(|titles| *titles.borrow_mut() = Titles::default());
}

#[cfg(test)]
mod tests {
  use super::Titles;

  /// The latest live title is in force, a rewrite keeps its place, and removing the
  /// latest restores the one before.
  #[test]
  fn titles_stack() {
    let mut titles = Titles::default();
    let outer = titles.add("Outer".to_owned());
    let inner = titles.add("Inner".to_owned());

    assert_eq!(titles.in_force(), Some("Inner"));

    titles.rewrite(outer, "Outer 2".to_owned());
    titles.remove(inner);

    assert_eq!(titles.in_force(), Some("Outer 2"));

    titles.remove(outer);

    assert_eq!(titles.in_force(), None);
  }
}
