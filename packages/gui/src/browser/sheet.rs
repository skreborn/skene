//! The browser's stylesheet: one base `<style>` holding the reset and the
//! registrations, one `<style>` per interned class, and the server's blocks adopted in
//! place of both when a page hydrates.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlHeadElement};

use crate::{
  condition::CLASS_PLACEHOLDER,
  markup::STYLE_ATTRIBUTE,
  style_book::{STYLE_LAYER, StyleSink, base_stylesheet, push_style_text},
};

pub(super) struct Sheet {
  document: Document,
  head: HtmlHeadElement,
  elements: FxHashMap<Rc<str>, Element>,
  base: Element,
  base_text: String,
  adopted: Option<String>,
  deltas: Vec<Element>,
}

impl Sheet {
  /// A sheet writing into `head`, starting from the base stylesheet.
  pub(super) fn new(document: &Document, head: &HtmlHeadElement) -> Self {
    let base_text = base_stylesheet();
    let base = style_element(document);

    base.set_text_content(Some(&base_text));
    let _ = head.append_child(&base);

    Self {
      document: document.clone(),
      head: head.clone(),
      elements: FxHashMap::default(),
      base,
      base_text,
      adopted: None,
      deltas: Vec::new(),
    }
  }

  /// A sheet adopting the server's `block` and the delta blocks a streamed render left
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
      elements: FxHashMap::default(),
      base: block,
      base_text,
      adopted: Some(adopted),
      deltas,
    }
  }

  /// Whether the adopted blocks carry `text`, as the server wrote it.
  fn adopted_holds(&self, text: &str) -> bool {
    let Some(adopted) = &self.adopted
    else {
      return false;
    };

    if !text.contains("</") {
      return adopted.contains(text);
    }

    let mut written = String::new();

    push_style_text(&mut written, text);

    adopted.contains(&written)
  }

  /// Appends `rule` to the base block, unless it or the adopted blocks carry it
  /// already.
  pub(super) fn add_base_rule(&mut self, rule: &str) {
    if self.base_text.contains(rule) || self.adopted_holds(rule) {
      return;
    }

    self.base_text.push_str(rule);
    self.base.set_text_content(Some(&self.base_text));
  }

  /// Removes every `<style>` element the sheet owns.
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

impl StyleSink for Sheet {
  fn add_rule(&mut self, class: &Rc<str>, template: &str) {
    let realized = template.replace(CLASS_PLACEHOLDER, class);

    if self.adopted_holds(&realized) {
      return;
    }

    let element = style_element(&self.document);

    element.set_text_content(Some(&format!("{STYLE_LAYER}{realized}}}")));
    let _ = self.head.append_child(&element);
    self.elements.insert(Rc::clone(class), element);
  }

  fn remove_rule(&mut self, class: &str) {
    if let Some(element) = self.elements.remove(class) {
      element.remove();
    }
  }

  fn add_registrations(&mut self, registrations: impl Iterator<Item = String>) {
    let mut added = false;

    for registration in registrations {
      if !self.adopted_holds(&registration) {
        self.base_text.push_str(&registration);
        added = true;
      }
    }

    if added {
      self.base.set_text_content(Some(&self.base_text));
    }
  }
}

fn style_element(document: &Document) -> Element {
  let element = document.create_element("style").expect("a style element");

  let _ = element.set_attribute(STYLE_ATTRIBUTE, "");

  element
}
