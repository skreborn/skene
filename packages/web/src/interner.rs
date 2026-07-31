//! Content-addressed CSS class interning, shared by the web leaves.
//!
//! The pure core both web platforms build a stylesheet on: a node's full rule template
//! (its base declarations plus variants, transitions and keyframes, with the class name
//! as a placeholder) is interned to a shared class name and reference-counted by its live
//! users, so an unused rule is collected. The naming (`skene-{hash:x}`, with a forward
//! probe on the rare collision) hashes the template text and is identical on every web
//! leaf, so a node styled the same way gets the same class whether it renders to a live
//! DOM or an HTML string—the parity hydration relies on.
//!
//! Each leaf layers its own sink on the outcomes reported here: the browser adds a rule's
//! realized text to the document when one is [newly interned](ClassInterner::class_for)
//! and removes it when it is [collected](ClassInterner::release); the server ignores both
//! and renders the [live rules](ClassInterner::live_rules) to text once, at the end of a
//! render.

use std::{
  hash::{Hash, Hasher},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};

/// A generated class rule: the template it was keyed on, and how many live nodes use it.
struct Rule {
  template: Rc<str>,
  count: u32,
}

/// Interns rule templates to a shared, content-addressed, reference-counted class name.
pub(crate) struct ClassInterner {
  /// Template -> generated class name, so identical templates dedup to one class.
  classes: FxHashMap<Rc<str>, Rc<str>>,
  /// Class name -> its rule and reference count.
  rules: FxHashMap<Rc<str>, Rule>,
  /// Name hashes already handed out, so a hash collision between two distinct templates
  /// still resolves to distinct class names.
  used_names: FxHashSet<u64>,
}

impl ClassInterner {
  pub(crate) fn new() -> Self {
    Self {
      classes: FxHashMap::default(),
      rules: FxHashMap::default(),
      used_names: FxHashSet::default(),
    }
  }

  /// Returns the class for `template`, and whether it was newly interned—a leaf adds
  /// the realized rule to its sink only then.
  ///
  /// The returned class has no claim on it yet: call [`retain`](Self::retain) once it is
  /// assigned to a node and [`release`](Self::release) when the node drops it.
  pub(crate) fn class_for(&mut self, template: &str) -> (Rc<str>, bool) {
    if let Some(class) = self.classes.get(template) {
      return (Rc::clone(class), false);
    }

    // Content-addressed name; probe forward on the rare hash collision so two distinct
    // templates never collapse onto the same class name.
    let mut tag = {
      let mut hasher = FxHasher::default();

      template.hash(&mut hasher);

      hasher.finish()
    };

    while !self.used_names.insert(tag) {
      tag = tag.wrapping_add(1);
    }

    let class: Rc<str> = format!("skene-{tag:x}").into();
    let template: Rc<str> = template.into();

    self.rules.insert(
      Rc::clone(&class),
      Rule {
        template: Rc::clone(&template),
        count: 0,
      },
    );

    self.classes.insert(template, Rc::clone(&class));

    (class, true)
  }

  /// Records that one more node is using `class`.
  pub(crate) fn retain(&mut self, class: &str) {
    if let Some(rule) = self.rules.get_mut(class) {
      rule.count += 1;
    }
  }

  /// Records that one node stopped using `class`, returning whether that was its last
  /// user—a leaf removes the rule from its sink only then.
  pub(crate) fn release(&mut self, class: &str) -> bool {
    let count = match self.rules.get_mut(class) {
      Some(rule) => {
        rule.count -= 1;

        rule.count
      }
      None => return false,
    };

    if count == 0 {
      let rule = self.rules.remove(class).expect("rule just decremented");

      self.classes.remove(&rule.template);

      let tag = class
        .strip_prefix("skene-")
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        .unwrap_or(0);

      self.used_names.remove(&tag);

      return true;
    }

    false
  }

  /// Each live rule as `(class, template)`, for a leaf that renders them in a batch—the
  /// server sink only; the browser mirrors rules into the document incrementally instead.
  #[cfg(target_os = "wasi")]
  pub(crate) fn live_rules(&self) -> impl Iterator<Item = (&str, &str)> {
    self
      .rules
      .iter()
      .filter(|(_, rule)| rule.count > 0)
      .map(|(class, rule)| (&**class, &*rule.template))
  }
}
