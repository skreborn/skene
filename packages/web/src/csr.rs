//! The web platform: a browser DOM.
//!
//! Creates elements, styles them through a shared stylesheet, sets text and wires click
//! listeners — what the built-in views realize against, and the escape hatch a low-level
//! web view calls directly. Layout, painting, text shaping and hit-testing are the
//! browser's. Mount with [`launch`].

pub(crate) mod tree;

mod logging;
mod stylesheet;
mod task;

use std::{
  cell::{Cell, RefCell},
  collections::hash_map::Entry,
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet};
use skene_core::{IntoView, NodeId, View};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, HtmlElement};

use self::stylesheet::StyleStore;
pub use self::task::drive;
// The CSS declaration set is web-family shared; this leaf takes it in `set_style`.
use crate::Declarations;

const ROOT_ID: NodeId = NodeId::new(0);

/// A live DOM click-listener callback, kept alive for the platform's lifetime.
type ClickClosure = Closure<dyn FnMut(web_sys::Event)>;

/// The click handlers registered for each node. Shared (`Rc`) with the listener
/// closures so they can run without borrowing the state.
type ClickHandlers = Rc<RefCell<FxHashMap<NodeId, Vec<Rc<dyn Fn()>>>>>;

/// The web platform's state: the element registry, the stylesheet store, and the live
/// event-listener closures. Private — reached only through this module's functions.
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
  /// The click handlers registered for each node.
  click_handlers: ClickHandlers,
  /// One live click listener per node, kept alive until the node is destroyed. Dropping
  /// it detaches the DOM listener.
  click_listeners: RefCell<FxHashMap<NodeId, ClickClosure>>,
  /// While hydrating, the server-rendered elements indexed by their creation-order marker;
  /// `None` in normal operation, when nodes are created fresh.
  hydration: RefCell<Option<FxHashMap<u64, Element>>>,
  /// The nodes adopted in place during hydration, so their attach skips the DOM insert; cleared
  /// when hydration ends.
  adopted: RefCell<FxHashSet<NodeId>>,
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
      click_handlers: Rc::new(RefCell::new(FxHashMap::default())),
      click_listeners: RefCell::new(FxHashMap::default()),
      hydration: RefCell::new(None),
      adopted: RefCell::new(FxHashSet::default()),
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

  /// Interns `declarations` as a shared, content-addressed class and assigns it to `node`,
  /// refcounting rules so unused ones are collected. The batched path — apply a node's full
  /// style once. It stays SSR-compatible (the class is content-addressed).
  pub fn set_style(&self, node: NodeId, declarations: &Declarations) {
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

  /// Applies each of `declarations` as an inline override on `node`, winning over its
  /// interned class — the path for a reactive change, so an animating property never
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

  /// Registers `handler` to run when `node` is clicked, until the node is destroyed. A
  /// single DOM listener per node runs every registered handler.
  pub fn handle_click(&self, node: NodeId, handler: Rc<dyn Fn()>) {
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
    // may re-enter the platform (e.g. to build or destroy nodes).
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

    self.nodes.borrow_mut().insert(id, element);

    id
  }

  /// The DOM element for a freshly built node `id`: during hydration the server element whose
  /// marker matches `id` is adopted (and its marker stripped); otherwise a fresh `tag` element
  /// is created. A hydration lookup that misses, or finds a different tag, is a mismatch — it is
  /// logged and a fresh element is created in the divergent node's place.
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
      Some(_) => {
        tracing::error!(
          "hydration mismatch: no server <{tag}> for node {}",
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

  /// Ends hydration: later builds create fresh nodes and attach them to the DOM.
  fn finish_hydration(&self) {
    *self.hydration.borrow_mut() = None;
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
    // one — a normal build, or a hydration-mismatch fallback — must be inserted.
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
    // is needed. No user code runs during teardown, so every map is borrowed once up front.
    let mut stack = vec![node];

    let mut children = self.children.borrow_mut();
    let mut parents = self.parents.borrow_mut();
    let mut nodes = self.nodes.borrow_mut();
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

      if let Some(class) = node_class.remove(&current) {
        styles.release(&class);
      }

      click_handlers.remove(&current);
      click_listeners.remove(&current);
    }
  }
}

thread_local! {
  static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
  /// The server-rendered resource values to adopt during hydration, in creation order.
  static ADOPTED: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) };
  /// The next resource's creation-order slot, matched against [`ADOPTED`].
  static NEXT_SLOT: Cell<usize> = const { Cell::new(0) };
}

/// Advances to the next resource slot, returning its adopted server value if hydrating.
pub fn adopt_next() -> Option<String> {
  let slot = NEXT_SLOT.get();

  NEXT_SLOT.set(slot + 1);

  ADOPTED.with(|adopted| adopted.borrow().get(slot).cloned().flatten())
}

/// A no-op on the client: it adopts server state rather than producing it.
#[inline(always)]
pub fn register_source(_source: crate::HydrationSource) {}

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

/// Installs the web platform for this thread, rooted at `root` — the element views are
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
}

/// Uninstalls the web platform and resets the reactive runtime and root scope.
///
/// Lets one thread run many independent mounts, as a test suite does.
#[cfg(feature = "test")]
pub fn uninstall() {
  skene_reactive::Runtime::reset();
  skene_core::reset_root_scope();

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
/// The raw-handle escape hatch, specific to this platform — not part of the shared
/// vocabulary, so a portable view reaches for [`set_attribute`] instead.
#[must_use]
pub fn element(node: NodeId) -> Element {
  with_state(|state| state.element(node))
}

/// Sets `node`'s `name` attribute to `value`.
pub fn set_attribute(node: NodeId, name: &str, value: &str) {
  with_state(|state| {
    let _ = state.element(node).set_attribute(name, value);
  });
}

/// Interns `declarations` as a shared, content-addressed class and assigns it to `node`.
pub fn set_style(node: NodeId, declarations: &Declarations) {
  with_state(|state| state.set_style(node, declarations));
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

/// Registers `handler` to run when `node` is clicked, until the node is destroyed.
pub fn handle_click(node: NodeId, handler: Rc<dyn Fn()>) {
  with_state(|state| state.handle_click(node, handler));
}

/// Builds `view` under the installed root, returning its node.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_scope().enter(|| {
    let node = view();

    with_state(|state| state.attach_child(ROOT_ID, node, None));

    node
  })
}

/// Hydrates a server-rendered tree under `root`, adopting its existing elements and the `state`
/// serialized alongside them.
///
/// Builds `app` but, rather than creating DOM nodes, takes over the server-rendered ones in
/// document order — wiring event listeners and reactive bindings onto them — so the markup is
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
  ADOPTED.with(|adopted| *adopted.borrow_mut() = crate::hydration::decode_state(state));

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
/// # Panics
///
/// Panics if there is no `window`, `document` or `<body>` (i.e. not running in a browser).
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  console_error_panic_hook::set_once();

  let _ = tracing::subscriber::set_global_default(logging::ConsoleSubscriber);

  let body = web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
    .body()
    .expect("a body");

  install(body.into());
  mount(|| app().into_view().build());
}
