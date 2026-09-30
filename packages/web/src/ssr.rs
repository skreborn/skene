//! The server leaf's realization of the platform: a string tree serialized to HTML with
//! the interned stylesheet, the head and the hydration state, whole or as an in-order
//! stream around deferred boundaries.
//!
//! Every element carries a `data-h` marker counting elements in creation order, which
//! the browser leaf adopts by; an anchor is a comment pair (`<!--s:N-->`, `<!--e:N-->`)
//! counting segments the same way; a scope is a `display: contents` element. Handlers are inert,
//! focus and sizes are the client's, and an overlay serializes closed. A streamed chunk opens with
//! a `<style>` delta holding the registrations and rules interned since the previous flush.

mod task;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
  time::Duration,
};

use rustc_hash::FxHashSet;
use skene_common::gui::{GuiExt, Head, HeadElement, HeadId, Painter, Painting};
use skene_core::{
  AccessProp,
  Boundary,
  BoxProps,
  BoxValue,
  ColorScheme,
  ControlProps,
  ControlState,
  Definitions,
  Environment,
  EventKind,
  FocusOrder,
  Inset,
  IntoView,
  Media,
  MediaSignals,
  NodeId,
  NodeKind,
  NodeMap,
  Observation,
  OverlayProps,
  PickerOption,
  Platform,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticBoxStyle,
  StaticLeaf,
  TextSelection,
  Timer,
  View,
};
use skene_reactive::{Memo, Owner, ReadSignal, Set, Signal, WriteSignal, batch, provide_context};

pub use self::task::{drive, install_executor, set_render_deadline, sleep};
use super::{
  Web,
  compile::slot_registrations,
  condition::CLASS_PLACEHOLDER,
  markup::{FormElement, MARKER_ATTRIBUTE, STYLE_ATTRIBUTE, STYLE_ID},
  motion::VIEW_TRANSITION_RULE,
  token::WebTokenValue,
};
use crate::{
  Declarations,
  Document,
  RESET_CSS,
  RenderHints,
  STYLE_LAYER,
  WebMarkup,
  document::{escape_attr, escape_text, push_script, state_script},
  realize::{Realized, Realizer, Sink},
  style_book::{StyleSink, push_style_text},
};

/// The element of the string tree standing for the page body, which the root realizes.
const ROOT: u32 = 0;

/// A probe for whether a deferred boundary's content has settled.
type BoundarySettled = Box<dyn Fn() -> bool>;

/// A position in the in-order stream walk: a node whose opening tag is emitted, the
/// scene child it last passed, by which it finds its place again among children that
/// changed while it paused, and how many it passed, where it resumes once that child is
/// gone.
#[derive(Clone, Copy)]
struct Frame {
  node: NodeId,
  passed: Option<NodeId>,
  passed_count: usize,
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
struct StringTree {
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
struct Sheet {
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
struct Tree {
  realizer: Realizer<StringTree>,
  language: Option<String>,
  head: Head,
}

impl Tree {
  fn new() -> Self {
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
  fn render_styles(&mut self) -> String {
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
  fn render_style_delta(&mut self) -> String {
    let mut delta = String::new();

    self.realizer.book.sheet.render_new(&mut delta);

    if delta.is_empty() {
      return delta;
    }

    format!("<style {STYLE_ATTRIBUTE}>{delta}</style>")
  }

  fn render_head(&self) -> String {
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
  fn render_body(&self, root: NodeId) -> String {
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
  fn stream(
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

/// The environment's signals with their write halves.
struct MediaState {
  /// The environment's signals.
  signals: MediaSignals<f32>,
  /// Writes the viewport.
  set_viewport: WriteSignal<Size<f32>>,
  /// Writes the color scheme.
  set_scheme: WriteSignal<ColorScheme>,
  /// Writes whether the reader prefers reduced motion.
  set_reduced_motion: WriteSignal<bool>,
  /// The server's render is never hydrated and shows no pixels.
  absent: ReadSignal<bool>,
}

thread_local! {
  static SCENE: RefCell<Scene<Web>> = RefCell::new(Scene::new());
  static TREE: RefCell<Tree> = RefCell::new(Tree::new());
  static HINTS: Cell<RenderHints> = const { Cell::new(RenderHints::DEFAULT) };
  static MEDIA: RefCell<Option<MediaState>> = const { RefCell::new(None) };
  static PREEMPT_SCHEME: Cell<bool> = const { Cell::new(false) };
  /// Whether the app opted into view transitions, so the stylesheet carries the
  /// cross-document rule.
  static VIEW_TRANSITIONS: Cell<bool> = const { Cell::new(false) };
  /// The path being rendered, as supplied by the runner; `/` when none was.
  static REQUEST_PATH: RefCell<String> = RefCell::new("/".to_owned());
  /// The nonce the renders' inline scripts carry, if any.
  static SCRIPT_NONCE: RefCell<Option<String>> = const { RefCell::new(None) };
  /// The client bundle the renders' pages boot, if any.
  static CLIENT_BUNDLE: RefCell<Option<String>> = const { RefCell::new(None) };
  /// Whether the renders' pages join the dev loop.
  static DEV_LOOP: Cell<bool> = const { Cell::new(false) };
  /// The statically-renderable routes the app recorded during the last render.
  static STATIC_ROUTES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
  /// Each deferred boundary's start anchor, mapped to a probe for whether its content
  /// has settled; consulted by the streaming walk to decide where to pause.
  static BOUNDARIES: RefCell<NodeMap<BoundarySettled>> = const { RefCell::new(NodeMap::new()) };
}

/// The script that applies a stored scheme choice before the first paint.
///
/// Inline and blocking in the head: it writes exactly what the platform's own rules are
/// gated on and reads only its own key, so a page that never chose a scheme is
/// untouched.
const PREEMPT_SCHEME_SCRIPT: &str = "try{var r=document.documentElement,\
s=localStorage.getItem('skene-scheme');\
if(s==='dark'||s==='light'){r.setAttribute('data-skene-scheme',s);r.style.colorScheme=s}}catch(e){}";

/// Sets the nonce the following renders' inline scripts carry, as the response's
/// Content Security Policy names it; `None` for none.
pub fn set_script_nonce(nonce: Option<&str>) {
  SCRIPT_NONCE.with(|current| *current.borrow_mut() = nonce.map(str::to_owned));
}

/// Sets the client bundle the following renders' pages boot and preload: the name
/// wasm-bindgen gave its files, served from the site root; `None` for none.
pub fn set_client_bundle(bundle: Option<&str>) {
  CLIENT_BUNDLE.with(|current| *current.borrow_mut() = bundle.map(str::to_owned));
}

/// Sets whether the following renders' pages join the dev loop of `skene run web`,
/// carrying its marker and its script.
pub fn set_dev_loop(dev: bool) {
  DEV_LOOP.with(|current| current.set(dev));
}

/// The document the following renders' pages are served as, in `language`.
fn served_document(language: Option<&str>) -> Document {
  Document {
    language: language.map(str::to_owned),
    bundle: CLIENT_BUNDLE.with(|bundle| bundle.borrow().clone()),
    nonce: SCRIPT_NONCE.with(|nonce| nonce.borrow().clone()),
    dev: DEV_LOOP.with(Cell::get),
  }
}

/// Routes `tracing` events to standard error, a render's deadline report among them,
/// unless the process already set a subscriber.
pub fn install_logging() {
  let _ = tracing::subscriber::set_global_default(crate::logging::LogSubscriber);
}

/// Sets the path the coming render is for, read back by [`current_path`].
pub fn set_request_path(path: &str) {
  REQUEST_PATH.with(|current| path.clone_into(&mut current.borrow_mut()));
}

/// The path being rendered, as supplied through [`set_request_path`].
#[must_use]
pub fn current_path() -> String {
  REQUEST_PATH.with(|current| current.borrow().clone())
}

/// Records the app's statically-renderable routes, readable after the render through
/// [`recorded_static_routes`].
pub fn record_static_routes(routes: Vec<String>) {
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = Some(routes));
}

/// The routes the last render recorded, or `None` when it recorded none.
#[must_use]
pub fn recorded_static_routes() -> Option<Vec<String>> {
  STATIC_ROUTES.with(|recorded| recorded.borrow().clone())
}

/// Forgets the routes the last render recorded, so the next starts clean.
fn forget_static_routes() {
  STATIC_ROUTES.with(|recorded| *recorded.borrow_mut() = None);
}

/// Runs `operation` on the tree.
fn with_tree<R>(operation: impl FnOnce(&mut Tree) -> R) -> R {
  TREE.with(|tree| operation(&mut tree.borrow_mut()))
}

/// Runs `operation` on the tree's realization.
fn with_realizer<R>(operation: impl FnOnce(&mut Realizer<StringTree>) -> R) -> R {
  with_tree(|tree| operation(&mut tree.realizer))
}

/// Sets the environment the following renders answer with.
pub fn set_render_hints(hints: RenderHints) {
  HINTS.with(|current| current.set(hints));
  MEDIA.with(|media| {
    if let Some(media) = &*media.borrow() {
      media.set_viewport.set(hints.viewport);
      media.set_scheme.set(hints.scheme);
      media.set_reduced_motion.set(hints.reduced_motion);
    }
  });
}

/// The environment's signals, created under the root owner on first use.
fn media_signals() -> MediaSignals<f32> {
  with_media(|media| media.signals)
}

/// Runs `operation` on the environment state, created under the root owner on first
/// use.
fn with_media<R>(operation: impl FnOnce(&MediaState) -> R) -> R {
  MEDIA.with(|media| {
    let mut media = media.borrow_mut();
    let media = media.get_or_insert_with(|| {
      let hints = HINTS.with(Cell::get);

      skene_core::root_owner().enter(|| {
        let (signals, writers) = MediaSignals::new(Environment {
          viewport: hints.viewport,
          scheme: hints.scheme,
          reduced_motion: hints.reduced_motion,
          reduced_transparency: hints.reduced_transparency,
          high_contrast: false,
          pointer: hints.pointer,
          hover: hints.hover,
          safe_area: Inset::ZERO,
        });
        let (absent, _) = Signal::opaque(false).split();

        MediaState {
          signals,
          set_viewport: writers.viewport,
          set_scheme: writers.scheme,
          set_reduced_motion: writers.reduced_motion,
          absent,
        }
      })
    });

    operation(media)
  })
}

impl Painting for Web {
  fn paint(_node: NodeId, _painter: Box<Painter>) {}

  fn available() -> ReadSignal<bool> {
    with_media(|media| media.absent)
  }
}

impl Platform for Web {
  type Scalar = f32;
  type StyleExt = GuiExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(node: NodeId, props: &BoxProps<Self>) {
    with_realizer(|realizer| realizer.create_box(node, props));
  }

  fn update_box(node: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiExt>>) {
    with_realizer(|realizer| realizer.update_box(node, slot, leaf));
  }

  fn update_focus_order(node: NodeId, focus_order: Option<FocusOrder>) {
    with_realizer(|realizer| realizer.update_focus_order(node, focus_order));
  }

  fn update_access(node: NodeId, access: &[AccessProp]) {
    with_realizer(|realizer| realizer.update_access(node, access));
  }

  fn create_control(node: NodeId, props: &ControlProps<Self>) {
    with_realizer(|realizer| realizer.create_control(node, props));
  }

  fn set_control_state(node: NodeId, state: &ControlState) {
    with_realizer(|realizer| realizer.set_control_state(node, *state));
  }

  fn create_overlay(node: NodeId, props: &OverlayProps<Self>) {
    with_realizer(|realizer| realizer.create_overlay(node, props));
  }

  fn set_overlay_open(_: NodeId, _: bool, _: Option<NodeId>) {}

  fn apply_layout(node: NodeId, layout: &StaticBoxStyle<Self>) {
    with_realizer(|realizer| realizer.apply_layout(node, layout));
  }

  fn create_run(node: NodeId, props: &RunProps<Self>) {
    with_realizer(|realizer| realizer.create_run(node, props));
  }

  fn update_run(node: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<GuiExt>>) {
    with_realizer(|realizer| realizer.update_run(node, slot, leaf));
  }

  fn set_text(node: NodeId, text: &str) {
    with_realizer(|realizer| realizer.set_text(node, text));
  }

  fn create_anchor(node: NodeId) {
    with_realizer(|realizer| realizer.create_anchor(node));
  }

  fn create_scope(node: NodeId) {
    with_realizer(|realizer| realizer.create_scope(node));
  }

  fn attach(parent: NodeId, child: NodeId, _before: Option<NodeId>) {
    with_realizer(|realizer| {
      let _label = realizer.enter_caption(parent, child);
    });
  }

  fn relocate(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    with_realizer(|realizer| {
      let _left = realizer.leave_caption(child);
    });
    Self::attach(parent, child, before);
  }

  fn hide(_parent: NodeId, child: NodeId) {
    with_realizer(|realizer| {
      let _left = realizer.leave_caption(child);
    });
  }

  fn remove(parent: NodeId, child: NodeId) {
    Self::hide(parent, child);
  }

  fn destroy(node: NodeId) {
    with_realizer(|realizer| {
      let _record = realizer.destroy(node);
    });
  }

  fn define_tokens(node: NodeId, definitions: &Definitions<WebTokenValue>) {
    with_realizer(|realizer| realizer.define_tokens(node, definitions));
  }

  fn define_root_tokens(definitions: &Definitions<WebTokenValue>) {
    with_realizer(|realizer| realizer.define_root_tokens(definitions));
  }

  fn listen(_: NodeId, _: EventKind) {}

  fn focus(_: NodeId) {}

  fn blur(_: NodeId) {}

  fn set_value(node: NodeId, value: &str) {
    with_realizer(|realizer| realizer.set_value(node, value));
  }

  fn set_options(node: NodeId, options: &[PickerOption]) {
    with_realizer(|realizer| realizer.set_options(node, options));
  }

  fn set_selection(_: NodeId, _: TextSelection) {}

  fn set_checked(node: NodeId, checked: bool) {
    with_realizer(|realizer| realizer.set_checked(node, checked));
  }

  fn observe_size(_: NodeId, _: Rc<dyn Fn(Size<f32>)>) -> Observation {
    Observation::unmeasured()
  }

  fn media() -> MediaSignals<f32> {
    media_signals()
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn hydration() -> Option<ReadSignal<bool>> {
    Some(with_media(|media| media.absent))
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    set_scheme(scheme);
  }

  /// The server renders once, so nothing scheduled ever runs.
  fn schedule(_: Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(_: Timer) {}

  fn stored_scheme() -> Option<ColorScheme> {
    stored_scheme()
  }
}

impl WebMarkup for Web {
  fn create_element(node: NodeId, tag: &'static str) {
    with_realizer(|realizer| {
      realizer.create_element(node, tag, Realized::Box);
    });
  }

  fn set_attribute(node: NodeId, name: &str, value: &str) {
    with_realizer(|realizer| realizer.set_attribute(node, name, value));
  }

  fn set_declarations(node: NodeId, declarations: &Declarations) {
    with_realizer(|realizer| realizer.set_declarations(node, declarations));
  }

  fn set_text(node: NodeId, text: &str) {
    <Self as Platform>::set_text(node, text);
  }
}

/// Marks the page as one whose reader may choose a scheme.
///
/// Its head then carries the blocking script applying the stored choice before the
/// first paint; the choice itself is the reader's and never known to a server.
pub fn set_scheme(_scheme: Option<ColorScheme>) {
  PREEMPT_SCHEME.with(|preempt| preempt.set(true));
}

/// The color scheme the reader chose on a previous visit; never known to a server.
#[inline(always)]
#[must_use]
pub const fn stored_scheme() -> Option<ColorScheme> {
  None
}

/// Opts the pages into view transitions, this render and every one after on the thread.
///
/// The stylesheet carries `@view-transition { navigation: auto }`, so a browser that
/// has them animates the cross-document navigations between the served pages; one that
/// does not cuts.
pub fn enable_view_transitions() {
  VIEW_TRANSITIONS.with(|enabled| enabled.set(true));
}

/// Registers `title` for the page, in force while it is the latest live one.
pub fn add_head_title(title: &str) -> HeadId {
  with_tree(|tree| tree.head.add_title(title.to_owned()))
}

/// Rewrites the title registration `id`.
pub fn set_head_title(id: HeadId, title: &str) {
  with_tree(|tree| tree.head.set_title(id, title.to_owned()));
}

/// Removes the head registration `id`.
pub fn remove_head_entry(id: HeadId) {
  with_tree(|tree| tree.head.remove(id));
}

/// Sets the page's language, a BCP 47 tag written as its root's `lang`.
pub fn set_document_language(language: &str) {
  with_tree(|tree| tree.language = Some(language.to_owned()));
}

/// The opening of a served document up to its body's content, in `language` when one
/// is set: see [`Document::opening`], the document carrying the bundle, the dev loop
/// and the nonce set for the renders.
#[must_use]
pub fn document_opening(language: Option<&str>, head: &str, styles: &str) -> String {
  served_document(language).opening(head, styles)
}

/// The closing of a served document from the hydration `state`: see
/// [`Document::closing`].
#[must_use]
pub fn document_closing(state: &str) -> String {
  served_document(None).closing(Some(state))
}

/// Registers an element for the page head, in force while it is the latest live one
/// of what it stands for; see [`Head`].
///
/// # Panics
///
/// Panics when `tag` is not an element a head holds (`base`, `link`, `meta` or
/// `script`) or an attribute name is not one.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) -> HeadId {
  with_tree(|tree| tree.head.add_element(HeadElement::new(tag, attributes)))
}

/// A server render, in parts: the stylesheet, the head, the body markup, the hydration
/// state and the page's language.
#[derive(Debug)]
#[must_use]
pub struct Rendered {
  styles: String,
  head: String,
  body: String,
  state: String,
  language: Option<String>,
}

impl Rendered {
  /// The `<style>` block: the reset, the slot and token registrations, and the interned
  /// rules in interning order.
  #[inline(always)]
  #[must_use]
  pub fn styles(&self) -> &str {
    &self.styles
  }

  /// The registered head content: the title and the elements added through
  /// [`add_head_element`].
  #[inline(always)]
  #[must_use]
  pub fn head(&self) -> &str {
    &self.head
  }

  /// The settled view tree: the page body.
  #[inline(always)]
  #[must_use]
  pub fn body(&self) -> &str {
    &self.body
  }

  /// The script element carrying the resource state the client adopts on hydration,
  /// escaped so it closes nothing, for a page of the app's own to embed.
  #[must_use]
  pub fn state_script(&self) -> String {
    state_script(&self.state)
  }

  /// The page's language, when the app set one.
  #[inline(always)]
  #[must_use]
  pub fn language(&self) -> Option<&str> {
    self.language.as_deref()
  }

  /// A complete HTML document: [`document_opening`], the tree, and
  /// [`document_closing`].
  #[must_use]
  pub fn page(&self) -> String {
    let mut page = document_opening(self.language(), &self.head, &self.styles);

    page.push_str(&self.body);
    page.push_str(&document_closing(&self.state));

    page
  }
}

/// Registers the segment `start` begins as a deferred boundary.
///
/// A streaming render flushes what precedes it, waits until `settled` reports true,
/// then emits its content in place. A whole render never consults it.
pub fn register_boundary(start: NodeId, settled: BoundarySettled) {
  if with_realizer(|realizer| realizer.records.contains(start)) {
    BOUNDARIES.with(|boundaries| boundaries.borrow_mut().insert(start, settled));
  }
}

/// Whether `start` begins a boundary whose content has not settled: the streaming
/// walk's pause test.
fn boundary_pending(start: NodeId) -> bool {
  BOUNDARIES.with(|boundaries| {
    boundaries
      .borrow()
      .get(start)
      .is_some_and(|settled| !settled())
  })
}

/// Starts a render clean: a fresh tree and scene, the root node, and the scope the app
/// builds under.
fn begin_render() -> (NodeId, Owner) {
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());
  Scene::<Web>::with(|scene| *scene = Scene::new());
  PREEMPT_SCHEME.with(|preempt| preempt.set(false));
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  crate::hydration::reset_sources();
  forget_static_routes();
  task::install_executor();

  let root = Scene::<Web>::with(|scene| scene.create(NodeKind::Box));

  with_realizer(|realizer| realizer.insert(root, ROOT, "", Realized::Box, false));

  (root, skene_core::root_owner().child())
}

/// Builds `app` under `scope` as the child of `root`, as one turn, with `boundary` in
/// context for the tasks it starts when one is given.
fn build_app<V, F>(root: NodeId, scope: Owner, boundary: Option<Boundary>, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  batch(|| {
    scope.enter(|| {
      if let Some(boundary) = boundary {
        provide_context(boundary);
      }

      let node = app().into_view().build();

      Scene::<Web>::with(|scene| scene.attach(root, node, None));
    });
  });
}

/// Tears a render down so the next one starts clean.
fn end_render(scope: Owner) {
  scope.dispose();
  task::clear();
  crate::hydration::reset_sources();
  BOUNDARIES.with(|boundaries| boundaries.borrow_mut().clear());
  Scene::<Web>::with(|scene| *scene = Scene::new());
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());
}

/// Renders `app` over a fresh tree.
///
/// It builds the app under a fresh scope, drives its async work to a fixpoint,
/// serializes the settled tree with its stylesheet, head and resource state, and tears
/// the render down so the next call starts clean.
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  let (root, scope) = begin_render();

  build_app(root, scope, None, app);
  task::stabilize();

  let state = crate::hydration::collect_state();
  let (styles, head, body, language) = with_tree(|tree| {
    (
      tree.render_styles(),
      tree.render_head(),
      tree.render_body(root),
      tree.language.clone(),
    )
  });

  end_render(scope);

  Rendered {
    styles,
    head,
    body,
    state,
    language,
  }
}

/// One piece of a streamed render, in emission order.
#[derive(Debug)]
pub enum StreamPiece<'a> {
  /// The settled shell's styles, head and language, before any body chunk.
  Shell {
    /// The `<style>` block.
    styles: &'a str,
    /// The registered head content.
    head: &'a str,
    /// The page's language, when the app set one.
    language: Option<&'a str>,
  },
  /// One body chunk: a `<style>` delta for what was interned since the previous
  /// flush, when anything was, then the markup up to the next deferred boundary, or
  /// after the last.
  Chunk(String),
  /// The encoded hydration state, once the walk completes.
  Tail(String),
}

/// Renders `app` as an in-order stream, each piece handed to `flush` as it is
/// produced: the shell once it settles, a body chunk at each deferred boundary, and
/// the hydration state last.
///
/// The shell settles against a boundary of its own and its styles render, then the
/// body walks in document order. At each deferred boundary the walk flushes the chunk
/// built so far, blocks until that boundary's content settles, and resumes emitting it
/// in place, opening the chunk with the styles interned since the previous flush, so a
/// client with no JavaScript receives a correct, styled document progressively, each
/// chunk appended to the last.
///
/// The head flushes with the shell: a title or head element registered inside a
/// deferred boundary is missing from the streamed page, a client receiving it only
/// once it hydrates, and the render logs a warning.
pub fn render_to_stream_with<V, F>(app: F, mut flush: impl FnMut(StreamPiece<'_>))
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  let (root, scope) = begin_render();
  let shell = Boundary::new();

  build_app(root, scope, Some(shell), app);
  task::stabilize_until(move || shell.is_settled());

  let (styles, head, language) = with_tree(|tree| {
    (
      tree.render_styles(),
      tree.render_head(),
      tree.language.clone(),
    )
  });

  flush(StreamPiece::Shell {
    styles: &styles,
    head: &head,
    language: language.as_deref(),
  });

  let mut cursor = vec![Frame {
    node: root,
    passed: None,
    passed_count: 0,
  }];

  loop {
    let mut chunk = String::new();
    let pending = with_tree(|tree| {
      chunk.push_str(&tree.render_style_delta());

      Scene::<Web>::read(|scene| tree.stream(scene, &mut cursor, &mut chunk, boundary_pending))
    });

    flush(StreamPiece::Chunk(chunk));

    let Some(start) = pending
    else {
      break;
    };

    task::stabilize_until(|| !boundary_pending(start));
    // Dropped whether or not it settled: settled emits its content, unsettled emits
    // what it has, and either way the walk must not pause on it again.
    BOUNDARIES.with(|boundaries| boundaries.borrow_mut().remove(start));
  }

  if with_tree(|tree| tree.render_head()) != head {
    tracing::warn!(
      "a head registration made after the shell flushed, inside a deferred boundary, \
       is missing from the streamed page until the client hydrates it"
    );
  }

  flush(StreamPiece::Tail(crate::hydration::collect_state()));
  end_render(scope);
}
