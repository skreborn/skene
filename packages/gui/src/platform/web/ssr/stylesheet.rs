//! The server's stylesheet sink: a text render over the shared class interner.
//!
//! Holds no live sheet. Interning, naming and refcounting live in the shared [`ClassInterner`];
//! this wrapper ignores the intern/collect outcomes and, once the render has settled,
//! [renders](Self::render) the live rules to a `<style>` block. Refcounting still matters: a
//! subtree torn down mid-render (a loading placeholder swapped for its loaded tree) releases its
//! classes, so only the settled tree's styles reach the output.

use std::rc::Rc;

use super::super::{Declarations, interner::ClassInterner};

pub(super) struct StyleStore {
  interner: ClassInterner,
}

impl StyleStore {
  pub(super) fn new() -> StyleStore {
    StyleStore {
      interner: ClassInterner::new(),
    }
  }

  /// Returns the class for `declarations`, interning it the first time the set is seen.
  pub(super) fn class_for(&mut self, declarations: &Declarations) -> Rc<str> {
    self.interner.class_for(declarations).0
  }

  /// Records that one more node is using `class`.
  pub(super) fn retain(&mut self, class: &str) {
    self.interner.retain(class);
  }

  /// Records that one node stopped using `class`.
  pub(super) fn release(&mut self, class: &str) {
    self.interner.release(class);
  }

  /// Renders every live rule as a CSS block, in class-name order so the output is deterministic.
  pub(super) fn render(&self) -> String {
    let mut live: Vec<(&str, &Declarations)> = self.interner.live_rules().collect();

    live.sort_unstable_by_key(|(class, _)| *class);

    let mut out = String::new();

    for (class, declarations) in live {
      out.push('.');
      out.push_str(class);
      out.push('{');
      out.push_str(&declarations.render());
      out.push('}');
    }

    out
  }
}
