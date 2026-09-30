//! The server leaf's realization of the platform: a string tree serialized to HTML with
//! the interned stylesheet, the head and the hydration state, whole or as an in-order
//! stream around deferred boundaries.
//!
//! Every element carries a `data-h` marker counting elements in creation order, which
//! the browser leaf adopts by; an anchor is a comment pair (`<!--s:N-->`, `<!--e:N-->`)
//! counting segments the same way; a scope is a `display: contents` element. Handlers are inert,
//! focus and sizes are the client's, and an overlay serializes closed. A streamed chunk opens with
//! a `<style>` delta holding the registrations and rules interned since the previous flush.

mod head;
mod media;
mod platform;
mod task;
mod tree;

use std::cell::{Cell, RefCell};

use skene_core::{Boundary, IntoView, NodeId, NodeKind, NodeMap, Scene, View};
use skene_reactive::{Owner, batch, provide_context};

pub use self::{
  head::{
    add_head_element,
    add_head_title,
    document_closing,
    document_opening,
    enable_view_transitions,
    remove_head_entry,
    set_document_language,
    set_head_title,
    set_scheme,
    stored_scheme,
  },
  media::set_render_hints,
  task::{drive, install_executor, set_render_deadline, sleep},
};
use self::{
  media::MediaState,
  tree::{Frame, StringTree, Tree},
};
use super::Web;
use crate::{
  Document,
  RenderHints,
  document::state_script,
  realize::{Realized, Realizer},
};

/// The element of the string tree standing for the page body, which the root realizes.
const ROOT: u32 = 0;

/// A probe for whether a deferred boundary's content has settled.
type BoundarySettled = Box<dyn Fn() -> bool>;

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
