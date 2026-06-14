//! The shared stylesheet store.
//!
//! Rather than inline styles, the DOM backend keeps a single `<style>` element
//! in `<head>` and a map from a content hash to a generated class name. Setting a
//! node's style means: render its declarations, find (or insert) the matching
//! class, and assign that class to the element.
//!
//! Because class names are derived deterministically from the declaration text,
//! the same `Style` always produces the same class. That is what will let this
//! store be reused for server-side rendering later: the server can emit the same
//! `<style>` contents and class attributes that the client would generate.

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use web_sys::{Document, Element};

pub struct StyleStore {
  style_el: Element,
  /// Accumulated stylesheet text (`.class{decls}` rules).
  buffer: String,
  /// content-hash -> generated class name (doubles as the "already inserted" set).
  classes: FxHashMap<u64, String>,
}

impl StyleStore {
  pub fn new(document: &Document) -> StyleStore {
    let style_el = document
      .create_element("style")
      .expect("create <style> element");
    let _ = style_el.set_attribute("data-skene", "");
    let head = document.head().expect("document has a <head>");
    head
      .append_child(style_el.as_ref())
      .expect("append <style> to <head>");
    StyleStore {
      style_el,
      buffer: String::new(),
      classes: FxHashMap::default(),
    }
  }

  /// Return the class name for a CSS declaration block, inserting a new rule
  /// into the stylesheet the first time a given block is seen.
  pub fn class_for(&mut self, declarations: &str) -> String {
    let mut hasher = FxHasher::default();
    declarations.hash(&mut hasher);
    let hash = hasher.finish();

    if let Some(class) = self.classes.get(&hash) {
      return class.clone();
    }

    let class = format!("f{hash:x}");
    self.buffer.push('.');
    self.buffer.push_str(&class);
    self.buffer.push('{');
    self.buffer.push_str(declarations);
    self.buffer.push('}');
    self.buffer.push('\n');
    self.style_el.set_text_content(Some(&self.buffer));

    self.classes.insert(hash, class.clone());
    class
  }
}
