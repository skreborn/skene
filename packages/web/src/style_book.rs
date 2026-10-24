//! The stylesheet bookkeeping both web leaves share: the compiled templates by their
//! style's address and their construction's facts, the interned classes each node keeps and
//! releases, and the custom properties registered for tokens and slots, told to the leaf's
//! [`StyleSink`] as they change.

use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use skene_foundation::{Definitions, TokenName};

use crate::{
  compile::{Compiled, KeptStyle, Key, SlotBinding, Template, slot_registration},
  interner::ClassInterner,
  token::WebTokenValue,
};

/// The reset stylesheet every web leaf applies: injected into a `<style>` by the
/// browser, serialized into the document by a server.
///
/// It declares the framework's cascade layers and fills `skene.reset`, beneath the
/// compiled rules' `skene.style`.
pub(crate) const RESET_CSS: &str = include_str!("reset.css");

/// Opens the cascade layer every compiled rule sits in, which a `}` closes.
pub(crate) const STYLE_LAYER: &str = "@layer skene.style{";

/// Where a web leaf's stylesheet goes: the rules and registrations a [`StyleBook`]
/// adds, and the rules it collects.
pub(crate) trait StyleSink {
  /// Adds the rule the newly interned `class` realizes from `template`.
  fn add_rule(&mut self, class: &Rc<str>, template: &str);

  /// Removes the rule of `class`, whose last user released it.
  fn remove_rule(&mut self, class: &str);

  /// Adds the registrations of newly registered custom properties, in order.
  fn add_registrations(&mut self, registrations: impl Iterator<Item = String>);
}

/// One leaf's stylesheet bookkeeping over the sink its stylesheet goes to.
pub(crate) struct StyleBook<K> {
  /// The classes interned for the rule templates.
  interner: ClassInterner,
  /// The compiled templates, by their style's address and their template's key.
  compiled: FxHashMap<(u128, u64), Rc<Compiled>>,
  /// The tokens whose custom property is registered.
  tokens: FxHashSet<TokenName>,
  /// The variant slots whose custom property is registered.
  slots: FxHashSet<(Key, usize)>,
  /// Where the stylesheet goes.
  pub sheet: K,
}

impl<K: StyleSink> StyleBook<K> {
  /// Bookkeeping over `sheet`, with nothing interned or registered yet.
  pub(crate) fn new(sheet: K) -> Self {
    Self {
      interner: ClassInterner::new(),
      compiled: FxHashMap::default(),
      tokens: FxHashSet::default(),
      slots: FxHashSet::default(),
      sheet,
    }
  }

  /// What `template` compiles `style` to, compiled on a miss.
  pub(crate) fn compiled(&mut self, style: &KeptStyle, template: &Template<'_>) -> Rc<Compiled> {
    let key = (style.address(), template.key());

    if let Some(cached) = self.compiled.get(&key) {
      debug_assert!(
        cached.template == template.compile(style).template,
        "the key {key:x?} names two templates"
      );

      return Rc::clone(cached);
    }

    let compiled = Rc::new(template.compile(style));

    self.compiled.insert(key, Rc::clone(&compiled));

    compiled
  }

  /// Interns `template` and swaps it in for `previous`, returning the class.
  pub(crate) fn swap_class(&mut self, template: &str, previous: Option<&str>) -> Rc<str> {
    let (class, newly_interned) = self.interner.class_for(template);

    if newly_interned {
      self.sheet.add_rule(&class, template);
    }

    if previous != Some(&*class) {
      self.interner.retain(&class);

      if let Some(previous) = previous {
        self.release(previous);
      }
    }

    class
  }

  /// Records that one node stopped using `class`, removing its rule once unused.
  pub(crate) fn release(&mut self, class: &str) {
    if self.interner.release(class) {
      self.sheet.remove_rule(class);
    }
  }

  /// Registers the custom property of every token in `definitions` not registered yet.
  pub(crate) fn register_tokens(&mut self, definitions: &Definitions<WebTokenValue>) {
    let registered = &mut self.tokens;

    self.sheet.add_registrations(
      definitions
        .iter()
        .filter(|(name, _)| registered.insert(*name))
        .map(|(name, value)| value.registration(name)),
    );
  }

  /// Registers the custom property of every variant slot among `slots` not registered
  /// yet; the base slots are registered with the reset.
  pub(crate) fn register_slots(&mut self, slots: &[SlotBinding]) {
    let registered = &mut self.slots;

    self.sheet.add_registrations(
      slots
        .iter()
        .filter(|binding| {
          binding.position > 0 && registered.insert((binding.property, binding.position))
        })
        .map(|binding| slot_registration(binding.property, binding.position)),
    );
  }
}

/// Appends CSS text to a `<style>` element's content, escaping every `</` so the text
/// cannot end the element.
pub(crate) fn push_style_text(out: &mut String, text: &str) {
  let mut rest = text;

  while let Some(at) = rest.find("</") {
    out.push_str(&rest[..at]);
    out.push_str("<\\/");
    rest = &rest[at + 2..];
  }

  out.push_str(rest);
}
