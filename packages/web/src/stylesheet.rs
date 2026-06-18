//! The shared stylesheet store.
//!
//! Rather than inline styles, the web backend keeps a single `<style>` element in
//! `<head>` and a map from a set of [`Declarations`] to a class name. A node's
//! initial style is interned here as a class; a runtime change is applied as an
//! inline style on the element, overriding the class, so the class—and thus the
//! stylesheet—stays stable while a property animates. (Text style changes currently
//! re-intern the text class instead; folding them onto the inline path is future
//! work.)
//!
//! The store is keyed on [`Declarations`] directly, so a lookup hashes and compares
//! structurally—two nodes whose declarations are equal share one class, and the
//! declarations are rendered to CSS text only the first time a set is interned, not
//! on every lookup. Class names are derived from a content hash (so the same
//! declarations yield the same class, which is what will let this store be reused
//! for server-side rendering), with a probe that keeps two distinct sets from ever
//! sharing a name.
//!
//! Each rule is reference-counted by the number of nodes currently assigned its
//! class. When the last user goes away the rule is collected and dropped from the
//! stylesheet, so the sheet doesn't grow without bound as styles come and go. Rules
//! are inserted into and deleted from the live [`CssStyleSheet`] one at a time
//! through the CSSOM, so interning or collecting a rule costs one CSSOM call rather
//! than a re-serialization of the whole sheet.

use std::{
  hash::{Hash, Hasher},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use wasm_bindgen::JsCast;
use web_sys::{CssStyleSheet, Document, HtmlHeadElement, HtmlStyleElement};

use crate::declarations::Declarations;

/// A generated class rule, its position in the sheet, and how many live nodes use it.
struct Rule {
  /// The declarations this rule was keyed on, so its `classes` entry can be removed
  /// when the rule is collected.
  key: Declarations,
  /// The rule's index in the sheet's `cssRules`, for deletion.
  index: u32,
  /// Number of nodes currently assigned this class.
  count: u32,
}

pub struct StyleStore {
  /// The live stylesheet backing the store.
  sheet: CssStyleSheet,
  /// Declarations -> generated class name, so identical sets dedup to one class. The
  /// name is a shared [`Rc<str>`] so the store, each rule, and every node using it
  /// hold one string rather than a copy each.
  classes: FxHashMap<Declarations, Rc<str>>,
  /// Class name -> its rule and reference count.
  rules: FxHashMap<Rc<str>, Rule>,
  /// Name hashes already handed out, so the (vanishingly rare) hash collision
  /// between two distinct sets still resolves to distinct class names.
  used_names: FxHashSet<u64>,
}

impl StyleStore {
  pub fn new(document: &Document, head: &HtmlHeadElement) -> StyleStore {
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
      classes: FxHashMap::default(),
      rules: FxHashMap::default(),
      used_names: FxHashSet::default(),
    }
  }

  /// Returns the class name for `declarations`, inserting a new (initially
  /// unreferenced) rule the first time a given set is seen. The declarations are
  /// rendered to CSS text only on that first insertion.
  ///
  /// The returned class has no claim on it yet—call [`retain`](Self::retain) once it
  /// is actually assigned to a node, and [`release`](Self::release) when the node
  /// drops it, so the rule can be collected when unused.
  pub fn class_for(&mut self, declarations: &Declarations) -> Rc<str> {
    if let Some(class) = self.classes.get(declarations) {
      return Rc::clone(class);
    }

    // Content-addressed name; probe forward on the rare hash collision so two
    // distinct sets never collapse onto the same class name.
    let mut tag = {
      let mut hasher = FxHasher::default();

      declarations.hash(&mut hasher);

      hasher.finish()
    };

    while !self.used_names.insert(tag) {
      tag = tag.wrapping_add(1);
    }

    let class: Rc<str> = format!("skene-{tag:x}").into();
    let text = format!(".{class}{{{}}}", declarations.render());

    // Append at the end: the new rule's index is the current rule count.
    let index = u32::try_from(self.rules.len()).expect("rule count fits in u32");

    self
      .sheet
      .insert_rule_with_index(&text, index)
      .expect("insert style rule");

    self.rules.insert(
      Rc::clone(&class),
      Rule {
        key: declarations.clone(),
        index,
        count: 0,
      },
    );

    self.classes.insert(declarations.clone(), Rc::clone(&class));

    class
  }

  /// Records that one more node is using `class`.
  pub fn retain(&mut self, class: &str) {
    if let Some(rule) = self.rules.get_mut(class) {
      rule.count += 1;
    }
  }

  /// Records that one node stopped using `class`; collects the rule if it was the
  /// last user, dropping it from the stylesheet.
  pub fn release(&mut self, class: &str) {
    let (count, index) = match self.rules.get_mut(class) {
      Some(rule) => {
        rule.count -= 1;

        (rule.count, rule.index)
      }
      None => return,
    };

    if count == 0 {
      let rule = self.rules.remove(class).expect("rule just decremented");

      self.classes.remove(&rule.key);

      let tag = class
        .strip_prefix("skene-")
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        .unwrap_or(0);

      self.used_names.remove(&tag);

      self.sheet.delete_rule(index).expect("delete style rule");

      // Deleting shifts every later rule down one; keep the stored indices in step.
      for rule in self.rules.values_mut() {
        if rule.index > index {
          rule.index -= 1;
        }
      }
    }
  }
}
