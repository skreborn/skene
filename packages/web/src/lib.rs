//! # skene web backend
//!
//! A [`Backend`] implementation that realizes a node tree against the browser DOM,
//! compiled to WebAssembly. Layout, painting, text shaping and hit-testing are all
//! offloaded to the browser; this crate creates elements, assigns styles through a
//! shared stylesheet, wires DOM event listeners back to the app, and defines its
//! own view types ([`Container`], [`Text`], [`Span`], …) implementing core's
//! [`View`]. Mount with [`launch`].

mod container;
mod declarations;
mod role;
mod style;
mod styler;
mod stylesheet;
mod text;

pub mod prelude;

use std::{
  cell::{Cell, RefCell},
  collections::hash_map::Entry,
  rc::Rc,
};

use rustc_hash::FxHashMap;
pub use skene_core::{
  AnyView,
  Children,
  DynamicView,
  HasChildren,
  IntoChildren,
  IntoView,
  NodeId,
  View,
  with_backend_as,
};
use skene_core::{Backend, Color, Number};
use stylesheet::StyleStore;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element};

pub use crate::{
  container::Container,
  declarations::{Declarations, StyleValue},
  role::Role,
  style::Border,
  styler::Styler,
  text::{RichText, Span, Text, TextStyle, TextTheme},
};

/// Runs a closure with access to the installed [`WebBackend`].
///
/// # Panics
///
/// Panics if no backend is installed, or it is not a [`WebBackend`].
#[inline(always)]
pub(crate) fn with_backend<R>(apply: impl FnOnce(&WebBackend) -> R) -> R {
  with_backend_as::<WebBackend, _>(apply)
}

const ROOT_ID: NodeId = NodeId::new(0);

/// A live DOM click-listener callback, kept alive for the backend's lifetime.
type ClickClosure = Closure<dyn FnMut(web_sys::Event)>;

/// The click handlers registered for each node. Shared (`Rc`) with the listener
/// closures so they can run without borrowing the backend.
type ClickHandlers = Rc<RefCell<FxHashMap<NodeId, Vec<Rc<dyn Fn()>>>>>;

/// The web backend. Owns the element registry, the stylesheet store, and the live
/// event-listener closures.
#[must_use]
pub struct WebBackend {
  /// The owning document, used to create elements.
  document: Document,
  /// Each node's DOM element.
  nodes: RefCell<FxHashMap<NodeId, Element>>,
  /// The generated style class currently assigned to each node (so it can be
  /// swapped when the node's declarations change). A shared [`Rc<str>`] with the
  /// stylesheet store, so the class name lives once no matter how many nodes use it.
  node_class: RefCell<FxHashMap<NodeId, Rc<str>>>,
  /// Each node's ordered children, mirroring the DOM tree so a subtree can be torn down on
  /// [`Backend::destroy`].
  children: RefCell<FxHashMap<NodeId, Vec<NodeId>>>,
  /// Each node's current parent.
  parents: RefCell<FxHashMap<NodeId, NodeId>>,
  /// The shared stylesheet store.
  styles: RefCell<StyleStore>,
  /// Each text node's interned static declarations, kept so a static style change can
  /// re-derive the class. Reactive changes take the inline path instead.
  text_styles: RefCell<FxHashMap<NodeId, Declarations>>,
  /// The next node id to allocate.
  next_id: Cell<u64>,
  /// The click handlers registered for each node.
  click_handlers: ClickHandlers,
  /// One live click listener per node, kept alive until the node is destroyed. Dropping
  /// it detaches the DOM listener.
  click_listeners: RefCell<FxHashMap<NodeId, ClickClosure>>,
}

impl WebBackend {
  /// Creates a backend mounted at `mount` (the element new views are inserted
  /// under).
  ///
  /// # Panics
  ///
  /// Panics if there is no `window`/`document` (i.e. not running in a browser).
  pub fn new(mount: Element) -> WebBackend {
    const RESET_CSS: &str = include_str!("reset.css");

    let document = web_sys::window()
      .expect("no window")
      .document()
      .expect("no document");

    let head = document.head().expect("document has a <head>");

    let reset = document
      .create_element("style")
      .expect("create reset <style> element");

    reset.set_text_content(Some(RESET_CSS));

    head
      .append_child(&reset)
      .expect("append reset <style> to <head>");

    let styles = StyleStore::new(&document, &head);
    let mut nodes = FxHashMap::default();

    nodes.insert(ROOT_ID, mount);

    WebBackend {
      document,
      nodes: RefCell::new(nodes),
      node_class: RefCell::new(FxHashMap::default()),
      children: RefCell::new(FxHashMap::default()),
      parents: RefCell::new(FxHashMap::default()),
      styles: RefCell::new(styles),
      text_styles: RefCell::new(FxHashMap::default()),
      next_id: Cell::new(1),
      click_handlers: Rc::new(RefCell::new(FxHashMap::default())),
      click_listeners: RefCell::new(FxHashMap::default()),
    }
  }

  fn alloc_id(&self) -> NodeId {
    let id = NodeId::new(self.next_id.get());

    self.next_id.set(self.next_id.get() + 1);

    id
  }

  /// Registers a freshly created element under a new node id.
  fn register(&self, element: Element) -> NodeId {
    let id = self.alloc_id();

    self.nodes.borrow_mut().insert(id, element);

    id
  }

  /// Runs `f` with the node's element, without cloning the handle. `f` must not make
  /// a backend call that inserts or removes nodes (which would borrow `nodes` again).
  fn with_element<R>(&self, node: NodeId, apply: impl FnOnce(&Element) -> R) -> R {
    let nodes = self.nodes.borrow();
    let element = nodes.get(&node).expect("web backend: unknown node id");

    apply(element)
  }

  /// Interns `declarations` into a class and swaps it onto `node`, refcounting the
  /// dropped and adopted rules so unused ones are collected.
  fn apply_class(&self, node: NodeId, declarations: &Declarations) {
    let previous = self.node_class.borrow().get(&node).cloned();

    // Intern and swap the refcounts in one borrow. No change? leave the DOM and the
    // class's refcount alone.
    let class = {
      let mut styles = self.styles.borrow_mut();
      let class = styles.class_for(declarations);

      if previous.as_deref() == Some(&*class) {
        return;
      }

      styles.retain(&class);

      if let Some(old) = &previous {
        styles.release(old);
      }

      class
    };

    self.with_element(node, |element| {
      let list = element.class_list();

      if let Some(old) = &previous {
        let _ = list.remove_1(old);
      }

      let _ = list.add_1(&class);
    });

    self.node_class.borrow_mut().insert(node, class);
  }

  /// Updates a text node's accumulated declarations and reapplies them as its class.
  fn restyle_text(&self, node: NodeId, update: impl FnOnce(Declarations) -> Declarations) {
    let declarations = {
      let mut styles = self.text_styles.borrow_mut();
      let entry = styles.entry(node).or_default();

      *entry = update(std::mem::take(entry));

      entry.clone()
    };

    self.apply_class(node, &declarations);
  }

  /// Registers `handler` to run when `node` is clicked, attaching a single click
  /// listener per node that runs every click handler for it.
  fn register_click(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self
      .click_handlers
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let mut listeners = self.click_listeners.borrow_mut();

    let Entry::Vacant(slot) = listeners.entry(node)
    else {
      return;
    };

    let click_handlers = Rc::clone(&self.click_handlers);

    // Cloning the handlers out before invoking keeps the borrow short, so a handler
    // may re-enter the backend (e.g. to build or destroy nodes).
    let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
      let Some(to_run) = click_handlers.borrow().get(&node).cloned()
      else {
        return;
      };

      for handler in to_run {
        handler();
      }
    }) as Box<dyn FnMut(web_sys::Event)>);

    self.with_element(node, |element| {
      let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    });

    slot.insert(closure);
  }

  /// Allocates a container node: an element chosen by its optional semantic [`Role`]
  /// (e.g. a button, else a plain grouping element). Detached until inserted with
  /// [`Backend::insert_child`].
  ///
  /// # Panics
  ///
  /// Panics if the document cannot create the element.
  pub fn create_container(&self, role: Option<Role>) -> NodeId {
    let element = self
      .document
      .create_element(role.map_or("div", Role::tag))
      .expect("create element");

    // A bare <button> defaults to form-submit; we only want the click handler.
    if matches!(role, Some(Role::Button)) {
      let _ = element.set_attribute("type", "button");
    }

    self.register(element)
  }

  /// Allocates a plain text node (an inline, styleable run of text). Its content is
  /// set with [`WebBackend::set_text`]. Detached until inserted.
  ///
  /// # Panics
  ///
  /// Panics if the document cannot create the element.
  pub fn create_text(&self) -> NodeId {
    let element = self
      .document
      .create_element("span")
      .expect("create element");

    self.register(element)
  }

  /// Allocates a text node that links to `destination`. Detached until inserted.
  ///
  /// # Panics
  ///
  /// Panics if the document cannot create the element.
  pub fn create_anchor(&self, destination: &str) -> NodeId {
    let element = self.document.create_element("a").expect("create element");

    let _ = element.set_attribute("href", destination);

    self.register(element)
  }

  /// Applies a node's style: a set of CSS declarations.
  ///
  /// The first call interns `declarations` as a shared, content-addressed class; a
  /// later call (for a reactive change) applies its declarations as inline overrides,
  /// so the class stays stable. Pass the full set initially and only the changed
  /// declarations thereafter.
  pub fn set_css(&self, node: NodeId, declarations: &Declarations) {
    let classed = self.node_class.borrow().contains_key(&node);

    if classed {
      self.with_element(node, |element| {
        if let Some(html) = element.dyn_ref::<web_sys::HtmlElement>() {
          let style = html.style();
          let mut buffer = String::new();

          for (property, value) in declarations.iter() {
            buffer.clear();
            value.write_css(&mut buffer);

            let _ = style.set_property(property, &buffer);
          }
        }
      });
    }
    else {
      self.apply_class(node, declarations);
    }
  }

  /// Sets a single raw CSS property as an inline override on a node.
  pub fn set_property(&self, node: NodeId, property: &str, value: &str) {
    self.with_element(node, |element| {
      if let Some(html) = element.dyn_ref::<web_sys::HtmlElement>() {
        let _ = html.style().set_property(property, value);
      }
    });
  }

  /// Sets the text content of a node.
  pub fn set_text(&self, node: NodeId, text: &str) {
    self.with_element(node, |element| element.set_text_content(Some(text)));
  }

  /// Sets a text node's color as an inline override, for a reactive update: it wins
  /// over the interned class, so an animating property never rewrites the shared
  /// stylesheet.
  pub fn set_text_color(&self, node: NodeId, color: Color) {
    self.set_property(node, "color", &StyleValue::Color(color).to_css());
  }

  /// Sets a text node's font size as an inline override (see
  /// [`set_text_color`](Self::set_text_color)).
  pub fn set_font_size(&self, node: NodeId, size: Number) {
    self.set_property(node, "font-size", &StyleValue::Length(size).to_css());
  }

  /// Sets a text node's font weight as an inline override (see
  /// [`set_text_color`](Self::set_text_color)).
  pub fn set_font_weight(&self, node: NodeId, weight: Number) {
    self.set_property(node, "font-weight", &StyleValue::Number(weight).to_css());
  }

  /// Sets a text node's color, size, and weight together, interning the class once
  /// rather than once per property.
  pub fn set_text_style(
    &self,
    node: NodeId,
    color: Option<Color>,
    size: Option<Number>,
    weight: Option<Number>,
  ) {
    self.restyle_text(node, |mut style| {
      if let Some(color) = color {
        style = style.set("color", StyleValue::Color(color));
      }

      if let Some(size) = size {
        style = style.set("font-size", StyleValue::Length(size));
      }

      if let Some(weight) = weight {
        style = style.set("font-weight", StyleValue::Number(weight));
      }

      style
    });
  }

  /// Marks a standalone text leaf as block-level, so it lays out consistently
  /// regardless of its parent and never as an inline run. Not for the inline spans of
  /// a [`RichText`](crate::RichText).
  pub fn set_text_block(&self, node: NodeId) {
    self.restyle_text(node, |style| {
      style.set("display", StyleValue::Keyword("block"))
    });
  }

  /// Marks `node` an inline-level block box (`inline-block`): it sits inline among
  /// surrounding text while its contents lay out as a block, contained within it. Used
  /// to embed an arbitrary view inline within a [`RichText`](crate::RichText).
  pub fn set_inline_block(&self, node: NodeId) {
    self.restyle_text(node, |style| {
      style.set("display", StyleValue::Keyword("inline-block"))
    });
  }

  /// Registers `handler` to run when `node` is clicked, until the node is destroyed.
  pub fn on_click(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self.register_click(node, handler);
  }
}

impl Backend for WebBackend {
  fn root(&self) -> NodeId {
    ROOT_ID
  }

  fn create_group(&self) -> NodeId {
    let element = self.document.create_element("div").expect("create element");

    self.register(element)
  }

  fn attach_child(&self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    {
      // Mirror the move in the children map (detach from any current parent first).
      let mut children = self.children.borrow_mut();

      if let Some(old_parent) = self.parents.borrow_mut().insert(child, parent)
        && let Some(list) = children.get_mut(&old_parent)
      {
        list.retain(|node| *node != child);
      }

      let list = children.entry(parent).or_default();

      match before.and_then(|before| list.iter().position(|node| *node == before)) {
        Some(i) => list.insert(i, child),
        None => list.push(child),
      }
    }

    let nodes = self.nodes.borrow();
    let parent_el = nodes.get(&parent).expect("web backend: unknown node id");
    let child_el = nodes.get(&child).expect("web backend: unknown node id");

    match before {
      Some(before) => {
        let before_el = nodes.get(&before).expect("web backend: unknown node id");
        let _ = parent_el.insert_before(child_el.as_ref(), Some(before_el.as_ref()));
      }
      None => {
        let _ = parent_el.append_child(child_el.as_ref());
      }
    }
  }

  fn detach_child(&self, parent: NodeId, child: NodeId) {
    if let Some(list) = self.children.borrow_mut().get_mut(&parent) {
      list.retain(|node| *node != child);
    }

    self.parents.borrow_mut().remove(&child);

    let nodes = self.nodes.borrow();
    let parent_el = nodes.get(&parent).expect("web backend: unknown node id");
    let child_el = nodes.get(&child).expect("web backend: unknown node id");
    let _ = parent_el.remove_child(child_el.as_ref());
  }

  fn destroy(&self, node: NodeId) {
    // Tear down the whole detached subtree, releasing each node's element, class, handlers and
    // listener closures. Dropping a closure detaches its DOM listener, so no `removeEventListener`
    // is needed. No user code runs during teardown, so every map is borrowed once up front.
    let mut stack = vec![node];

    let mut children = self.children.borrow_mut();
    let mut parents = self.parents.borrow_mut();
    let mut nodes = self.nodes.borrow_mut();
    let mut text_styles = self.text_styles.borrow_mut();
    let mut node_class = self.node_class.borrow_mut();
    let mut styles = self.styles.borrow_mut();
    let mut click_handlers = self.click_handlers.borrow_mut();
    let mut click_listeners = self.click_listeners.borrow_mut();

    while let Some(current) = stack.pop() {
      if let Some(descendants) = children.remove(&current) {
        stack.extend(descendants);
      }

      parents.remove(&current);
      nodes.remove(&current);
      text_styles.remove(&current);

      if let Some(class) = node_class.remove(&current) {
        styles.release(&class);
      }

      click_handlers.remove(&current);
      click_listeners.remove(&current);
    }
  }
}

/// Builds `app` as the root view under the installed backend's root node.
pub fn mount<V: IntoView>(app: impl FnOnce() -> V) -> NodeId {
  skene_core::mount(|| app().into_view().build())
}

/// Mounts and runs a skene application in the browser.
///
/// Installs a [`WebBackend`] rooted at `<body>`, builds `app` as the root view,
/// and returns. The reactive runtime, event listeners and view tree are kept
/// alive for the lifetime of the page.
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>` (i.e. not running in
/// a browser).
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  console_error_panic_hook::set_once();

  let window = web_sys::window().expect("no window");
  let document = window.document().expect("no document");
  let body = document.body().expect("document has a <body>");

  let backend = WebBackend::new(body.into());
  let backend: Rc<dyn Backend> = Rc::new(backend);

  skene_core::install_backend(backend);

  mount(app);
}
