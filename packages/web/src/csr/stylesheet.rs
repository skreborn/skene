//! The browser's stylesheet sink: a live `CssStyleSheet` over the shared class interner.
//!
//! Rather than inline styles, the browser keeps one `<style>` element in `<head>` and a class
//! per interned [`Declarations`] set. Interning, naming and refcounting live in the shared
//! [`ClassInterner`]; this wrapper mirrors its outcomes into the live sheet — inserting a rule
//! when a class is newly interned, deleting it when the interner reports the class collected.
//! Rules go in and out of the CSSOM one at a time, so interning or collecting costs one call
//! rather than a re-serialization of the whole sheet.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use wasm_bindgen::JsCast;
use web_sys::{CssStyleSheet, Document, HtmlHeadElement, HtmlStyleElement};

use crate::{Declarations, interner::ClassInterner};

pub(super) struct StyleStore {
  /// The live stylesheet backing the store.
  sheet: CssStyleSheet,
  /// The shared naming and refcounting.
  interner: ClassInterner,
  /// Each live rule's index in the sheet's `cssRules`, for deletion.
  indices: FxHashMap<Rc<str>, u32>,
}

impl StyleStore {
  pub(super) fn new(document: &Document, head: &HtmlHeadElement) -> StyleStore {
    let style_el = document
      .create_element("style")
      .expect("create <style> element");

    head
      .append_child(&style_el)
      .expect("append <style> to <head>");

    let sheet = style_el
      .dyn_into::<HtmlStyleElement>()
      .expect("<style> is an HtmlStyleElement")
      .sheet()
      .expect("connected <style> has a sheet")
      .dyn_into::<CssStyleSheet>()
      .expect("sheet is a CssStyleSheet");

    StyleStore {
      sheet,
      interner: ClassInterner::new(),
      indices: FxHashMap::default(),
    }
  }

  /// Returns the class for `declarations`, inserting its rule into the live sheet the first time
  /// the set is seen. The declarations are rendered to CSS text only on that first insertion.
  pub(super) fn class_for(&mut self, declarations: &Declarations) -> Rc<str> {
    let (class, newly_interned) = self.interner.class_for(declarations);

    if newly_interned {
      let text = format!(".{class}{{{}}}", declarations.render());
      // Append at the end: the new rule's index is the current rule count.
      let index = u32::try_from(self.indices.len()).expect("rule count fits in u32");

      self
        .sheet
        .insert_rule_with_index(&text, index)
        .expect("insert style rule");

      self.indices.insert(Rc::clone(&class), index);
    }

    class
  }

  /// Records that one more node is using `class`.
  pub(super) fn retain(&mut self, class: &str) {
    self.interner.retain(class);
  }

  /// Records that one node stopped using `class`; deletes its rule from the sheet if the
  /// interner reports it collected.
  pub(super) fn release(&mut self, class: &str) {
    if self.interner.release(class) {
      let index = self
        .indices
        .remove(class)
        .expect("collected rule has an index");

      self.sheet.delete_rule(index).expect("delete style rule");

      // Deleting shifts every later rule down one; keep the stored indices in step.
      for stored in self.indices.values_mut() {
        if *stored > index {
          *stored -= 1;
        }
      }
    }
  }
}
