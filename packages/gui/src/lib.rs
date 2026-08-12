//! # skene
//!
//! The single dependency an app pulls in. Holds the built-in views, realized once against
//! the active driver's typed vocabulary, the composition helpers, the transparent entry
//! point, and re-exports the spine + reactive engine. App code names no driver: write a
//! view function, mark it [`macro@main`], and it runs on whatever driver the build
//! selects.

pub mod prelude;

mod container;
mod containers;
mod control;
#[cfg(target_arch = "wasm32")]
mod deferred;
mod focus;
#[cfg(target_arch = "wasm32")]
pub mod head;
#[cfg(target_arch = "wasm32")]
mod lazy;
mod lazy_column;
#[cfg(target_arch = "wasm32")]
mod link;
mod overlay;
mod raw_button;
mod raw_checkbox;
mod raw_picker;
mod raw_radio;
mod raw_slider;
mod raw_text_editor;
mod raw_text_field;
mod realize;
#[cfg(target_arch = "wasm32")]
mod resource;
#[cfg(target_arch = "wasm32")]
mod routing;
mod simple;
mod simple_button;
mod simple_card;
mod simple_checkbox;
mod simple_picker;
mod simple_radio;
mod simple_slider;
mod simple_text_editor;
mod simple_text_field;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod split;
#[cfg(target_arch = "wasm32")]
mod task;
mod text;

pub use skene_core::{self as core, *};
pub use skene_macros::{lazy, main};
pub use skene_reactive::{self as reactive};
// The active driver, reached by name from the shared realization: the web on wasm
// targets, the recording driver on the host, for headless view tests.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use skene_record as driver;
/// The recording driver's test seams (`trigger_` events, tree inspection), for
/// headless view tests on the host.
#[cfg(not(target_arch = "wasm32"))]
pub use skene_record as record;
pub use skene_router as router;
#[cfg(target_arch = "wasm32")]
pub(crate) use skene_web as driver;
/// The web driver, for a library author writing a view directly against the browser DOM.
///
/// These are the only web-specific names the facade exposes, and they are deliberately kept
/// out of [`prelude`] so portable app code never names a driver.
#[cfg(target_arch = "wasm32")]
pub use skene_web as web;
pub use tracing;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_bindgen;

#[cfg(target_arch = "wasm32")]
pub use crate::deferred::Deferred;
#[cfg(target_arch = "wasm32")]
pub use crate::lazy::Lazy;
#[cfg(target_arch = "wasm32")]
pub use crate::link::Link;
#[cfg(target_arch = "wasm32")]
pub use crate::resource::Resource;
#[cfg(target_arch = "wasm32")]
pub use crate::routing::{install_router, install_static_router};
#[cfg(target_arch = "wasm32")]
pub use crate::task::spawn;
pub use crate::{
  container::Container,
  containers::{Column, Flexible, Grid, GridCell, Pinned, Row, Stack},
  focus::FocusHandle,
  lazy_column::LazyColumn,
  overlay::{Anchor, Overlay},
  raw_button::RawButton,
  raw_checkbox::RawCheckbox,
  raw_picker::RawPicker,
  raw_radio::RawRadio,
  raw_slider::RawSlider,
  raw_text_editor::RawTextEditor,
  raw_text_field::RawTextField,
  simple::{SimplePalette, SimpleVariant},
  simple_button::SimpleButton,
  simple_card::SimpleCard,
  simple_checkbox::SimpleCheckbox,
  simple_picker::SimplePicker,
  simple_radio::SimpleRadio,
  simple_slider::SimpleSlider,
  simple_text_editor::SimpleTextEditor,
  simple_text_field::SimpleTextField,
  text::{RichText, Span, Text, TextStyle, TextTheme},
};

/// The active driver's tree mutations, instantiating the spine's shared structural
/// machinery. App code sees it only through the [`Children`] and [`DerivedView`]
/// aliases.
#[derive(Clone, Copy, Default)]
pub struct GuiOps;

impl skene_core::TreeOps for GuiOps {
  #[inline(always)]
  fn create_group(self) -> NodeId {
    driver::create_group()
  }

  #[inline(always)]
  fn attach(self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    driver::attach(parent, child, before);
  }

  #[inline(always)]
  fn detach(self, parent: NodeId, child: NodeId) {
    driver::detach(parent, child);
  }

  #[inline(always)]
  fn destroy(self, node: NodeId) {
    driver::destroy(node);
  }
}

/// A container's children.
pub type Children = skene_core::Children<GuiOps>;
/// A suspense view over the active driver's tree; see [`skene_core::Suspense`].
pub type Suspense = skene_core::Suspense<GuiOps>;
/// An error boundary over the active driver's tree; see [`skene_core::ErrorBoundary`].
pub type ErrorBoundary = skene_core::ErrorBoundary<GuiOps>;
/// A reactive subtree.
pub type DerivedView = skene_core::DerivedView<GuiOps>;

/// Reactively reconciles a parent's children from a keyed collection; see
/// [`skene_core::reconcile`].
#[inline(always)]
pub fn reconcile<T, I, K>(
  parent: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + std::hash::Hash + 'static,
{
  skene_core::reconcile::<GuiOps, T, I, K>(parent, items, key, build);
}

/// Anchors a reactive subtree under a fresh group node, returning that node; see
/// [`skene_core::derived`].
#[inline(always)]
pub fn derived(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::derived::<GuiOps>(build)
}

/// Anchors a transitional reactive subtree, each rebuild swapping in once it settles;
/// see [`skene_core::transition`].
#[inline(always)]
pub fn transition(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::transition::<GuiOps>(build)
}

/// A view that renders nothing.
#[inline(always)]
#[must_use]
pub fn empty() -> impl View {
  skene_core::empty::<GuiOps>()
}

/// Spacing on the four edges of a box, in logical pixels.
pub type Inset = skene_core::Inset<f32>;
/// A 2D size, in logical pixels.
pub type Size = skene_core::Size<f32>;
/// A point in the logical-pixel coordinate space.
pub type Point = skene_core::Point<f32>;
/// A positioned, sized rectangle, in logical pixels.
pub type Rect = skene_core::Rect<f32>;

/// Wraps a view that only a client can realize, so it renders on an interactive platform and
/// falls back to [`empty`] where it cannot run.
///
/// The place for a low-level view that reaches a platform's raw handles—a `<canvas>` driven
/// through `web::element`, a JS charting library—which have no meaning off the client. On a
/// server-rendering platform this yields nothing; the real content arrives when the client
/// hydrates. `build` returns the node it realizes, exactly as a view's `build` does.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[inline(always)]
pub fn client_only(build: impl FnOnce() -> skene_core::NodeId + 'static) -> impl View {
  skene_core::AnyView::new(build)
}

/// On a server-rendering platform this yields nothing; the real content arrives when the
/// client hydrates.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[inline(always)]
pub fn client_only(_build: impl FnOnce() -> skene_core::NodeId + 'static) -> impl View {
  empty()
}

/// Mounts and runs the application in the browser.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[inline(always)]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  #[cfg(feature = "snapshot")]
  driver::boot_with_dev_state(move || {
    driver::launch(app);

    tracing::info!("application mounted");
  });

  #[cfg(not(feature = "snapshot"))]
  {
    driver::launch(app);

    tracing::info!("application mounted");
  }
}

/// Hydrates a server-rendered page: adopts the document body and reads the embedded state, so
/// the client takes over the markup `render_to_string` produced without a rebuild.
///
/// The client entry point for a server-rendered app, the counterpart to [`launch`] for a
/// fresh client render.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[inline(always)]
pub fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  #[cfg(feature = "snapshot")]
  driver::boot_with_dev_state(move || {
    driver::hydrate_page(app);

    tracing::info!("application hydrated");
  });

  #[cfg(not(feature = "snapshot"))]
  {
    driver::hydrate_page(app);

    tracing::info!("application hydrated");
  }
}

/// A server render, in parts: the stylesheet, the body markup, and the hydration state.
///
/// Assemble a full document with [`page`](Self::page), or place the parts yourself—the
/// [`styles`](Self::styles) in `<head>`, the [`body`](Self::body) as the app root, and the
/// [`state`](Self::state) wherever the client's `hydrate_page` can read it back. The client boot
/// pairs with `page`; a custom placement pairs with the lower-level `web::hydrate`.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[must_use]
pub struct Rendered {
  styles: String,
  head: String,
  body: String,
  state: String,
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
impl Rendered {
  /// The `<style>` block: the reset plus the interned rules.
  #[must_use]
  #[inline(always)]
  pub fn styles(&self) -> &str {
    &self.styles
  }

  /// The registered head content: the title and the elements added through
  /// [`head`](crate::head).
  #[must_use]
  #[inline(always)]
  pub fn head(&self) -> &str {
    &self.head
  }

  /// The settled view tree—the page body.
  #[must_use]
  #[inline(always)]
  pub fn body(&self) -> &str {
    &self.body
  }

  /// The encoded resource state, or `[]` when there is none.
  #[must_use]
  #[inline(always)]
  pub fn state(&self) -> &str {
    &self.state
  }

  /// Assembles a complete HTML document: the styles in `<head>`, the tree in `<body>`, and the
  /// state in a `<script>` the client's `hydrate_page` reads back.
  #[must_use]
  pub fn page(&self) -> String {
    format!(
      "<!DOCTYPE html><html><head>{head}{styles}</head><body>{body}<script id=\"{id}\" \
       type=\"application/json\">{state}</script></body></html>",
      head = self.head,
      styles = self.styles,
      body = self.body,
      id = driver::STATE_SCRIPT_ID,
      // `<` keeps a value's `<` from closing the script early, and is valid JSON that
      // decodes back to `<`.
      state = self.state.replace('<', "\\u003c"),
    )
  }
}

/// Renders the application, for a server-rendering platform.
///
/// Builds the tree, drives its resources until they settle, and serializes the result plus the
/// resource state for hydration. The render is self-contained—state is set up and torn down
/// per call—so a runner can serve many requests from one instance.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[inline(always)]
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let (styles, head, body, state) = driver::render_to_string(app);

  Rendered {
    styles,
    head,
    body,
    state,
  }
}

/// Renders the application for the given request path: the router (when installed via
/// [`install_router`]) resolves against `path`, and the render proceeds as
/// [`render_to_string`].
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_to_string_at<V, F>(path: &str, app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  driver::set_request_path(path);

  render_to_string(app)
}

/// The server entry `#[main]` expands to: renders the path given as the first argument
/// (default `/`) and writes the complete page to stdout—or, given `--routes`, writes
/// the statically-renderable routes one per line (see
/// [`install_static_router`]), or, given `--stream <path>`, writes the page flush by
/// flush, each one followed through to stdout as it is produced.
///
/// This makes the app's server build a plain command—`wasmtime app.wasm /some/path`
/// prints the document—which is exactly the interface the runner embeds.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_main<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let argument = std::env::args().nth(1).unwrap_or_else(|| "/".to_owned());

  if argument == "--routes" {
    let _ = render_to_string_at("/", app);

    for route in driver::recorded_static_routes().unwrap_or_else(|| vec!["/".to_owned()]) {
      println!("{route}");
    }

    return;
  }

  if argument == "--stream" {
    let path = std::env::args().nth(2).unwrap_or_else(|| "/".to_owned());

    driver::set_request_path(&path);
    render_page_stream(app, |piece| {
      use std::io::Write;

      print!("{piece}");

      let _ = std::io::stdout().flush();
    });

    return;
  }

  let rendered = render_to_string_at(&argument, app);

  print!("{}", rendered.page());
}

/// A server render as an ordered sequence of HTML chunks, ready to flush one after another.
///
/// Each chunk is written to the response as it is produced: the first opens the document and
/// carries the shell, and each subsequent one carries a [`Deferred`] boundary's content, streamed
/// once it settled. A client with no JavaScript receives a correct document progressively, since
/// the chunks concatenate—in order—into the same markup [`render_to_string`] would produce.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[must_use]
pub struct Stream {
  chunks: Vec<String>,
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
impl Stream {
  /// The chunks, in flush order.
  #[must_use]
  #[inline(always)]
  pub fn chunks(&self) -> &[String] {
    &self.chunks
  }

  /// Consumes the stream into its chunks, in flush order.
  #[must_use]
  #[inline(always)]
  pub fn into_chunks(self) -> Vec<String> {
    self.chunks
  }

  /// The whole document, the chunks concatenated in order—what a no-JavaScript client ends up
  /// with once every chunk has arrived.
  #[must_use]
  #[inline(always)]
  pub fn document(&self) -> String {
    self.chunks.concat()
  }
}

/// Renders the application as an in-order HTML stream, for a server-rendering platform.
///
/// Like [`render_to_string`], but flushes the shell before every [`Deferred`] boundary's slow
/// content: the returned [`Stream`]'s chunks are written to the response as they are produced, so
/// the client sees the page progressively without waiting on the slowest data—and, because the
/// chunks are emitted in document order, with no JavaScript required.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_to_stream<V, F>(app: F) -> Stream
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let (styles, head, mut chunks, state) = driver::render_to_stream(app);
  let head = format!("<!DOCTYPE html><html><head>{head}{styles}</head><body>");
  let tail = format!(
    "<script id=\"{id}\" type=\"application/json\">{state}</script></body></html>",
    id = driver::STATE_SCRIPT_ID,
    // `<` keeps a value's `<` from closing the script early, and is valid JSON that decodes
    // back to `<`.
    state = state.replace('<', "\\u003c"),
  );

  if let Some(first) = chunks.first_mut() {
    first.insert_str(0, &head);
  }

  if let Some(last) = chunks.last_mut() {
    last.push_str(&tail);
  }

  Stream { chunks }
}

/// Renders the application as a progressive page, each flush handed to `sink` as it is
/// produced: the opening shell, a body chunk per settled [`Deferred`] boundary, and the
/// closing tail. The flushes concatenate to [`render_to_stream`]'s
/// [`document`](Stream::document), so writing each one to a response streams the page.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_page_stream<V, F>(app: F, mut sink: impl FnMut(&str))
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  driver::render_to_stream_with(app, |piece| match piece {
    driver::StreamPiece::Shell { styles, head } => {
      sink(&format!(
        "<!DOCTYPE html><html><head>{head}{styles}</head><body>"
      ));
    }
    driver::StreamPiece::Chunk(chunk) => sink(&chunk),
    driver::StreamPiece::Tail(state) => {
      sink(&format!(
        "<script id=\"{id}\" type=\"application/json\">{state}</script></body></html>",
        id = driver::STATE_SCRIPT_ID,
        // `<` keeps a value's `<` from closing the script early, and is valid JSON that decodes
        // back to `<`.
        state = state.replace('<', "\\u003c"),
      ));
    }
  });
}
