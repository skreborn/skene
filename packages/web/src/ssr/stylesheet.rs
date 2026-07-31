//! The server's stylesheet sink: a text render over the shared class interner.
//!
//! Holds no live sheet. Interning, naming and refcounting live in the shared
//! [`ClassInterner`]; this wrapper ignores the intern/collect outcomes and, once the
//! render has settled, [renders](Self::render) the live rules to a `<style>` block.
//! Refcounting still matters: a subtree torn down mid-render (a loading placeholder
//! swapped for its loaded tree) releases its classes, so only the settled tree's styles
//! reach the output.

use std::rc::Rc;

use crate::{interner::ClassInterner, mapping::CLASS_PLACEHOLDER};

pub(super) struct StyleStore {
  interner: ClassInterner,
}

impl StyleStore {
  pub(super) fn new() -> StyleStore {
    StyleStore {
      interner: ClassInterner::new(),
    }
  }

  /// Returns the class for `template`, interning it the first time it is seen.
  pub(super) fn class_for(&mut self, template: &str) -> Rc<str> {
    self.interner.class_for(template).0
  }

  /// Records that one more node is using `class`.
  pub(super) fn retain(&mut self, class: &str) {
    self.interner.retain(class);
  }

  /// Records that one node stopped using `class`.
  pub(super) fn release(&mut self, class: &str) {
    self.interner.release(class);
  }

  /// Renders every live rule, in class-name order so the output is deterministic.
  pub(super) fn render(&self) -> String {
    let mut live: Vec<(&str, &str)> = self.interner.live_rules().collect();

    live.sort_unstable_by_key(|(class, _)| *class);

    let mut out = String::new();

    for (class, template) in live {
      out.push_str(&template.replace(CLASS_PLACEHOLDER, class));
    }

    out
  }
}
