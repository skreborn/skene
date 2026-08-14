//! The web platform: a browser DOM.
//!
//! Creates elements, styles them through a shared stylesheet, sets text and wires click
//! listeners—what the built-in views realize against, and the escape hatch a low-level
//! web view calls directly. Layout, painting, text shaping and hit-testing are the
//! browser's. Mount with [`launch`].

pub(crate) mod tree;

#[cfg(feature = "snapshot")]
mod dev;
mod events;
mod history;
mod logging;
mod stylesheet;
mod task;

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{
  IntoView,
  KeyEvent,
  Modifiers,
  NodeId,
  Placement,
  PointerButton,
  PointerEvent,
  Prevention,
  ScrollEvent,
  View,
};
use wasm_bindgen::JsCast;
use web_sys::{
  Document,
  Element,
  HtmlElement,
  HtmlInputElement,
  HtmlSelectElement,
  HtmlTextAreaElement,
  KeyboardEvent,
};

#[cfg(feature = "snapshot")]
pub use self::dev::boot_with_dev_state;
use self::stylesheet::StyleStore;
pub use self::{
  history::{current_path, on_pop, push_path},
  task::drive,
};
// The CSS declaration set is web-family shared; this leaf takes it in `set_style`.
use crate::Declarations;

const ROOT_ID: NodeId = NodeId::new(0);

/// The web platform's state: the element registry, the stylesheet store, and the live
/// event-listener closures. Private—reached only through this module's functions.
#[must_use]
struct State {
  /// The owning document, used to create elements.
  document: Document,
  /// Each node's DOM element.
  nodes: RefCell<FxHashMap<NodeId, Element>>,
  /// The generated style class currently assigned to each node (so it can be
  /// swapped when the node's declarations change). A shared [`Rc<str>`] with the
  /// stylesheet store, so the class name lives once no matter how many nodes use it.
  node_class: RefCell<FxHashMap<NodeId, Rc<str>>>,
  /// Each node's ordered children, mirroring the DOM tree so a subtree can be torn down on
  /// [`destroy`].
  children: RefCell<FxHashMap<NodeId, Vec<NodeId>>>,
  /// Each node's current parent.
  parents: RefCell<FxHashMap<NodeId, NodeId>>,
  /// The shared stylesheet store.
  styles: RefCell<StyleStore>,
  /// The next node id to allocate.
  next_id: Cell<u64>,
  /// The event registries and live listeners.
  events: events::Events,
  /// While hydrating, the server-rendered elements indexed by their creation-order marker;
  /// `None` in normal operation, when nodes are created fresh.
  hydration: RefCell<Option<FxHashMap<u64, Element>>>,
  /// The nodes adopted in place during hydration, so their attach skips the DOM insert; cleared
  /// when hydration ends.
  adopted: RefCell<FxHashSet<NodeId>>,
  /// Each overlay node's realization config.
  overlays: RefCell<FxHashMap<NodeId, (bool, Placement)>>,
  /// Each watched node's size observer and its live callback closure.
  #[expect(clippy::type_complexity)]
  observers: RefCell<
    FxHashMap<
      NodeId,
      (
        web_sys::ResizeObserver,
        wasm_bindgen::closure::Closure<dyn FnMut()>,
      ),
    >,
  >,
}

impl State {
  /// Creates the state rooted at `mount` (the element new views are inserted under).
  ///
  /// # Panics
  ///
  /// Panics if there is no `window`/`document` (i.e. not running in a browser).
  pub fn new(mount: Element) -> State {
    let document = web_sys::window()
      .expect("a window")
      .document()
      .expect("a document");

    let head = document.head().expect("a head");

    let reset = document.create_element("style").expect("a style element");

    reset.set_text_content(Some(crate::RESET_CSS));

    head
      .append_child(&reset)
      .expect("a style element to be appended to the head");

    let styles = StyleStore::new(&document, &head);
    let mut nodes = FxHashMap::default();

    nodes.insert(ROOT_ID, mount);

    State {
      document,
      nodes: RefCell::new(nodes),
      node_class: RefCell::new(FxHashMap::default()),
      children: RefCell::new(FxHashMap::default()),
      parents: RefCell::new(FxHashMap::default()),
      styles: RefCell::new(styles),
      next_id: Cell::new(1),
      events: events::Events::new(),
      hydration: RefCell::new(None),
      adopted: RefCell::new(FxHashSet::default()),
      overlays: RefCell::new(FxHashMap::default()),
      observers: RefCell::new(FxHashMap::default()),
    }
  }

  /// Creates a DOM element with the given `tag` as a detached, tree-participating node.
  ///
  /// # Panics
  ///
  /// Panics if the document cannot create the element.
  pub fn create_element(&self, tag: &str) -> NodeId {
    self.build_node(tag)
  }

  /// The DOM element backing `node`, to manipulate directly: set attributes, append
  /// children, or hand it to a JS library.
  ///
  /// # Panics
  ///
  /// Panics if `node` is not a node of this platform.
  #[must_use]
  pub fn element(&self, node: NodeId) -> Element {
    self.with_element(node, Clone::clone)
  }

  /// Interns `template` as a shared, content-addressed class and assigns it to `node`,
  /// refcounting rules so unused ones are collected. The batched path—apply a node's
  /// full style once. It stays SSR-compatible (the class is content-addressed).
  pub fn assign_class(&self, node: NodeId, template: &str) {
    let previous = self.node_class.borrow().get(&node).cloned();

    // Intern and swap the refcounts in one borrow. No change? leave the DOM and the
    // class's refcount alone.
    let class = {
      let mut styles = self.styles.borrow_mut();
      let class = styles.class_for(template);

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

  /// Applies each of `declarations` as an inline override on `node`, winning over its
  /// interned class—the path for a reactive change, so an animating property never
  /// rewrites the shared stylesheet.
  pub fn override_style(&self, node: NodeId, declarations: &Declarations) {
    self.with_element(node, |element| {
      if let Some(html) = element.dyn_ref::<HtmlElement>() {
        let style = html.style();

        for (property, value) in declarations.iter() {
          let _ = style.set_property(property, value);
        }
      }
    });
  }

  /// Sets a node's text content.
  pub fn set_text(&self, node: NodeId, text: &str) {
    self.with_element(node, |element| element.set_text_content(Some(text)));
  }

  /// Registers `handler` to run when `node` is clicked (or a descendant's click bubbles
  /// to it), until the node is destroyed. Delivered by delegation: one listener on the
  /// mount root serves every node.
  pub fn handle_click(&self, node: NodeId, handler: Rc<skene_core::ClickHandler>) {
    self
      .events
      .click
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let handlers = Rc::clone(&self.events.click);

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, "click", move |id, _element, _event| {
          events::run_preventable(&handlers, id, (), |run, ()| run())
        });
    });
  }

  /// Registers `handler` to run when `node` gains focus, until the node is destroyed.
  pub fn handle_focus(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self
      .events
      .focus
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let handlers = Rc::clone(&self.events.focus);

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, "focusin", move |id, _element, _event| {
          events::run_handlers(&handlers, id, (), |run, ()| run());

          Prevention::default()
        });
    });
  }

  /// Registers `handler` to run when `node` loses focus, until the node is destroyed.
  pub fn handle_blur(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self
      .events
      .blur
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let handlers = Rc::clone(&self.events.blur);

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, "focusout", move |id, _element, _event| {
          events::run_handlers(&handlers, id, (), |run, ()| run());

          Prevention::default()
        });
    });
  }

  /// Registers `handler` to run when a key is pressed on `node` or within it, until the
  /// node is destroyed.
  pub fn handle_key_down(&self, node: NodeId, handler: Rc<skene_core::KeyHandler>) {
    self
      .events
      .key_down
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let handlers = Rc::clone(&self.events.key_down);

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, "keydown", move |id, _element, event| {
          let Some(keyboard) = event.dyn_ref::<KeyboardEvent>()
          else {
            return Prevention::default();
          };

          let key_event = KeyEvent {
            key: keyboard.key(),
            modifiers: Modifiers {
              ctrl: keyboard.ctrl_key(),
              shift: keyboard.shift_key(),
              alt: keyboard.alt_key(),
              meta: keyboard.meta_key(),
            },
          };

          events::run_preventable(&handlers, id, &key_event, |run, key_event| run(key_event))
        });
    });
  }

  /// Registers `handler` to run when the pointer enters `node`, until the node is
  /// destroyed.
  pub fn handle_pointer_enter(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self.with_element(node, |element| {
      self
        .events
        .add_direct_listener(node, element, "pointerenter", move |_| handler());
    });
  }

  /// Registers `handler` to run when the pointer leaves `node`, until the node is
  /// destroyed.
  pub fn handle_pointer_leave(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    self.with_element(node, |element| {
      self
        .events
        .add_direct_listener(node, element, "pointerleave", move |_| handler());
    });
  }

  /// Registers `handler` to run when a pointer button is pressed on `node` or within it,
  /// until the node is destroyed.
  pub fn handle_pointer_down(&self, node: NodeId, handler: Rc<skene_core::PointerHandler>) {
    self.delegate_pointer(node, handler, |events| &events.pointer_down, "pointerdown");
  }

  /// Registers `handler` to run when a pointer button is released on `node` or within
  /// it, until the node is destroyed.
  pub fn handle_pointer_up(&self, node: NodeId, handler: Rc<skene_core::PointerHandler>) {
    self.delegate_pointer(node, handler, |events| &events.pointer_up, "pointerup");
  }

  /// Registers `handler` to run when the pointer moves over `node` or within it, until
  /// the node is destroyed.
  pub fn handle_pointer_move(&self, node: NodeId, handler: Rc<skene_core::PointerHandler>) {
    self.delegate_pointer(node, handler, |events| &events.pointer_move, "pointermove");
  }

  /// Registers a delegated pointer handler for `event_name`, backed by the registry
  /// `registry` selects.
  fn delegate_pointer(
    &self,
    node: NodeId,
    handler: Rc<skene_core::PointerHandler>,
    registry: impl Fn(&events::Events) -> &events::Handlers<skene_core::PointerHandler>,
    event_name: &'static str,
  ) {
    registry(&self.events)
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    let handlers = Rc::clone(registry(&self.events));

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, event_name, move |id, element, event| {
          let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>()
          else {
            return Prevention::default();
          };

          let payload = pointer_payload(pointer, element);

          events::run_preventable(&handlers, id, &payload, |run, payload| run(payload))
        });
    });
  }

  /// Watches `node`'s content size, running `handler` with its width and height in
  /// logical pixels once observed and on every change, until the node is destroyed.
  pub fn observe_size(&self, node: NodeId, handler: Rc<dyn Fn(f32, f32)>) {
    self.with_element(node, |element| {
      let target = element.clone();
      let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        #[expect(clippy::cast_precision_loss)]
        handler(target.client_width() as f32, target.client_height() as f32);
      });
      let observer =
        web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref()).expect("a resize observer");

      observer.observe(element);
      self
        .observers
        .borrow_mut()
        .insert(node, (observer, closure));
    });
  }

  /// Registers `handler` to run with the scrolled offsets when `node` scrolls, until the
  /// node is destroyed.
  pub fn handle_scroll(&self, node: NodeId, handler: Rc<dyn Fn(&ScrollEvent)>) {
    self.with_element(node, |element| {
      self
        .events
        .add_direct_listener(node, element, "scroll", move |event| {
          let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
          else {
            return;
          };

          #[expect(clippy::cast_precision_loss)]
          handler(&ScrollEvent {
            x: target.scroll_left() as f32,
            y: target.scroll_top() as f32,
          });
        });
    });
  }

  /// Registers `handler` to run with the new value on every change of `node`'s (or a
  /// descendant control's) value, in real time, until the node is destroyed.
  pub fn handle_change(&self, node: NodeId, handler: Rc<dyn Fn(&str)>) {
    self
      .events
      .change
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    self.ensure_input_listener();
  }

  /// Registers `handler` to run with the new checked state, in real time, when `node`'s
  /// (or a descendant toggle's) state changes, until the node is destroyed.
  pub fn handle_toggle(&self, node: NodeId, handler: Rc<dyn Fn(bool)>) {
    self
      .events
      .toggle
      .borrow_mut()
      .entry(node)
      .or_default()
      .push(handler);

    self.ensure_input_listener();
  }

  /// Installs the one delegated `input` listener serving both the change and toggle
  /// registries—the two share the event, so a lone per-registry listener would starve
  /// whichever registered second.
  fn ensure_input_listener(&self) {
    let change = Rc::clone(&self.events.change);
    let toggle = Rc::clone(&self.events.toggle);

    self.with_element(ROOT_ID, |root| {
      self
        .events
        .ensure_root_listener(root, "input", move |id, _element, event| {
          if let Some(value) = element_value(event) {
            events::run_handlers(&change, id, value.as_str(), |run, value| run(value));
          }

          if let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
          {
            refresh_slider_fill(&input);
            events::run_handlers(&toggle, id, input.checked(), |run, checked| run(checked));
          }

          Prevention::default()
        });
    });
  }

  /// Records an overlay's realization config, consulted when it opens.
  pub fn register_overlay(&self, node: NodeId, modal: bool, placement: Placement) {
    self.overlays.borrow_mut().insert(node, (modal, placement));
  }

  /// Opens or closes an overlay: a modal shows as a dialog (top layer, focus trapped),
  /// a popup shows as a popover—positioned against `anchor` when one is bound.
  pub fn set_overlay_open(&self, node: NodeId, open: bool, anchor: Option<NodeId>) {
    let Some((modal, placement)) = self.overlays.borrow().get(&node).copied()
    else {
      return;
    };

    let element = self.element(node);
    let anchor = anchor.map(|anchor| self.element(anchor));

    if modal {
      let Some(dialog) = element.dyn_ref::<web_sys::HtmlDialogElement>()
      else {
        return;
      };

      if open {
        let _ = dialog.show_modal();
      }
      else {
        dialog.close();
      }

      return;
    }

    let Some(html) = element.dyn_ref::<HtmlElement>()
    else {
      return;
    };

    if open {
      let _ = html.show_popover();

      if let Some(anchor) = anchor {
        position_at_anchor(html, &anchor, placement);
      }
    }
    else {
      let _ = html.hide_popover();
    }
  }

  /// Registers `handler` to run when the user asks the overlay to close: Escape, a
  /// modal's backdrop click, or a pointer press outside a popup. The overlay never
  /// closes itself—the handler owns the open state.
  pub fn handle_overlay_dismiss(&self, node: NodeId, handler: Rc<dyn Fn()>) {
    let Some((modal, _)) = self.overlays.borrow().get(&node).copied()
    else {
      return;
    };

    if modal {
      self.with_element(node, |element| {
        {
          let handler = Rc::clone(&handler);

          self
            .events
            .add_direct_listener(node, element, "cancel", move |event| {
              event.prevent_default();
              handler();
            });
        }

        let dialog = element.clone();

        // A backdrop click targets the dialog element itself, but so does a click on
        // the dialog's own padding—only coordinates outside the dialog's box dismiss.
        self
          .events
          .add_direct_listener(node, element, "click", move |event| {
            if event.target().as_ref() != Some(dialog.unchecked_ref()) {
              return;
            }

            let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>()
            else {
              return;
            };

            let rect = dialog.get_bounding_client_rect();
            let x = f64::from(mouse.client_x());
            let y = f64::from(mouse.client_y());

            if x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom() {
              handler();
            }
          });
      });

      return;
    }

    // A popup dismisses from anywhere: Escape and outside presses reach the document
    // root, gated on the popover actually being shown.
    let root = self
      .document
      .document_element()
      .expect("a document element");
    let popup = self.element(node);

    {
      let popup = popup.clone();
      let handler = Rc::clone(&handler);

      self
        .events
        .add_direct_listener(node, &root, "keydown", move |event| {
          if popup.matches(":popover-open").unwrap_or(false)
            && event
              .dyn_ref::<KeyboardEvent>()
              .is_some_and(|event| event.key() == "Escape")
          {
            handler();
          }
        });
    }

    self
      .events
      .add_direct_listener(node, &root, "pointerdown", move |event| {
        let outside = event
          .target()
          .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
          .is_some_and(|target| !popup.contains(Some(&target)));

        if popup.matches(":popover-open").unwrap_or(false) && outside {
          handler();
        }
      });
  }

  /// Moves keyboard focus to `node`.
  pub fn focus_node(&self, node: NodeId) {
    self.with_element(node, |element| {
      if let Some(html) = element.dyn_ref::<HtmlElement>() {
        let _ = html.focus();
      }
    });
  }

  /// Removes keyboard focus from `node`.
  pub fn blur_node(&self, node: NodeId) {
    self.with_element(node, |element| {
      if let Some(html) = element.dyn_ref::<HtmlElement>() {
        let _ = html.blur();
      }
    });
  }

  /// Sets the live value of an input, text area, or select node.
  pub fn set_value(&self, node: NodeId, value: &str) {
    self.with_element(node, |element| {
      if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
        input.set_value(value);
        refresh_slider_fill(input);
      }
      else if let Some(area) = element.dyn_ref::<HtmlTextAreaElement>() {
        area.set_value(value);
      }
      else if let Some(select) = element.dyn_ref::<HtmlSelectElement>() {
        select.set_value(value);
      }
    });
  }

  /// Sets the live checked state of a checkbox or radio node.
  pub fn set_checked(&self, node: NodeId, checked: bool) {
    self.with_element(node, |element| {
      if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
        input.set_checked(checked);
      }
    });
  }

  fn alloc_id(&self) -> NodeId {
    let id = NodeId::new(self.next_id.get());

    self.next_id.set(self.next_id.get() + 1);

    id
  }

  /// Creates a tree-participating node for the given `tag`, adopting the matching
  /// server-rendered element while hydrating and creating a fresh one otherwise.
  fn build_node(&self, tag: &str) -> NodeId {
    let id = self.alloc_id();
    let element = self.element_for(id, tag);

    events::stamp_node_id(&element, id);
    self.nodes.borrow_mut().insert(id, element);

    id
  }

  /// The DOM element for a freshly built node `id`: during hydration the server element whose
  /// marker matches `id` is adopted (and its marker stripped); otherwise a fresh `tag` element
  /// is created. A marker the serialized page lacks builds fresh quietly—a subtree the server
  /// built detached (a suspense fallback) never serializes—while a tag disagreement is a loud
  /// mismatch, built fresh in the divergent node's place.
  fn element_for(&self, id: NodeId, tag: &str) -> Element {
    let candidate = match self.hydration.borrow_mut().as_mut() {
      Some(markers) => Some(markers.remove(&id.value())),
      None => None,
    };

    match candidate {
      Some(Some(element)) if element.tag_name().eq_ignore_ascii_case(tag) => {
        let _ = element.remove_attribute("data-h");

        self.adopted.borrow_mut().insert(id);

        element
      }
      Some(Some(element)) => {
        tracing::error!(
          "hydration mismatch: server rendered <{}> where the client builds <{tag}> (node {})",
          element.tag_name().to_lowercase(),
          id.value()
        );

        self.document.create_element(tag).expect("create element")
      }
      Some(None) => {
        tracing::debug!(
          "no serialized <{tag}> for node {}; building fresh",
          id.value()
        );

        self.document.create_element(tag).expect("create element")
      }
      None => self.document.create_element(tag).expect("create element"),
    }
  }

  /// Indexes the root's server-rendered elements by their hydration marker, so each built node
  /// adopts the server element carrying its id.
  fn seed_hydration(&self) {
    let markers = {
      let nodes = self.nodes.borrow();
      let root = nodes.get(&ROOT_ID).expect("the root is registered");

      root
        .query_selector_all("[data-h]")
        .expect("query the hydration markers")
    };

    let mut map = FxHashMap::default();

    for index in 0..markers.length() {
      let element = markers
        .item(index)
        .expect("a node at a valid index")
        .dyn_into::<Element>()
        .expect("a marked node is an element");

      if let Some(id) = element
        .get_attribute("data-h")
        .and_then(|value| value.parse::<u64>().ok())
      {
        map.insert(id, element);
      }
    }

    *self.hydration.borrow_mut() = Some(map);
  }

  /// Claims a lazy group's server-rendered subtree while hydrating: the markup stays
  /// in place, visible until the real content arrives, and its markers leave the
  /// adoption map so nothing else adopts into it. Whether there was server content to
  /// keep.
  fn claim_lazy(&self, node: NodeId) -> bool {
    let mut hydration = self.hydration.borrow_mut();
    let Some(markers) = hydration.as_mut()
    else {
      return false;
    };

    let element = self.element(node);
    let descendants = element
      .query_selector_all("[data-h]")
      .expect("query the claimed markers");

    for index in 0..descendants.length() {
      if let Some(id) = descendants
        .item(index)
        .and_then(|descendant| descendant.dyn_into::<Element>().ok())
        .and_then(|descendant| descendant.get_attribute("data-h"))
        .and_then(|value| value.parse::<u64>().ok())
      {
        markers.remove(&id);
      }
    }

    element.first_child().is_some()
  }

  /// Removes a claimed lazy group's server-rendered children, as the real content
  /// arrives to take their place.
  fn discard_lazy(&self, node: NodeId) {
    let element = self.element(node);

    while let Some(child) = element.first_child() {
      let _ = element.remove_child(&child);
    }
  }

  /// Ends hydration: later builds create fresh nodes and attach them to the DOM. Server
  /// elements the client never built are a loud mismatch—they linger in the document
  /// unmanaged.
  fn finish_hydration(&self) {
    if let Some(markers) = self.hydration.borrow_mut().take()
      && !markers.is_empty()
    {
      tracing::error!(
        "hydration mismatch: {} server element(s) were never adopted",
        markers.len()
      );
    }

    self.adopted.borrow_mut().clear();
  }

  /// Runs `f` with the node's element, without cloning the handle. `f` must not make
  /// a call that inserts or removes nodes (which would borrow `nodes` again).
  fn with_element<R>(&self, node: NodeId, apply: impl FnOnce(&Element) -> R) -> R {
    let nodes = self.nodes.borrow();
    let element = nodes.get(&node).expect("web platform: unknown node id");

    apply(element)
  }
}

impl State {
  fn create_group(&self) -> NodeId {
    self.build_node("div")
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

    // A node adopted in place already sits under its parent in the server DOM; a freshly built
    // one—a normal build, or a hydration-mismatch fallback—must be inserted.
    if self.adopted.borrow().contains(&child) {
      return;
    }

    let nodes = self.nodes.borrow();
    let parent_el = nodes.get(&parent).expect("web platform: unknown node id");
    let child_el = nodes.get(&child).expect("web platform: unknown node id");

    match before {
      Some(before) => {
        let before_el = nodes.get(&before).expect("web platform: unknown node id");
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
    let parent_el = nodes.get(&parent).expect("web platform: unknown node id");
    let child_el = nodes.get(&child).expect("web platform: unknown node id");
    let _ = parent_el.remove_child(child_el.as_ref());
  }

  fn destroy(&self, node: NodeId) {
    // Tear down the whole detached subtree, releasing each node's element, class, handlers and
    // listener closures. Dropping a closure detaches its DOM listener, so no `removeEventListener`
    // is needed. No user code runs during teardown, so the structural maps are borrowed once up
    // front; the event registries hold their own cells.
    let mut stack = vec![node];

    let mut children = self.children.borrow_mut();
    let mut parents = self.parents.borrow_mut();
    let mut nodes = self.nodes.borrow_mut();
    let mut node_class = self.node_class.borrow_mut();
    let mut styles = self.styles.borrow_mut();
    let mut observers = self.observers.borrow_mut();

    while let Some(current) = stack.pop() {
      if let Some(descendants) = children.remove(&current) {
        stack.extend(descendants);
      }

      parents.remove(&current);
      nodes.remove(&current);

      if let Some(class) = node_class.remove(&current) {
        styles.release(&class);
      }

      if let Some((observer, closure)) = observers.remove(&current) {
        observer.disconnect();
        drop(closure);
      }

      self.events.remove(current);
    }
  }
}

thread_local! {
  static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
  /// The server-rendered resource values to adopt during hydration, in creation order.
  static ADOPTED: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) };
  /// The next resource's creation-order slot, matched against [`ADOPTED`].
  static NEXT_SLOT: Cell<usize> = const { Cell::new(0) };
  /// The server-rendered tag-keyed values to adopt during hydration.
  static TAGGED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Advances to the next resource slot, returning its adopted server value if hydrating.
#[must_use]
pub fn adopt_next() -> Option<String> {
  let slot = NEXT_SLOT.get();

  NEXT_SLOT.set(slot + 1);

  ADOPTED.with(|adopted| adopted.borrow().get(slot).cloned().flatten())
}

/// A no-op on the client: it adopts server state rather than producing it.
#[inline(always)]
pub fn register_source(_source: crate::HydrationSource) {}

/// Takes the adopted server value registered under `tag`, if hydrating left one.
#[must_use]
pub fn adopt_tagged(tag: &str) -> Option<String> {
  TAGGED.with(|tagged| {
    let mut tagged = tagged.borrow_mut();
    let index = tagged.iter().position(|(existing, _)| existing == tag)?;

    Some(tagged.remove(index).1)
  })
}

/// A no-op on the client: it adopts server state rather than producing it.
#[inline(always)]
pub fn register_tagged_source(_tag: &str, _source: crate::HydrationSource) {}

/// The browser is a client, not the server.
#[inline(always)]
#[must_use]
pub fn is_server() -> bool {
  false
}

/// Runs `apply` against the installed platform state.
///
/// Only ever takes a shared borrow, so a click handler may re-enter and build or destroy
/// nodes.
///
/// # Panics
///
/// Panics if the web platform is not installed for this thread.
#[inline(always)]
fn with_state<R>(apply: impl FnOnce(&State) -> R) -> R {
  STATE.with(|state| {
    apply(
      state
        .borrow()
        .as_ref()
        .expect("the web platform to be installed for this thread"),
    )
  })
}

/// Installs the web platform for this thread, rooted at `root`—the element views are
/// inserted under.
///
/// # Panics
///
/// Panics if it is already installed, or if there is no `window`/`document`.
pub fn install(root: Element) {
  skene_reactive::Runtime::init_on_ui_thread();

  STATE.with(|state| {
    let mut state = state.borrow_mut();

    assert!(
      state.is_none(),
      "the web platform is already installed for this thread",
    );

    *state = Some(State::new(root));
  });

  install_deselection();
}

/// Keeps a range input's `--skene-slider-fill` at the value's fraction of its span,
/// painting the filled run on engines without a native progress part.
fn refresh_slider_fill(input: &HtmlInputElement) {
  if input.type_() != "range" {
    return;
  }

  let number = |raw: String, fallback: f32| raw.parse::<f32>().unwrap_or(fallback);
  let min = number(input.min(), 0.0);
  let value = number(input.value(), min);
  let fraction =
    skene_core::SliderRange::new(min, number(input.max(), 100.0), 0.0).fraction_of(value);

  if let Some(html) = input.dyn_ref::<HtmlElement>() {
    let _ = html
      .style()
      .set_property("--skene-slider-fill", &format!("{}%", fraction * 100.0));
  }
}

/// Collapses the document selection when a primary press lands on unselectable
/// content: selection is opt-in, but clearing one is not—a click anywhere outside
/// selectable text deselects, as it would on a plain page. Presses on selectable text
/// keep the browser's own anchoring, and a `Shift`-press keeps its extension.
fn install_deselection() {
  let Some(window) = web_sys::window()
  else {
    return;
  };
  let Some(document) = window.document()
  else {
    return;
  };
  let closure = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::PointerEvent)>::new(
    move |event: web_sys::PointerEvent| {
      if event.button() != 0 || event.shift_key() {
        return;
      }

      let Some(window) = web_sys::window()
      else {
        return;
      };
      let Ok(Some(selection)) = window.get_selection()
      else {
        return;
      };

      if selection.is_collapsed() {
        return;
      }

      let Some(target) = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
      else {
        return;
      };
      let selectable = window
        .get_computed_style(&target)
        .ok()
        .flatten()
        .and_then(|style| style.get_property_value("user-select").ok())
        .is_some_and(|value| value != "none");

      if !selectable {
        let _ = selection.remove_all_ranges();
      }
    },
  );

  let _ =
    document.add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref());

  closure.forget();
}

/// Uninstalls the web platform and resets the reactive runtime and root scope.
///
/// Lets one thread run many independent mounts, as a test suite does.
#[cfg(feature = "test")]
pub fn uninstall() {
  skene_reactive::Runtime::reset();
  skene_core::reset_root_owner();

  STATE.with(|state| *state.borrow_mut() = None);
}

/// The owning [`Document`], for DOM work beyond the other functions (creating text nodes,
/// running arbitrary DOM APIs).
#[must_use]
pub fn document() -> Document {
  with_state(|state| state.document.clone())
}

/// Creates a DOM element with the given `tag` as a detached, tree-participating node.
#[must_use]
pub fn create_element(tag: &str) -> NodeId {
  with_state(|state| state.create_element(tag))
}

/// The DOM element backing `node`, to manipulate directly: set attributes, append children,
/// or hand it to a JS library.
///
/// The raw-handle escape hatch, specific to this platform—not part of the shared
/// vocabulary, so a portable view reaches for [`set_attribute`] instead.
#[must_use]
pub fn element(node: NodeId) -> Element {
  with_state(|state| state.element(node))
}

/// Claims a lazy view's server-rendered subtree while hydrating: the markup stays in
/// place, visible until the real content arrives, and its markers leave the adoption
/// map. `false` when not hydrating, or when the server rendered nothing there.
#[must_use]
pub fn claim_lazy(node: NodeId) -> bool {
  with_state(|state| state.claim_lazy(node))
}

/// Removes a claimed lazy view's server-rendered children, as the real content
/// arrives to take their place.
pub fn discard_lazy(node: NodeId) {
  with_state(|state| state.discard_lazy(node));
}

/// Sets `node`'s `name` attribute to `value`.
pub fn set_attribute(node: NodeId, name: &str, value: &str) {
  with_state(|state| {
    let _ = state.element(node).set_attribute(name, value);
  });
}

/// Interns `declarations` as a shared, content-addressed class and assigns it to `node`.
pub fn set_style(node: NodeId, declarations: &Declarations) {
  with_state(|state| {
    state.assign_class(node, &crate::mapping::declarations_template(declarations));
  });
}

/// Interns a full rule template as a shared, content-addressed class and assigns it to
/// `node`.
pub(crate) fn assign_class(node: NodeId, template: &str) {
  with_state(|state| state.assign_class(node, template));
}

/// Applies each of `declarations` as an inline override on `node`, winning over its
/// interned class.
pub fn override_style(node: NodeId, declarations: &Declarations) {
  with_state(|state| state.override_style(node, declarations));
}

/// Sets a node's text content.
pub fn set_text(node: NodeId, text: &str) {
  with_state(|state| state.set_text(node, text));
}

/// Registers `handler` to run when `node` is clicked (or a descendant's click bubbles to
/// it), until the node is destroyed.
pub fn handle_click(node: NodeId, handler: Rc<skene_core::ClickHandler>) {
  with_state(|state| state.handle_click(node, handler));
}

/// Registers `handler` to run when the pointer enters `node`, until the node is destroyed.
pub fn handle_pointer_enter(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_pointer_enter(node, handler));
}

/// Registers `handler` to run when the pointer leaves `node`, until the node is destroyed.
pub fn handle_pointer_leave(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_pointer_leave(node, handler));
}

/// Registers `handler` to run when `node` gains focus, until the node is destroyed.
pub fn handle_focus(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_focus(node, handler));
}

/// Registers `handler` to run when `node` loses focus, until the node is destroyed.
pub fn handle_blur(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_blur(node, handler));
}

/// Registers `handler` to run when a key is pressed on `node` or within it, until the
/// node is destroyed.
pub fn handle_key_down(node: NodeId, handler: Rc<skene_core::KeyHandler>) {
  with_state(|state| state.handle_key_down(node, handler));
}

/// Registers `handler` to run with the new value when text is entered on `node` or an
/// Registers a delegated committed-change handler on `node`; see
/// [`State::handle_change`].
pub fn handle_change(node: NodeId, handler: Rc<dyn Fn(&str)>) {
  with_state(|state| state.handle_change(node, handler));
}

/// Registers a delegated pointer-down handler on `node`; see
/// [`State::handle_pointer_down`].
pub fn handle_pointer_down(node: NodeId, handler: Rc<skene_core::PointerHandler>) {
  with_state(|state| state.handle_pointer_down(node, handler));
}

/// Registers a delegated pointer-up handler on `node`; see
/// [`State::handle_pointer_up`].
pub fn handle_pointer_up(node: NodeId, handler: Rc<skene_core::PointerHandler>) {
  with_state(|state| state.handle_pointer_up(node, handler));
}

/// Registers a delegated pointer-move handler on `node`; see
/// [`State::handle_pointer_move`].
pub fn handle_pointer_move(node: NodeId, handler: Rc<skene_core::PointerHandler>) {
  with_state(|state| state.handle_pointer_move(node, handler));
}

/// Registers a scroll handler on `node`; see [`State::handle_scroll`].
pub fn handle_scroll(node: NodeId, handler: Rc<dyn Fn(&ScrollEvent)>) {
  with_state(|state| state.handle_scroll(node, handler));
}

/// Watches a node's content size; see [`State::observe_size`].
pub fn observe_size(node: NodeId, handler: Rc<dyn Fn(f32, f32)>) {
  with_state(|state| state.observe_size(node, handler));
}

/// Moves keyboard focus to `node`.
pub fn focus_node(node: NodeId) {
  with_state(|state| state.focus_node(node));
}

/// Removes keyboard focus from `node`.
pub fn blur_node(node: NodeId) {
  with_state(|state| state.blur_node(node));
}

/// Registers a delegated toggle handler on `node`; see [`State::handle_toggle`].
pub fn handle_toggle(node: NodeId, handler: Rc<dyn Fn(bool)>) {
  with_state(|state| state.handle_toggle(node, handler));
}

/// Sets the live checked state of a checkbox or radio node.
pub fn set_checked(node: NodeId, checked: bool) {
  with_state(|state| state.set_checked(node, checked));
}

/// Sets the document title.
pub fn set_document_title(title: &str) {
  if let Some(document) = web_sys::window().and_then(|window| window.document()) {
    document.set_title(title);
  }
}

/// Adds an element to the document head, unless an identical one is already present
/// (as after hydrating a server render that registered the same element).
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  let Some(head) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.head())
  else {
    return;
  };

  let children = head.children();

  for index in 0..children.length() {
    let Some(existing) = children.item(index)
    else {
      continue;
    };

    let matches = existing.tag_name().eq_ignore_ascii_case(tag)
      && attributes
        .iter()
        .all(|(name, value)| existing.get_attribute(name).as_deref() == Some(*value));

    if matches {
      return;
    }
  }

  let Some(document) = web_sys::window().and_then(|window| window.document())
  else {
    return;
  };

  let Ok(element) = document.create_element(tag)
  else {
    return;
  };

  for (name, value) in attributes {
    let _ = element.set_attribute(name, value);
  }

  let _ = head.append_child(&element);
}

/// Sets the live value of an input node.
pub fn set_value(node: NodeId, value: &str) {
  with_state(|state| state.set_value(node, value));
}

/// Builds `view` under the installed root, returning its node.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_owner().enter(|| {
    let node = view();

    with_state(|state| state.attach_child(ROOT_ID, node, None));

    node
  })
}

/// Hydrates a server-rendered tree under `root`, adopting its existing elements and the `state`
/// serialized alongside them.
///
/// Builds `app` but, rather than creating DOM nodes, takes over the server-rendered ones in
/// document order—wiring event listeners and reactive bindings onto them—so the markup is
/// reused in place, with no rebuild and no flash. Critical resources adopt their server value
/// from `state` (as produced by `render_to_string`) instead of loading again. The build must
/// produce the same structure the server did.
///
/// # Panics
///
/// Panics if the platform is already installed for this thread.
pub fn hydrate<V, F>(root: Element, state: &str, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_reactive::Runtime::init_on_ui_thread();

  STATE.with(|state| {
    let mut state = state.borrow_mut();

    assert!(
      state.is_none(),
      "the web platform is already installed for this thread",
    );

    *state = Some(State::new(root));
  });

  NEXT_SLOT.set(0);
  let (ordered, tagged) = crate::hydration::decode_state(state);

  ADOPTED.with(|adopted| *adopted.borrow_mut() = ordered);
  TAGGED.with(|adopted| *adopted.borrow_mut() = tagged);

  with_state(State::seed_hydration);

  mount(|| app().into_view().build());

  with_state(State::finish_hydration);
}

/// Hydrates the server-rendered page: adopts `<body>` and reads the state script embedded by
/// the server's page assembly, so the client takes over without a rebuild.
///
/// # Panics
///
/// Panics if there is no `window`, `document`, or `<body>` (i.e. not running in a browser).
pub fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(logging::ConsoleSubscriber);

  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");

  let body = document.body().expect("a body");
  let state = document
    .get_element_by_id(crate::STATE_SCRIPT_ID)
    .and_then(|script| script.text_content())
    .unwrap_or_default();

  hydrate(body.into(), &state, app);
}

/// Mounts and runs a skene application in the browser.
///
/// Installs the web platform rooted at `<body>` and builds `app` under it. The reactive
/// runtime, event listeners and view tree are kept alive for the lifetime of the page.
///
/// A server-rendered page—recognized by its embedded state script—is adopted in place
/// instead of rebuilt, as [`hydrate_page`] does: a statically generated page carries
/// its content as plain markup and this entry enhances it.
///
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>` (i.e. not running in a browser).
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let document = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document");

  if document.get_element_by_id(crate::STATE_SCRIPT_ID).is_some() {
    hydrate_page(app);

    return;
  }

  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(logging::ConsoleSubscriber);

  let body = document.body().expect("a body");

  install(body.into());
  mount(|| app().into_view().build());
}

/// The portable payload of a DOM pointer event, with local coordinates relative to the
/// handling `element`.
fn pointer_payload(pointer: &web_sys::PointerEvent, element: &Element) -> PointerEvent {
  let rect = element.get_bounding_client_rect();

  #[expect(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
  PointerEvent {
    x: pointer.client_x() as f32,
    y: pointer.client_y() as f32,
    local_x: pointer.client_x() as f32 - rect.left() as f32,
    local_y: pointer.client_y() as f32 - rect.top() as f32,
    button: match pointer.button() {
      0 => Some(PointerButton::Primary),
      1 => Some(PointerButton::Auxiliary),
      2 => Some(PointerButton::Secondary),
      _ => None,
    },
    modifiers: Modifiers {
      ctrl: pointer.ctrl_key(),
      shift: pointer.shift_key(),
      alt: pointer.alt_key(),
      meta: pointer.meta_key(),
    },
  }
}

/// The current value of the control an event targets, whichever control kind it is.
fn element_value(event: &web_sys::Event) -> Option<String> {
  let target = event.target()?;

  if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
    return Some(input.value());
  }

  if let Some(area) = target.dyn_ref::<HtmlTextAreaElement>() {
    return Some(area.value());
  }

  if let Some(select) = target.dyn_ref::<HtmlSelectElement>() {
    return Some(select.value());
  }

  None
}

/// Positions an open popup against its anchor: below it for `Bottom`, above it for
/// `Top`, start-aligned either way; other placements keep their viewport position.
fn position_at_anchor(popup: &HtmlElement, anchor: &Element, placement: Placement) {
  let rect = anchor.get_bounding_client_rect();
  let style = popup.style();
  let top = match placement {
    Placement::Top => rect.top() - f64::from(popup.offset_height()),
    Placement::Bottom => rect.bottom(),
    _ => return,
  };

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");
  let _ = style.set_property("left", &format!("{}px", rect.left()));
  let _ = style.set_property("top", &format!("{top}px"));
}

/// Records an overlay's realization config; see [`State::register_overlay`].
pub fn register_overlay(node: NodeId, modal: bool, placement: Placement) {
  with_state(|state| state.register_overlay(node, modal, placement));
}

/// Opens or closes an overlay; see [`State::set_overlay_open`].
pub fn set_overlay_open(node: NodeId, open: bool, anchor: Option<NodeId>) {
  with_state(|state| state.set_overlay_open(node, open, anchor));
}

/// Registers an overlay dismiss handler; see [`State::handle_overlay_dismiss`].
pub fn handle_overlay_dismiss(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_overlay_dismiss(node, handler));
}
