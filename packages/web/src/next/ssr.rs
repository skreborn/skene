//! The server leaf's realization of the driver: a string tree serialized to HTML with
//! the interned stylesheet, the head and the hydration state.
//!
//! Every element carries a creation-order `data-h` marker the browser leaf adopts by;
//! an anchor is a comment pair (`<!--s:N-->`, `<!--e:N-->`); a scope is a
//! `display: contents` element. Handlers are inert, focus and sizes are the client's,
//! and an overlay serializes closed.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{
  AccessProp,
  ColorScheme,
  FocusOrder,
  Size,
  Toggle,
  next::{
    BoxProps,
    BoxValue,
    ContainerSizes,
    ControlProps,
    ControlState,
    Definitions,
    Driver,
    EventKind,
    IntoView,
    MediaSignals,
    NodeKind,
    OverlayProps,
    PixelExt,
    RunProps,
    RunValue,
    Scene,
    Slot,
    TokenName,
    View,
  },
};
use skene_reactive::{Set, WriteSignal, signal};

use super::{
  Web,
  compile::{
    Compiled,
    SlotBinding,
    all_unconditional,
    box_fingerprint,
    compile_box,
    compile_control,
    compile_definitions,
    compile_overlay,
    compile_root_definitions,
    compile_run,
    inline_definitions,
    overlay_fingerprint,
    render_box_value,
    render_run_value,
    run_fingerprint,
    slot_registrations,
  },
  condition::{CLASS_PLACEHOLDER, CONTROL_ATTRIBUTE},
  markup::{
    MARKER_ATTRIBUTE,
    NATIVE_CLASS,
    STYLE_ID,
    access_attribute,
    anchor_text,
    drawn_element,
    element_tag,
    focus_order_attribute,
    link_attributes,
    native_element,
  },
  token::WebTokenValue,
};
use crate::{
  RESET_CSS,
  interner::ClassInterner,
  ssr::{escape_attr, escape_text},
};

/// The root of the string tree; its children are the page body.
const ROOT: u32 = 0;

/// What a node is in the string tree.
enum Content {
  Element(&'static str),
  Comment(String),
}

/// One node of the string tree.
struct Node {
  content: Content,
  attributes: Vec<(String, String)>,
  style_class: Option<Rc<str>>,
  token_class: Option<Rc<str>>,
  inline: Vec<(String, String)>,
  text: Option<String>,
  children: Vec<u32>,
  parent: Option<u32>,
  /// The native element carrying a control's state, focus and value, when the control
  /// has one apart from its root.
  carrier: Option<u32>,
  control: bool,
  slots: Rc<[SlotBinding]>,
}

impl Node {
  fn element(tag: &'static str) -> Self {
    Self {
      content: Content::Element(tag),
      attributes: Vec::new(),
      style_class: None,
      token_class: None,
      inline: Vec::new(),
      text: None,
      children: Vec::new(),
      parent: None,
      carrier: None,
      control: false,
      slots: Rc::from([]),
    }
  }

  fn comment(text: String) -> Self {
    Self {
      content: Content::Comment(text),
      ..Self::element("")
    }
  }

  fn tag(&self) -> &'static str {
    match self.content {
      Content::Element(tag) => tag,
      Content::Comment(_) => "",
    }
  }

  fn set_attribute(&mut self, name: &str, value: &str) {
    match self
      .attributes
      .iter_mut()
      .find(|(existing, _)| existing == name)
    {
      Some((_, existing)) => value.clone_into(existing),
      None => self.attributes.push((name.to_owned(), value.to_owned())),
    }
  }

  fn remove_attribute(&mut self, name: &str) {
    self.attributes.retain(|(existing, _)| existing != name);
  }

  fn set_flag(&mut self, name: &str, on: bool) {
    if on {
      self.set_attribute(name, "");
    }
    else {
      self.remove_attribute(name);
    }
  }

  fn set_inline(&mut self, property: String, value: String) {
    match self
      .inline
      .iter_mut()
      .find(|(existing, _)| *existing == property)
    {
      Some((_, existing)) => *existing = value,
      None => self.inline.push((property, value)),
    }
  }

  fn is_void(&self) -> bool {
    matches!(self.tag(), "input" | "br" | "hr" | "img")
  }

  fn emit_open(&self, index: u32, out: &mut String) {
    out.push('<');
    out.push_str(self.tag());
    out.push(' ');
    out.push_str(MARKER_ATTRIBUTE);
    out.push_str("=\"");
    out.push_str(&index.to_string());
    out.push('"');

    for (name, value) in &self.attributes {
      out.push(' ');
      out.push_str(name);

      if !value.is_empty() {
        out.push_str("=\"");
        escape_attr(value, out);
        out.push('"');
      }
    }

    let classes: Vec<&str> = self
      .style_class
      .iter()
      .chain(&self.token_class)
      .map(|class| &**class)
      .collect();

    if !classes.is_empty() {
      out.push_str(" class=\"");
      out.push_str(&classes.join(" "));
      out.push('"');
    }

    if !self.inline.is_empty() {
      out.push_str(" style=\"");

      let mut style = String::new();

      for (property, value) in &self.inline {
        style.push_str(property);
        style.push(':');
        style.push_str(value);
        style.push(';');
      }

      escape_attr(&style, out);
      out.push('"');
    }

    out.push('>');
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

/// The string tree, its stylesheet and the head, for one render.
struct Tree {
  nodes: Vec<Option<Node>>,
  styles: ClassInterner,
  cache: FxHashMap<u64, Rc<Compiled>>,
  registered: FxHashSet<TokenName>,
  registrations: String,
  root_class: Option<Rc<str>>,
  anchors: u32,
  title: Option<String>,
  head: Vec<(String, Vec<(String, String)>)>,
}

impl Tree {
  fn new() -> Self {
    Self {
      nodes: vec![Some(Node::element(""))],
      styles: ClassInterner::new(),
      cache: FxHashMap::default(),
      registered: FxHashSet::default(),
      registrations: String::new(),
      root_class: None,
      anchors: 0,
      title: None,
      head: Vec::new(),
    }
  }

  fn create(&mut self, node: Node) -> u32 {
    let index = u32::try_from(self.nodes.len()).expect("a tree holds at most 2^32 nodes");

    self.nodes.push(Some(node));

    index
  }

  fn element(&mut self, tag: &'static str) -> u32 {
    self.create(Node::element(tag))
  }

  fn node(&mut self, index: u32) -> Option<&mut Node> {
    self.nodes.get_mut(index as usize).and_then(Option::as_mut)
  }

  /// The element carrying `index`'s state: its native input, else itself.
  fn carrier(&mut self, index: u32) -> u32 {
    self
      .nodes
      .get(index as usize)
      .and_then(Option::as_ref)
      .and_then(|node| node.carrier)
      .unwrap_or(index)
  }

  /// The compiled template for `key`, rendered by `compile` on a miss.
  fn compiled(&mut self, key: u64, compile: impl FnOnce() -> Compiled) -> Rc<Compiled> {
    Rc::clone(self.cache.entry(key).or_insert_with(|| Rc::new(compile())))
  }

  /// Interns `template` and swaps it in for `previous`, returning the class.
  fn swap_class(&mut self, template: &str, previous: Option<&str>) -> Rc<str> {
    let (class, _) = self.styles.class_for(template);

    if previous != Some(&*class) {
      self.styles.retain(&class);

      if let Some(previous) = previous {
        self.styles.release(previous);
      }
    }

    class
  }

  fn assign_style(&mut self, index: u32, compiled: &Rc<Compiled>) {
    let previous = self.node(index).and_then(|node| node.style_class.clone());
    let class = self.swap_class(&compiled.template, previous.as_deref());

    if let Some(node) = self.node(index) {
      node.style_class = Some(class);
      node.slots = Rc::from(compiled.slots.as_slice());
    }
  }

  fn attach(&mut self, parent: u32, child: u32, before: Option<u32>) {
    if let Some(old) = self.node(child).and_then(|node| node.parent)
      && let Some(old) = self.node(old)
    {
      old.children.retain(|node| *node != child);
    }

    if let Some(node) = self.node(child) {
      node.parent = Some(parent);
    }

    if let Some(parent) = self.node(parent) {
      match before.and_then(|before| parent.children.iter().position(|node| *node == before)) {
        Some(index) => parent.children.insert(index, child),
        None => parent.children.push(child),
      }
    }
  }

  fn detach(&mut self, parent: u32, child: u32) {
    if let Some(parent) = self.node(parent) {
      parent.children.retain(|node| *node != child);
    }

    if let Some(child) = self.node(child) {
      child.parent = None;
    }
  }

  fn destroy(&mut self, index: u32) {
    let mut stack = vec![index];

    while let Some(current) = stack.pop() {
      let Some(node) = self.nodes.get_mut(current as usize).and_then(Option::take)
      else {
        continue;
      };

      stack.extend(node.children);

      for class in node.style_class.iter().chain(&node.token_class) {
        self.styles.release(class);
      }
    }
  }

  fn register(&mut self, definitions: &Definitions<WebTokenValue>) {
    for (name, value) in definitions.iter() {
      if self.registered.insert(name) {
        self.registrations.push_str(&value.registration(name));
      }
    }
  }

  fn render_styles(&self) -> String {
    let mut out = format!("<style id=\"{STYLE_ID}\">");

    out.push_str(RESET_CSS);
    out.push_str(&slot_registrations());
    out.push_str(&self.registrations);

    for (class, template) in self.styles.live_rules() {
      out.push_str(&template.replace(CLASS_PLACEHOLDER, class));
    }

    out.push_str("</style>");

    out
  }

  fn render_head(&self) -> String {
    let mut out = String::new();

    if let Some(title) = &self.title {
      out.push_str("<title>");
      escape_text(title, &mut out);
      out.push_str("</title>");
    }

    for (tag, attributes) in &self.head {
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

  fn render_body(&self) -> String {
    let mut out = String::new();

    if let Some(Some(root)) = self.nodes.first() {
      for child in &root.children {
        self.serialize(*child, &mut out);
      }
    }

    out
  }

  fn serialize(&self, index: u32, out: &mut String) {
    let Some(Some(node)) = self.nodes.get(index as usize)
    else {
      return;
    };

    if let Content::Comment(text) = &node.content {
      out.push_str("<!--");
      out.push_str(text);
      out.push_str("-->");

      return;
    }

    node.emit_open(index, out);

    match &node.text {
      Some(text) => escape_text(text, out),
      None => {
        for child in &node.children {
          self.serialize(*child, out);
        }
      }
    }

    node.emit_close(out);
  }
}

/// The environment a render answers with: what the client hints say, else the defaults.
///
/// Without hints a render assumes a 1024 by 768 viewport, the light scheme and no
/// reduced-motion preference.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct RenderHints {
  /// The viewport's size, in logical pixels.
  pub viewport: Size<f32>,
  /// The color scheme in force.
  pub scheme: ColorScheme,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: bool,
}

impl RenderHints {
  /// The defaults a render assumes without hints.
  pub const DEFAULT: Self = Self {
    viewport: Size::new(1024., 768.),
    scheme: ColorScheme::Light,
    reduced_motion: false,
  };
}

impl Default for RenderHints {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// The environment's signals with their write halves.
struct MediaState {
  signals: MediaSignals<f32>,
  set_viewport: WriteSignal<Size<f32>>,
  set_scheme: WriteSignal<ColorScheme>,
  set_reduced_motion: WriteSignal<bool>,
}

thread_local! {
  static SCENE: RefCell<Scene<Web>> = RefCell::new(Scene::new());
  static TREE: RefCell<Tree> = RefCell::new(Tree::new());
  static HINTS: Cell<RenderHints> = const { Cell::new(RenderHints::DEFAULT) };
  static MEDIA: RefCell<Option<MediaState>> = const { RefCell::new(None) };
}

/// Runs `operation` on the tree.
fn with_tree<R>(operation: impl FnOnce(&mut Tree) -> R) -> R {
  TREE.with(|tree| operation(&mut tree.borrow_mut()))
}

/// Runs `operation` on the node `index`, if it is live.
fn with_node<R>(index: u32, operation: impl FnOnce(&mut Node) -> R) -> Option<R> {
  with_tree(|tree| tree.node(index).map(operation))
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
  MEDIA.with(|media| {
    media
      .borrow_mut()
      .get_or_insert_with(|| {
        let hints = HINTS.with(Cell::get);

        skene_core::root_owner().enter(|| {
          let (viewport, set_viewport) = signal(hints.viewport);
          let (scheme, set_scheme) = signal(hints.scheme);
          let (reduced_motion, set_reduced_motion) = signal(hints.reduced_motion);

          MediaState {
            signals: MediaSignals {
              viewport,
              scheme,
              reduced_motion,
              container_sizes: ContainerSizes::new(),
            },
            set_viewport,
            set_scheme,
            set_reduced_motion,
          }
        })
      })
      .signals
  })
}

/// Writes `focus_order` as `tabindex` on `node`; `None` takes a control out of focus
/// and leaves a box without one.
fn write_focus_order(node: &mut Node, focus_order: Option<FocusOrder>) {
  match focus_order_attribute(focus_order, node.control) {
    Some(tabindex) => node.set_attribute("tabindex", &tabindex),
    None => node.remove_attribute("tabindex"),
  }
}

/// Replaces the ARIA attributes of `node` with those `access` maps to.
fn write_access(node: &mut Node, access: &[AccessProp]) {
  node
    .attributes
    .retain(|(name, _)| !name.starts_with("aria-"));

  for prop in access {
    let (name, value) = access_attribute(prop);

    node.set_attribute(name, &value);
  }
}

/// Realizes the props every box-like node shares onto `index`.
fn finish_box(index: u32, props: &BoxProps<Web>, compiled: &Rc<Compiled>) {
  with_tree(|tree| {
    tree.assign_style(index, compiled);

    let target = tree.carrier(index);

    if let Some(node) = tree.node(target) {
      write_focus_order(node, props.focus_order);
      write_access(node, &props.access);
    }
  });
}

impl Driver for Web {
  type Scalar = f32;
  type Sink = u32;
  type StyleExt = PixelExt;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(&self, props: &BoxProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = tree.element(element_tag(props.role));

      if let Some(role) = props.role.filter(|role| role.is_widget())
        && let Some(node) = tree.node(index)
      {
        node.set_attribute("role", crate::aria_role(role));
      }

      let compiled = tree.compiled(box_fingerprint(props, 0), || compile_box(props));

      (index, compiled)
    });

    finish_box(index, props, &compiled);

    index
  }

  fn update_box(&self, sink: &u32, slot: Slot, value: &BoxValue<f32, PixelExt>) {
    with_node(*sink, |node| {
      let binding = node
        .slots
        .iter()
        .find(|binding| binding.slot == slot)
        .cloned();

      if let Some(binding) = binding {
        node.set_inline(
          binding.custom_property(),
          render_box_value(binding.property, value),
        );
      }
    });
  }

  fn update_focus_order(&self, sink: &u32, focus_order: Option<FocusOrder>) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        write_focus_order(node, focus_order);
      }
    });
  }

  fn update_access(&self, sink: &u32, access: &[AccessProp]) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        write_access(node, access);
      }
    });
  }

  fn create_control(&self, props: &ControlProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = if let Some(native) = native_element(props.kind) {
        let root = tree.element("label");
        let input = tree.element(native.tag);

        if let Some(node) = tree.node(input) {
          if let Some(kind) = native.kind {
            node.set_attribute("type", kind);
          }

          node.set_attribute("class", NATIVE_CLASS);

          if let Some(placeholder) = &props.placeholder {
            node.set_attribute("placeholder", placeholder);
          }

          if native.tag == "textarea" {
            node.set_attribute("rows", &props.lines.to_string());
          }

          if let Some(label) = &props.label {
            node.set_attribute("aria-label", label);
          }
        }

        tree.attach(root, input, None);

        if let Some(node) = tree.node(root) {
          node.carrier = Some(input);
        }

        root
      }
      else {
        let (tag, attribute) = drawn_element(props.kind);
        let root = tree.element(tag);

        if let Some(node) = tree.node(root) {
          if let Some((name, value)) = attribute {
            node.set_attribute(name, value);
          }

          if let Some(label) = &props.label {
            node.set_attribute("aria-label", label);
          }
        }

        root
      };

      if let Some(node) = tree.node(index) {
        node.control = true;
        node.set_attribute(CONTROL_ATTRIBUTE, "");
      }

      let compiled = tree.compiled(box_fingerprint(&props.box_props, 1), || {
        compile_control(props)
      });

      (index, compiled)
    });

    finish_box(index, &props.box_props, &compiled);

    index
  }

  fn set_control_state(&self, sink: &u32, state: &ControlState) {
    with_tree(|tree| {
      let root = *sink;
      let carrier = tree.carrier(root);

      if carrier == root {
        let is_button = tree.node(root).is_some_and(|node| node.tag() == "button");

        if let Some(node) = tree.node(root) {
          node.set_flag("data-skene-checked", state.checked == Toggle::On);
          node.set_flag("data-skene-indeterminate", state.checked == Toggle::Mixed);
          node.set_flag("data-skene-disabled", state.disabled);
          node.set_flag("data-skene-invalid", state.invalid);

          if is_button {
            node.set_flag("disabled", state.disabled);
          }
        }
      }
      else if let Some(node) = tree.node(carrier) {
        node.set_flag("checked", state.checked == Toggle::On);
        node.set_flag("indeterminate", state.checked == Toggle::Mixed);
        node.set_flag("disabled", state.disabled);

        if state.invalid {
          node.set_attribute("aria-invalid", "true");
        }
        else {
          node.remove_attribute("aria-invalid");
        }
      }

      if let Some(node) = tree.node(root) {
        node.set_flag("data-skene-open", state.open);
      }
    });
  }

  fn create_overlay(&self, props: &OverlayProps<Self>) -> u32 {
    let (index, compiled) = with_tree(|tree| {
      let index = if props.modal {
        tree.element("dialog")
      }
      else {
        let index = tree.element("div");

        if let Some(node) = tree.node(index) {
          node.set_attribute("popover", "manual");
        }

        index
      };
      let compiled = tree.compiled(overlay_fingerprint(props), || compile_overlay(props));

      (index, compiled)
    });

    finish_box(index, &props.box_props, &compiled);

    index
  }

  fn set_overlay_open(&self, _: &u32, _: bool, _: Option<&u32>) {}

  fn create_run(&self, props: &RunProps<Self>) -> u32 {
    with_tree(|tree| {
      let index = match props.link.as_deref() {
        Some(href) => {
          let index = tree.element("a");

          if let Some(node) = tree.node(index) {
            for (name, value) in link_attributes(href, props.link_policy) {
              node.set_attribute(name, &value);
            }
          }

          index
        }
        None => tree.element("span"),
      };
      let compiled = tree.compiled(run_fingerprint(props), || compile_run(props));

      tree.assign_style(index, &compiled);

      index
    })
  }

  fn update_run(&self, sink: &u32, slot: Slot, value: &RunValue<PixelExt>) {
    with_node(*sink, |node| {
      let binding = node
        .slots
        .iter()
        .find(|binding| binding.slot == slot)
        .cloned();

      if let Some(binding) = binding {
        node.set_inline(
          binding.custom_property(),
          render_run_value(binding.property, value),
        );
      }
    });
  }

  fn set_text(&self, sink: &u32, text: &str) {
    with_node(*sink, |node| node.text = Some(text.to_owned()));
  }

  fn create_anchor(&self) -> u32 {
    with_tree(|tree| {
      let ordinal = tree.anchors;

      tree.anchors += 1;

      tree.create(Node::comment(anchor_text(ordinal)))
    })
  }

  fn create_scope(&self) -> u32 {
    with_tree(|tree| {
      let index = tree.element("div");

      if let Some(node) = tree.node(index) {
        node.set_inline("display".to_owned(), "contents".to_owned());
      }

      index
    })
  }

  fn attach(&self, parent: &u32, child: &u32, before: Option<&u32>) {
    with_tree(|tree| tree.attach(*parent, *child, before.copied()));
  }

  fn detach(&self, parent: &u32, child: &u32) {
    with_tree(|tree| tree.detach(*parent, *child));
  }

  fn destroy(&self, sink: u32) {
    with_tree(|tree| tree.destroy(sink));
  }

  fn define_tokens(&self, scope: &u32, definitions: &Definitions<WebTokenValue>) {
    with_tree(|tree| {
      tree.register(definitions);

      let previous = tree.node(*scope).and_then(|node| node.token_class.clone());

      if previous.is_some() && all_unconditional(definitions) {
        if let Some(node) = tree.node(*scope) {
          for (property, value) in inline_definitions(definitions) {
            node.set_inline(property, value);
          }
        }

        return;
      }

      let class = tree.swap_class(&compile_definitions(definitions), previous.as_deref());

      if let Some(node) = tree.node(*scope) {
        node.token_class = Some(class);
      }
    });
  }

  fn define_root_tokens(&self, definitions: &Definitions<WebTokenValue>) {
    with_tree(|tree| {
      tree.register(definitions);

      let previous = tree.root_class.take();
      let class = tree.swap_class(&compile_root_definitions(definitions), previous.as_deref());

      tree.root_class = Some(class);
    });
  }

  fn listen(&self, _: &u32, _: EventKind) {}

  fn focus(&self, _: &u32) {}

  fn blur(&self, _: &u32) {}

  fn set_value(&self, sink: &u32, value: &str) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        if node.tag() == "textarea" {
          node.text = Some(value.to_owned());
        }
        else {
          node.set_attribute("value", value);
        }
      }
    });
  }

  fn set_checked(&self, sink: &u32, checked: bool) {
    with_tree(|tree| {
      let target = tree.carrier(*sink);

      if let Some(node) = tree.node(target) {
        node.set_flag("checked", checked);
      }
    });
  }

  fn observe_size(&self, _: &u32, _: Rc<dyn Fn(Size<f32>)>) {}

  fn media(&self) -> MediaSignals<f32> {
    media_signals()
  }
}

/// Sets the document title, serialized into the page head.
pub fn set_document_title(title: &str) {
  with_tree(|tree| tree.title = Some(title.to_owned()));
}

/// Adds an element to the page head, serialized in registration order.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  with_tree(|tree| {
    tree.head.push((
      tag.to_owned(),
      attributes
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect(),
    ));
  });
}

/// A server render, in parts: the stylesheet, the head, the body markup and the
/// hydration state.
#[must_use]
pub struct Rendered {
  styles: String,
  head: String,
  body: String,
  state: String,
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

  /// The encoded resource state the client adopts on hydration.
  #[inline(always)]
  #[must_use]
  pub fn state(&self) -> &str {
    &self.state
  }

  /// A complete HTML document: the styles in `<head>`, the tree in `<body>`, and the
  /// state in a `<script>` the client reads back.
  #[must_use]
  pub fn page(&self) -> String {
    format!(
      "<!DOCTYPE html><html><head>{head}{styles}</head><body>{body}<script id=\"{id}\" \
       type=\"application/json\">{state}</script></body></html>",
      head = self.head,
      styles = self.styles,
      body = self.body,
      id = crate::STATE_SCRIPT_ID,
      state = self.state.replace('<', "\\u003c"),
    )
  }
}

/// Renders `app` over the new tree: builds it under a fresh scope, drives its async
/// work to a fixpoint, serializes the settled tree with its stylesheet, head and
/// resource state, and tears the render down so the next call starts clean.
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Web>,
{
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());
  Scene::<Web>::with(|scene| *scene = Scene::new());
  crate::ssr::reset_hydration_sources();

  let root = Scene::<Web>::with(|scene| scene.create(NodeKind::Box, ROOT));
  let scope = skene_core::root_owner().child();

  scope.enter(|| {
    let node = app().into_view().build();

    Scene::<Web>::with(|scene| scene.attach(root, node, None));
  });

  crate::ssr::task::stabilize();

  let state = crate::ssr::collect_hydration_state();
  let (styles, head, body) =
    with_tree(|tree| (tree.render_styles(), tree.render_head(), tree.render_body()));

  scope.dispose();
  crate::ssr::task::clear();
  crate::ssr::reset_hydration_sources();
  Scene::<Web>::with(|scene| *scene = Scene::new());
  TREE.with(|tree| *tree.borrow_mut() = Tree::new());

  Rendered {
    styles,
    head,
    body,
    state,
  }
}
