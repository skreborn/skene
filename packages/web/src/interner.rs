//! Content-addressed CSS class interning, shared by the web leaves.
//!
//! The pure core both web platforms build a stylesheet on: a node's full rule template
//! (its base declarations plus variants, transitions and keyframes, with the class name
//! as a placeholder) is interned to a shared class name and reference-counted by its live
//! users, so an unused rule is collected. The name is a 128-bit hash of the template
//! text in hex (`skene-{hash:x}`), so it depends on nothing but the template: a node
//! styled the same way gets the same class whether it renders to a live DOM or an HTML
//! string, whatever was interned before it and whatever is still alive—the parity
//! hydration relies on.
//!
//! Each leaf layers its own sink on the outcomes reported here: the browser adds a rule's
//! realized text to the document when one is [newly interned](ClassInterner::class_for)
//! and removes it when it is [collected](ClassInterner::release); the server ignores both
//! and renders the [live rules](ClassInterner::live_rules) to text once, at the end of a
//! render, in interning order.

use std::rc::Rc;

use rustc_hash::FxHashMap;

/// A generated class rule: the template it was keyed on, how many live nodes use it,
/// and its place in interning order.
struct Rule {
  template: Rc<str>,
  count: u32,
  #[cfg_attr(
    all(not(skene_leaf = "host"), not(skene_leaf = "server")),
    expect(dead_code)
  )]
  order: u64,
}

/// Interns rule templates to a shared, content-addressed, reference-counted class name.
pub(crate) struct ClassInterner {
  /// Template -> generated class name, so identical templates dedup to one class.
  classes: FxHashMap<Rc<str>, Rc<str>>,
  /// Class name -> its rule and reference count.
  rules: FxHashMap<Rc<str>, Rule>,
  /// The interning-order stamp the next rule takes.
  next_order: u64,
}

/// The 128-bit FNV-1a hash of `text`.
///
/// Content addressing needs a wide, deterministic hash of the template text and nothing
/// else; the name it yields is identical on every leaf and independent of intern order.
#[must_use]
pub(crate) fn content_hash(text: &str) -> u128 {
  const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
  const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

  let mut hash = OFFSET;

  for byte in text.bytes() {
    hash ^= u128::from(byte);
    hash = hash.wrapping_mul(PRIME);
  }

  hash
}

/// The class name for a template: `skene-` and the template's content hash in hex.
#[must_use]
pub(crate) fn class_name(template: &str) -> String {
  format!("skene-{:032x}", content_hash(template))
}

impl ClassInterner {
  pub(crate) fn new() -> Self {
    Self {
      classes: FxHashMap::default(),
      rules: FxHashMap::default(),
      next_order: 0,
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

    let class: Rc<str> = class_name(template).into();
    let template: Rc<str> = template.into();
    let order = self.next_order;

    self.next_order += 1;
    self.rules.insert(
      Rc::clone(&class),
      Rule {
        template: Rc::clone(&template),
        count: 0,
        order,
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

      return true;
    }

    false
  }

  /// Each live rule as `(class, template)` in interning order, for a leaf that renders
  /// them in a batch—the server sink only; the browser mirrors rules into the document
  /// incrementally instead.
  #[cfg(skene_leaf = "server")]
  pub(crate) fn live_rules(&self) -> impl Iterator<Item = (&str, &str)> {
    let mut live: Vec<(&Rule, &str)> = self
      .rules
      .iter()
      .filter(|(_, rule)| rule.count > 0)
      .map(|(class, rule)| (rule, &**class))
      .collect();

    live.sort_unstable_by_key(|(rule, _)| rule.order);

    live
      .into_iter()
      .map(|(rule, class)| (class, &*rule.template))
  }
}

#[cfg(test)]
mod tests {
  use super::{ClassInterner, class_name};

  /// A name depends on the template alone: not on what was interned before it, nor on
  /// whether the template was interned, released and interned again.
  #[test]
  fn names_are_content_addressed() {
    let mut first = ClassInterner::new();
    let mut second = ClassInterner::new();
    let (a1, _) = first.class_for("a");
    let (b1, _) = first.class_for("b");
    let (b2, _) = second.class_for("b");
    let (a2, _) = second.class_for("a");

    assert_eq!(a1, a2);
    assert_eq!(b1, b2);
    assert_ne!(a1, b1);
    assert_eq!(&*a1, class_name("a"));

    first.retain(&a1);

    assert!(first.release(&a1));
    assert_eq!(first.class_for("a").0, a1);
  }

  /// Interning reports newness once per live template, and releasing the last user
  /// collects it.
  #[test]
  fn refcounts_collect_unused_rules() {
    let mut interner = ClassInterner::new();
    let (class, fresh) = interner.class_for("x");

    assert!(fresh);
    assert!(!interner.class_for("x").1);

    interner.retain(&class);
    interner.retain(&class);

    assert!(!interner.release(&class));
    assert!(interner.release(&class));
    assert!(interner.class_for("x").1);
  }
}
