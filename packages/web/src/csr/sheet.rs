//! The browser's stylesheet sink over the shared interner: one base `<style>` holding
//! the reset and the registrations, one `<style>` per interned class, and the server's
//! blocks adopted in place of both when a page hydrates.

use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{Definitions, TokenName};
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlHeadElement};

use crate::{
  RESET_CSS,
  compile::{Compiled, slot_registrations},
  condition::CLASS_PLACEHOLDER,
  interner::ClassInterner,
  markup::STYLE_ATTRIBUTE,
  token::WebTokenValue,
};

pub(super) struct Sheet {
  document: Document,
  head: HtmlHeadElement,
  interner: ClassInterner,
  cache: FxHashMap<u64, Rc<Compiled>>,
  elements: FxHashMap<Rc<str>, Element>,
  registered: FxHashSet<TokenName>,
  base: Element,
  base_text: String,
  adopted: Option<String>,
  deltas: Vec<Element>,
}

impl Sheet {
  /// A sink writing into `head`, starting from the reset and the slot registrations.
  pub(super) fn new(document: &Document, head: &HtmlHeadElement) -> Self {
    let base_text = format!("{RESET_CSS}{}", slot_registrations());
    let base = style_element(document);

    base.set_text_content(Some(&base_text));
    let _ = head.append_child(&base);

    Self {
      document: document.clone(),
      head: head.clone(),
      interner: ClassInterner::new(),
      cache: FxHashMap::default(),
      elements: FxHashMap::default(),
      registered: FxHashSet::default(),
      base,
      base_text,
      adopted: None,
      deltas: Vec::new(),
    }
  }

  /// A sink adopting the server's `block` and the delta blocks a streamed render left
  /// in the body, which move into the head after it: every rule already in them is left
  /// alone, and only what they lack is added.
  pub(super) fn adopt(document: &Document, head: &HtmlHeadElement, block: Element) -> Self {
    let base_text = block.text_content().unwrap_or_default();
    let mut adopted = base_text.clone();
    let mut deltas = Vec::new();

    let _ = block.set_attribute(STYLE_ATTRIBUTE, "");

    if let Ok(blocks) = document.query_selector_all(&format!("style[{STYLE_ATTRIBUTE}]")) {
      for index in 0..blocks.length() {
        if let Some(delta) = blocks
          .item(index)
          .and_then(|node| node.dyn_into::<Element>().ok())
          && delta != block
        {
          adopted.push_str(&delta.text_content().unwrap_or_default());
          let _ = head.append_child(&delta);
          deltas.push(delta);
        }
      }
    }

    Self {
      document: document.clone(),
      head: head.clone(),
      interner: ClassInterner::new(),
      cache: FxHashMap::default(),
      elements: FxHashMap::default(),
      registered: FxHashSet::default(),
      base: block,
      base_text,
      adopted: Some(adopted),
      deltas,
    }
  }

  /// The compiled template for `key`, rendered by `compile` on a miss.
  pub(super) fn compiled(&mut self, key: u64, compile: impl FnOnce() -> Compiled) -> Rc<Compiled> {
    if let Some(cached) = self.cache.get(&key) {
      debug_assert!(
        cached.template == compile().template,
        "the fingerprint {key:x} names two templates"
      );

      return Rc::clone(cached);
    }

    let compiled = Rc::new(compile());

    self.cache.insert(key, Rc::clone(&compiled));

    compiled
  }

  /// Interns `template` and swaps it in for `previous`, returning the class.
  pub(super) fn swap_class(&mut self, template: &str, previous: Option<&str>) -> Rc<str> {
    let (class, newly_interned) = self.interner.class_for(template);

    if newly_interned {
      let realized = template.replace(CLASS_PLACEHOLDER, &class);
      let present = self
        .adopted
        .as_ref()
        .is_some_and(|adopted| adopted.contains(&realized));

      if !present {
        let element = style_element(&self.document);

        element.set_text_content(Some(&realized));
        let _ = self.head.append_child(&element);
        self.elements.insert(Rc::clone(&class), element);
      }
    }

    if previous != Some(&*class) {
      self.interner.retain(&class);

      if let Some(previous) = previous {
        self.release(previous);
      }
    }

    class
  }

  /// Records that one node stopped using `class`, removing its rules once unused.
  pub(super) fn release(&mut self, class: &str) {
    if self.interner.release(class)
      && let Some(element) = self.elements.remove(class)
    {
      element.remove();
    }
  }

  /// Registers the custom property of every token in `definitions` not registered yet.
  pub(super) fn register(&mut self, definitions: &Definitions<WebTokenValue>) {
    let mut added = false;

    for (name, value) in definitions.iter() {
      if !self.registered.insert(name) {
        continue;
      }

      let registration = value.registration(name);
      let present = self
        .adopted
        .as_ref()
        .is_some_and(|adopted| adopted.contains(&registration));

      if !present {
        self.base_text.push_str(&registration);
        added = true;
      }
    }

    if added {
      self.base.set_text_content(Some(&self.base_text));
    }
  }

  /// Appends `rule` to the base block, unless the adopted blocks already carry it.
  pub(super) fn add_rule(&mut self, rule: &str) {
    if self.base_text.contains(rule)
      || self
        .adopted
        .as_ref()
        .is_some_and(|adopted| adopted.contains(rule))
    {
      return;
    }

    self.base_text.push_str(rule);
    self.base.set_text_content(Some(&self.base_text));
  }

  /// Removes every `<style>` element the sink owns.
  pub(super) fn remove_all(&mut self) {
    self.base.remove();

    for element in self.deltas.drain(..) {
      element.remove();
    }

    for (_, element) in self.elements.drain() {
      element.remove();
    }
  }
}

fn style_element(document: &Document) -> Element {
  let element = document.create_element("style").expect("a style element");

  let _ = element.set_attribute(STYLE_ATTRIBUTE, "");

  element
}
