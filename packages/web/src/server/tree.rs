//! The server's sink: a string tree of elements the realization writes, its stylesheet,
//! and the walks serializing the scene through it, whole or streamed.

use std::{cell::Cell, rc::Rc};

use rustc_hash::FxHashSet;
use skene_common::gui::{Head, HeadElement};
use skene_core::{NodeId, PickerOption, Scene};

use super::{PREEMPT_SCHEME, PREEMPT_SCHEME_SCRIPT, ROOT, SCRIPT_NONCE, VIEW_TRANSITIONS};
use crate::{
  compile::slot_registrations,
  condition::CLASS_PLACEHOLDER,
  document::{escape_attr, escape_text, push_script},
  markup::{FormElement, MARKER_ATTRIBUTE, STYLE_ATTRIBUTE, STYLE_ID},
  motion::VIEW_TRANSITION_RULE,
  platform::Web,
  realize::{Realizer, Sink},
  style_book::{RESET_CSS, STYLE_LAYER, StyleSink, push_style_text},
};

/// A position in the in-order stream walk: a node whose opening tag is emitted, the
/// scene child it last passed, by which it finds its place again among children that
/// changed while it paused, and how many it passed, where it resumes once that child is
/// gone.
#[derive(Clone, Copy)]
pub(super) struct Frame {
  pub(super) node: NodeId,
  pub(super) passed: Option<NodeId>,
  pub(super) passed_count: usize,
}

/// What an element of the string tree is.
enum Content {
  Element(&'static str),
  Comment(String),
}

/// One element or comment of the string tree.
struct Element {
  /// What it is.
  content: Content,
  /// The element's hydration marker: its place in element creation order.
  marker: u32,
  /// The element's attributes, in the order first set: `class` holding nothing, its
  /// place for the classes, and `style` the text it was set to, before the
  /// declarations.
  attributes: Vec<(String, String)>,
  /// The classes, in the order first added.
  classes: Vec<Rc<str>>,
  /// The inline style declarations, in the order first set.
  inline: Vec<(String, String)>,
  /// The text content: a run's text, or a `<textarea>`'s value.
  text: Option<String>,
  /// The elements the realization placed inside it apart from the scene's children,
  /// serialized first inside it: a control's native input inside its label.
  nested: Vec<u32>,
  /// A `<select>`'s options.
  options: Vec<PickerOption>,
  /// A `<select>`'s chosen value.
  chosen: Option<String>,
}

impl Element {
  const fn new(content: Content, marker: u32) -> Self {
    Self {
      content,
      marker,
      attributes: Vec::new(),
      classes: Vec::new(),
      inline: Vec::new(),
      text: None,
      nested: Vec::new(),
      options: Vec::new(),
      chosen: None,
    }
  }

  const fn tag(&self) -> &'static str {
    match self.content {
      Content::Element(tag) => tag,
      Content::Comment(_) => "",
    }
  }

  /// Sets the attribute `name` to `value`: `class` replaces the classes and `style`
  /// the declarations, as a browser parses them.
  fn set_attribute(&mut self, name: &str, value: &str) {
    let value = match name {
      "class" => {
        self.classes = value.split_whitespace().map(Rc::from).collect();

        ""
      }
      "style" => {
        self.inline.clear();

        value
      }
      _ => value,
    };

    match self
      .attributes
      .iter_mut()
      .find(|(existing, _)| existing == name)
    {
      Some((_, existing)) => value.clone_into(existing),
      None => self.attributes.push((name.to_owned(), value.to_owned())),
    }
  }

  /// Removes every attribute whose name `matches`, the classes or the declarations
  /// with `class` or `style`.
  fn remove_attributes(&mut self, matches: impl Fn(&str) -> bool) {
    if matches("class") {
      self.classes.clear();
    }

    if matches("style") {
      self.inline.clear();
    }

    self.attributes.retain(|(name, _)| !matches(name));
  }

  fn set_inline(&mut self, property: &str, value: &str) {
    match self
      .inline
      .iter_mut()
      .find(|(existing, _)| existing == property)
    {
      Some((_, existing)) => value.clone_into(existing),
      None => self.inline.push((property.to_owned(), value.to_owned())),
    }
  }

  fn is_void(&self) -> bool {
    matches!(self.tag(), "input" | "br" | "hr" | "img")
  }

  /// Appends the element's text, escaped; a textarea's leading newline doubled, since
  /// the parser drops the first.
  fn emit_text(&self, text: &str, out: &mut String) {
    if self.tag() == "textarea" && text.starts_with('\n') {
      out.push('\n');
    }

    escape_text(text, out);
  }

  fn emit_comment(text: &str, out: &mut String) {
    out.push_str("<!--");
    out.push_str(text);
    out.push_str("-->");
  }

  fn emit_open(&self, out: &mut String) {
    out.push('<');
    out.push_str(self.tag());
    out.push(' ');
    out.push_str(MARKER_ATTRIBUTE);
    out.push_str("=\"");
    out.push_str(&self.marker.to_string());
    out.push('"');

    let mut classes = false;
    let mut style = false;

    for (name, value) in &self.attributes {
      match name.as_str() {
        "class" => {
          classes = true;
          self.emit_classes(out);
        }
        "style" => {
          style = true;
          self.emit_style(value, out);
        }
        _ => {
          out.push(' ');
          out.push_str(name);

          if !value.is_empty() {
            out.push_str("=\"");
            escape_attr(value, out);
            out.push('"');
          }
        }
      }
    }

    if !classes {
      self.emit_classes(out);
    }

    if !style {
      self.emit_style("", out);
    }

    out.push('>');
  }

  /// Writes the `class` attribute, where the element has a class.
  fn emit_classes(&self, out: &mut String) {
    if self.classes.is_empty() {
      return;
    }

    out.push_str(" class=\"");

    for (index, class) in self.classes.iter().enumerate() {
      if index > 0 {
        out.push(' ');
      }

      out.push_str(class);
    }

    out.push('"');
  }

  /// Writes the `style` attribute: the text it was set to, then the declarations;
  /// nothing where both are empty.
  fn emit_style(&self, text: &str, out: &mut String) {
    let mut style = text.to_owned();

    if !style.is_empty() && !self.inline.is_empty() && !style.trim_end().ends_with(';') {
      style.push(';');
    }

    for (property, value) in &self.inline {
      style.push_str(property);
      style.push(':');
      style.push_str(value);
      style.push(';');
    }

    if style.is_empty() {
      return;
    }

    out.push_str(" style=\"");
    escape_attr(&style, out);
    out.push('"');
  }

  /// Writes a `<select>`'s options, the chosen one and the disabled ones marked.
  fn emit_options(&self, out: &mut String) {
    for option in &self.options {
      out.push_str("<option value=\"");
      escape_attr(&option.value, out);
      out.push('"');

      if self.chosen.as_deref() == Some(option.value.as_str()) {
        out.push_str(" selected");
      }

      if option.disabled {
        out.push_str(" disabled");
      }

      out.push('>');
      escape_text(&option.name, out);
      out.push_str("</option>");
    }
  }

  fn emit_close(&self, out: &mut String) {
    if self.is_void() {
      return;
    }

    out.push_str("</");
    out.push_str(self.tag());
    out.push('>');
  }
}

/// The server's sink: the string tree's elements by index, the root's first.
pub(super) struct StringTree {
  elements: Vec<Option<Element>>,
  markers: u32,
}

impl StringTree {
  fn new() -> Self {
    Self {
      elements: vec![Some(Element::new(Content::Element(""), 0))],
      markers: 0,
    }
  }

  fn element(&self, index: u32) -> Option<&Element> {
    self.elements.get(index as usize).and_then(Option::as_ref)
  }

  fn element_mut(&mut self, index: u32) -> Option<&mut Element> {
    self
      .elements
      .get_mut(index as usize)
      .and_then(Option::as_mut)
  }

  fn push(&mut self, element: Element) -> u32 {
    let index = u32::try_from(self.elements.len()).expect("a tree holds at most 2^32 elements");

    self.elements.push(Some(element));

    index
  }

  /// Runs `write` on the element `index`, if it is live.
  fn write(&mut self, index: u32, write: impl FnOnce(&mut Element)) {
    if let Some(element) = self.element_mut(index) {
      write(element);
    }
  }
}

impl Sink for StringTree {
  type Element = u32;
  type Extra = ();
  type Sheet = Sheet;

  const TITLE_TOOLTIPS: bool = true;

  fn create_element(&mut self, tag: &'static str) -> (u32, bool) {
    self.markers += 1;

    (
      self.push(Element::new(Content::Element(tag), self.markers)),
      false,
    )
  }

  fn create_comment(&mut self, text: String) -> (u32, bool) {
    (self.push(Element::new(Content::Comment(text), 0)), false)
  }

  fn bind(&mut self, _: &u32, _: NodeId) {}

  fn discard(&mut self, element: &u32) {
    if let Some(slot) = self.elements.get_mut(*element as usize) {
      *slot = None;
    }
  }

  fn append_child(&mut self, parent: &u32, child: &u32) {
    self.write(*parent, |parent| {
      if !parent.nested.contains(child) {
        parent.nested.push(*child);
      }
    });
  }

  fn set_attribute(&mut self, element: &u32, name: &str, value: &str) {
    self.write(*element, |element| element.set_attribute(name, value));
  }

  fn remove_attribute(&mut self, element: &u32, name: &str) {
    self.write(*element, |element| {
      element.remove_attributes(|existing| existing == name);
    });
  }

  fn remove_attributes(&mut self, element: &u32, matches: impl Fn(&str) -> bool) {
    self.write(*element, |element| element.remove_attributes(matches));
  }

  fn set_property(&mut self, element: &u32, property: &str, value: &str) {
    self.write(*element, |element| element.set_inline(property, value));
  }

  fn remove_property(&mut self, element: &u32, property: &str) {
    self.write(*element, |element| {
      element.inline.retain(|(existing, _)| existing != property);
    });
  }

  fn custom_properties(&self, element: &u32) -> Vec<String> {
    self.element(*element).map_or_else(Vec::new, |element| {
      element
        .inline
        .iter()
        .filter(|(property, _)| property.starts_with("--"))
        .map(|(property, _)| property.clone())
        .collect()
    })
  }

  fn text(&self, element: &u32) -> Option<String> {
    self.element(*element)?.text.clone()
  }

  fn set_text(&mut self, element: &u32, text: &str) {
    self.write(*element, |element| element.text = Some(text.to_owned()));
  }

  fn swap_class(&mut self, element: &u32, previous: Option<&str>, class: &str) {
    self.write(*element, |element| {
      if let Some(previous) = previous
        && previous != class
      {
        element.classes.retain(|existing| **existing != *previous);
      }

      if !element.classes.iter().any(|existing| **existing == *class) {
        element.classes.push(Rc::from(class));
      }
    });
  }

  fn set_checked(&mut self, element: &u32, checked: bool) {
    self.set_flag(element, "checked", checked);
  }

  fn set_indeterminate(&mut self, _: &u32, _: bool) {}

  fn set_value(&mut self, element: &u32, value: &str) {
    self.write(*element, |element| match element.tag() {
      "textarea" => element.text = Some(value.to_owned()),
      "select" => element.chosen = Some(value.to_owned()),
      _ => element.set_attribute("value", value),
    });
  }

  fn set_options(&mut self, element: &u32, options: &[PickerOption]) {
    self.write(*element, |element| element.options = options.to_vec());
  }

  fn note_live_state(&mut self, _: NodeId, _: &u32, _: FormElement) {}
}

/// The server's stylesheet: what each flush emits, and the classes whose rules are
/// emitted.
#[derive(Default)]
pub(super) struct Sheet {
  /// The token and slot registrations not emitted yet.
  registrations: String,
  /// The rules interned since the last flush and still live, realized, in interning
  /// order.
  rules: Vec<(Rc<str>, String)>,
  /// The classes whose rules are emitted.
  emitted: FxHashSet<Rc<str>>,
}

impl Sheet {
  /// Appends the registrations and rules not emitted yet, the rules in the style
  /// layer, marking them emitted; no `</` is written, so the text cannot end the
  /// `<style>` it sits in.
  fn render_new(&mut self, out: &mut String) {
    push_style_text(out, &self.registrations);
    self.registrations.clear();

    if self.rules.is_empty() {
      return;
    }

    out.push_str(STYLE_LAYER);

    for (class, rule) in self.rules.drain(..) {
      push_style_text(out, &rule);
      self.emitted.insert(class);
    }

    out.push('}');
  }
}

impl StyleSink for Sheet {
  fn add_rule(&mut self, class: &Rc<str>, template: &str) {
    if !self.emitted.contains(class) {
      self
        .rules
        .push((Rc::clone(class), template.replace(CLASS_PLACEHOLDER, class)));
    }
  }

  fn remove_rule(&mut self, class: &str) {
    self.rules.retain(|(pending, _)| **pending != *class);
  }

  fn add_registrations(&mut self, registrations: impl Iterator<Item = String>) {
    self.registrations.extend(registrations);
  }
}

/// The string tree, its stylesheet and the head, for one render.
pub(super) struct Tree {
  pub(super) realizer: Realizer<StringTree>,
  pub(super) language: Option<String>,
  pub(super) head: Head,
}

impl Tree {
  pub(super) fn new() -> Self {
    Self {
      realizer: Realizer::new(StringTree::new(), Sheet::default()),
      language: None,
      head: Head::default(),
    }
  }

  /// The element realizing `node`, if it is live.
  fn element_of(&self, node: NodeId) -> Option<&Element> {
    let record = self.realizer.records.get(node)?;

    self.realizer.sink.element(record.element)
  }

  /// The scene's children of `node` the tree realizes, in order: the caption a control
  /// keeps beside it passed over, which the control serializes.
  fn scene_children<'a>(
    &'a self,
    scene: &'a Scene<Web>,
    node: NodeId,
  ) -> impl Iterator<Item = NodeId> + 'a {
    let caption = scene.caption_beside(node);

    scene
      .children(node)
      .filter(move |child| Some(*child) != caption && self.realizer.records.contains(*child))
  }

  /// The scene child after `passed` among those of `node` the tree realizes, the
  /// caption a control keeps beside it passed over.
  fn next_scene_child(&self, scene: &Scene<Web>, node: NodeId, passed: NodeId) -> Option<NodeId> {
    let caption = scene.caption_beside(node);
    let mut next = scene.next_sibling(passed);

    while let Some(child) = next {
      if Some(child) != caption && self.realizer.records.contains(child) {
        return Some(child);
      }

      next = scene.next_sibling(child);
    }

    None
  }

  /// The `<style>` block a document opens with: the reset, the slot registrations, and
  /// every token registration and live rule not emitted yet, in interning order.
  pub(super) fn render_styles(&mut self) -> String {
    let mut out = format!("<style id=\"{STYLE_ID}\">");

    out.push_str(RESET_CSS);
    out.push_str(&slot_registrations());

    if VIEW_TRANSITIONS.with(Cell::get) {
      out.push_str(VIEW_TRANSITION_RULE);
    }

    self.realizer.book.sheet.render_new(&mut out);
    out.push_str("</style>");

    out
  }

  /// The `<style>` block a later chunk opens with: the token registrations and live
  /// rules not emitted yet, in interning order; empty when there are none.
  pub(super) fn render_style_delta(&mut self) -> String {
    let mut delta = String::new();

    self.realizer.book.sheet.render_new(&mut delta);

    if delta.is_empty() {
      return delta;
    }

    format!("<style {STYLE_ATTRIBUTE}>{delta}</style>")
  }

  pub(super) fn render_head(&self) -> String {
    let mut out = String::new();

    if PREEMPT_SCHEME.with(Cell::get) {
      SCRIPT_NONCE.with(|nonce| {
        push_script(
          &mut out,
          nonce.borrow().as_deref(),
          false,
          PREEMPT_SCHEME_SCRIPT,
        );
      });
    }

    if let Some(title) = self.head.title() {
      out.push_str("<title>");
      escape_text(title, &mut out);
      out.push_str("</title>");
    }

    for HeadElement { tag, attributes } in self.head.elements() {
      out.push('<');
      out.push_str(tag);

      for (name, value) in attributes {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_attr(value, &mut out);
        out.push('"');
      }

      out.push('>');

      if tag == "script" {
        out.push_str("</script>");
      }
    }

    out
  }

  /// The page body: the scene's children of `root`, serialized.
  pub(super) fn render_body(&self, root: NodeId) -> String {
    let mut out = String::new();

    Scene::<Web>::read(|scene| {
      for child in self.scene_children(scene, root) {
        self.serialize(scene, child, &mut out);
      }
    });

    out
  }

  fn serialize(&self, scene: &Scene<Web>, node: NodeId, out: &mut String) {
    let Some(element) = self.element_of(node)
    else {
      return;
    };

    if let Content::Comment(text) = &element.content {
      Element::emit_comment(text, out);

      return;
    }

    self.serialize_caption(scene, node, out);
    element.emit_open(out);
    element.emit_options(out);

    if let Some(text) = &element.text {
      element.emit_text(text, out);
    }
    else {
      self.serialize_nested(element, out);

      for child in self.scene_children(scene, node) {
        self.serialize(scene, child, out);
      }
    }

    element.emit_close(out);
  }

  /// Serializes the caption `node` keeps beside it in its `<label>`, where it keeps one.
  fn serialize_caption(&self, scene: &Scene<Web>, node: NodeId, out: &mut String) {
    let Some(label) = self
      .realizer
      .records
      .get(node)
      .and_then(|record| record.caption)
      .and_then(|label| self.realizer.sink.element(label))
    else {
      return;
    };
    let Some(child) = scene
      .caption_beside(node)
      .filter(|child| self.realizer.records.contains(*child))
    else {
      return;
    };

    label.emit_open(out);
    self.serialize(scene, child, out);
    label.emit_close(out);
  }

  /// Serializes the elements the realization placed inside `element`.
  fn serialize_nested(&self, element: &Element, out: &mut String) {
    for nested in element
      .nested
      .iter()
      .filter_map(|nested| self.realizer.sink.element(*nested))
    {
      nested.emit_open(out);
      nested.emit_options(out);

      if let Some(text) = &nested.text {
        nested.emit_text(text, out);
      }
      else {
        self.serialize_nested(nested, out);
      }

      nested.emit_close(out);
    }
  }

  /// Serializes the tree in document order from `cursor`, pausing before a child
  /// `pending` reports as an unsettled boundary.
  ///
  /// Appends to `out` until it reaches such a child, returning that node with the
  /// cursor poised to emit it next; ancestors stay open across the pause, exactly as a
  /// streamed document expects. `None` means the whole tree is serialized.
  pub(super) fn stream(
    &self,
    scene: &Scene<Web>,
    cursor: &mut Vec<Frame>,
    out: &mut String,
    pending: impl Fn(NodeId) -> bool,
  ) -> Option<NodeId> {
    while let Some(frame) = cursor.last_mut() {
      let Some(record) = self.realizer.records.get(frame.node)
      else {
        cursor.pop();

        continue;
      };
      let next = match frame.passed {
        None => self.scene_children(scene, frame.node).next(),
        Some(passed) if scene.parent(passed) == Some(frame.node) => {
          self.next_scene_child(scene, frame.node, passed)
        }
        Some(_) => self
          .scene_children(scene, frame.node)
          .nth(frame.passed_count),
      };
      let Some(child) = next
      else {
        if record.element != ROOT
          && let Some(element) = self.realizer.sink.element(record.element)
        {
          element.emit_close(out);
        }

        cursor.pop();

        continue;
      };

      if pending(child) {
        return Some(child);
      }

      frame.passed = Some(child);
      frame.passed_count += 1;

      let Some(element) = self.element_of(child)
      else {
        continue;
      };

      if let Content::Comment(text) = &element.content {
        Element::emit_comment(text, out);

        continue;
      }

      self.serialize_caption(scene, child, out);
      element.emit_open(out);
      element.emit_options(out);

      if let Some(text) = &element.text {
        element.emit_text(text, out);
        element.emit_close(out);

        continue;
      }

      self.serialize_nested(element, out);

      if self.scene_children(scene, child).next().is_none() {
        element.emit_close(out);
      }
      else {
        cursor.push(Frame {
          node: child,
          passed: None,
          passed_count: 0,
        });
      }
    }

    None
  }
}
