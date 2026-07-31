//! The browser's stylesheet sink: one `<style>` element per interned rule template.
//!
//! Interning, naming and refcounting live in the shared [`ClassInterner`]; this wrapper
//! mirrors its outcomes into the document — appending a `<style>` holding the realized
//! template when a class is newly interned, removing it when the interner reports the
//! class collected. A template may hold several rules (variants, `@media` blocks,
//! `@keyframes`), so an element per class keeps insertion and removal one operation with
//! no index bookkeeping.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use web_sys::{Document, Element, HtmlHeadElement};

use crate::{interner::ClassInterner, mapping::CLASS_PLACEHOLDER};

pub(super) struct StyleStore {
  /// The owning document, used to create the per-class elements.
  document: Document,
  /// The `<head>` the elements are appended to.
  head: HtmlHeadElement,
  /// The shared naming and refcounting.
  interner: ClassInterner,
  /// Each live class's `<style>` element, for removal.
  elements: FxHashMap<Rc<str>, Element>,
}

impl StyleStore {
  pub(super) fn new(document: &Document, head: &HtmlHeadElement) -> StyleStore {
    StyleStore {
      document: document.clone(),
      head: head.clone(),
      interner: ClassInterner::new(),
      elements: FxHashMap::default(),
    }
  }

  /// Returns the class for `template`, adding its realized rules to the document the
  /// first time the template is seen.
  pub(super) fn class_for(&mut self, template: &str) -> Rc<str> {
    let (class, newly_interned) = self.interner.class_for(template);

    if newly_interned {
      let element = self
        .document
        .create_element("style")
        .expect("create <style> element");

      element.set_text_content(Some(&template.replace(CLASS_PLACEHOLDER, &class)));

      self
        .head
        .append_child(&element)
        .expect("append <style> to <head>");

      self.elements.insert(Rc::clone(&class), element);
    }

    class
  }

  /// Records that one more node is using `class`.
  pub(super) fn retain(&mut self, class: &str) {
    self.interner.retain(class);
  }

  /// Records that one node stopped using `class`; removes its `<style>` element if the
  /// interner reports it collected.
  pub(super) fn release(&mut self, class: &str) {
    if self.interner.release(class)
      && let Some(element) = self.elements.remove(class)
    {
      element.remove();
    }
  }
}
