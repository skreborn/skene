//! # skene-gui
//!
//! The GUI facade: a GUI app's single dependency. It fixes the scalar (logical pixels,
//! `f32`), names the driver the build selects ([`ActiveDriver`]: the web on `wasm32`,
//! the headless host driver elsewhere) and aliases every built-in view and every
//! `Simple*` view to it, so app code names no platform. The medium's own contribution
//! is [`Typography`], the [`media`] module and the entry points: `launch` in the
//! browser, `render_to_string`, `render_to_stream` and `render_main` on the server
//! leaf, and [`macro@main`] expanding to whichever the target has.

pub mod media;
pub mod prelude;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod split;

#[cfg(target_arch = "wasm32")]
mod deferred;
#[cfg(target_arch = "wasm32")]
pub mod head;
#[cfg(target_arch = "wasm32")]
mod lazy;
#[cfg(target_arch = "wasm32")]
mod link;
#[cfg(target_arch = "wasm32")]
mod resource;
#[cfg(target_arch = "wasm32")]
mod routing;
#[cfg(target_arch = "wasm32")]
mod task;
mod typography_set;

pub use skene_core as core;
pub use skene_core::{
  AnyKind,
  Bubbling,
  Children,
  Color,
  ColorScheme,
  ContainerName,
  ControlKind,
  ControlState,
  Definitions,
  Errors,
  Expr,
  FocusOrder,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  KeyEvent,
  Leaf,
  Length,
  Modifiers,
  NoChild,
  NodeId,
  Number,
  Part,
  PixelExt,
  PointerEvent,
  Prevent,
  Prevented,
  Role,
  Scene,
  ScrollEvent,
  State,
  StaticLeaf,
  TokenName,
  TokenRef,
  TokenSet,
  When,
};
/// The host driver's test seams (`trigger_` events, tree inspection), for headless
/// view tests on the host.
#[cfg(not(target_arch = "wasm32"))]
pub use skene_headless as headless;
pub use skene_macros::{lazy, main};
pub use skene_reactive as reactive;
pub use skene_router as router;
pub use skene_simple::SimpleVariant;
pub use skene_views::{
  Activate,
  Anchor,
  Behavior,
  ControlChild,
  CornerRadius,
  ListItem,
  Listbox,
  ListboxGroup,
  PointerPhase,
  Popup,
  RadioMember,
  Range,
  Roster,
  Roving,
  RovingGroup,
  TextEntry,
  Toggle,
  TypeMetrics,
  typography,
};
/// The web driver, for a library author writing a view directly against the browser
/// DOM or the server's markup.
///
/// The only web-specific names the facade exposes, kept out of [`prelude`] so portable
/// app code never names a driver.
#[cfg(target_arch = "wasm32")]
pub use skene_web as web;
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub use skene_web::Rendered;
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
  media::{derive_scheme, set_scheme, stored_scheme},
  typography_set::Typography,
};

/// The driver the build selected: the web driver on `wasm32` (the browser leaf on
/// `wasm32-unknown-unknown`, the server leaf on `wasm32-wasip2`), the headless host
/// driver elsewhere.
#[cfg(target_arch = "wasm32")]
pub type ActiveDriver = skene_web::Web;
/// The driver the build selected: the web driver on `wasm32` (the browser leaf on
/// `wasm32-unknown-unknown`, the server leaf on `wasm32-wasip2`), the headless host
/// driver elsewhere.
#[cfg(not(target_arch = "wasm32"))]
pub type ActiveDriver = skene_headless::Headless;

/// The erased token definition the active driver stores per token.
pub type TokenValue = <ActiveDriver as skene_core::Driver>::TokenValue;

/// A view on the active driver; see [`skene_core::View`].
pub trait View: skene_core::View<ActiveDriver> {
  /// Realizes this view and returns its root node.
  #[inline(always)]
  fn build(self) -> NodeId
  where
    Self: Sized,
  {
    skene_core::View::build(self)
  }

  /// Erases this view's concrete type.
  #[inline(always)]
  fn into_any(self) -> AnyView
  where
    Self: Sized,
  {
    skene_core::View::into_any(self)
  }
}

impl<V: skene_core::View<ActiveDriver>> View for V {}

/// A value that can be turned into a view on the active driver, accepted as a child
/// of every kind; see [`skene_core::IntoView`] and [`AnyKind`].
pub trait IntoView: skene_core::IntoView<ActiveDriver> + AnyKind {
  /// Converts this value into a view.
  #[inline(always)]
  fn into_view(self) -> AnyView
  where
    Self: Sized,
  {
    skene_core::IntoView::into_view(self)
  }
}

impl<V: skene_core::IntoView<ActiveDriver> + AnyKind> IntoView for V {}

/// A token set read from the theme in force on the active driver; see
/// [`skene_views::ThemedIn`].
///
/// Every [`TokenSet`] with a default has it: `SimplePalette::current()` reads the
/// nearest theme's palette, or the default when no theme is above.
pub trait ThemedIn: skene_views::ThemedIn<ActiveDriver> {
  /// The set the nearest theme provides, or the default without one.
  #[inline(always)]
  #[must_use]
  fn current() -> Self {
    <Self as skene_views::ThemedIn<ActiveDriver>>::current()
  }

  /// The nearest theme's set as a memo, or a memo of the default without one.
  #[inline(always)]
  fn watch() -> skene_reactive::Memo<Self> {
    <Self as skene_views::ThemedIn<ActiveDriver>>::watch()
  }
}

impl<P: skene_views::ThemedIn<ActiveDriver>> ThemedIn for P {}

/// A type-erased view.
pub type AnyView = skene_core::AnyView<ActiveDriver>;
/// The one freely-decorated box around exactly one child; see
/// [`skene_views::Container`].
pub type Container = skene_views::Container<ActiveDriver>;
/// A horizontal flex layout; see [`skene_views::Row`].
pub type Row = skene_views::Row<ActiveDriver>;
/// A vertical flex layout; see [`skene_views::Column`].
pub type Column = skene_views::Column<ActiveDriver>;
/// A grid layout; see [`skene_views::Grid`].
pub type Grid = skene_views::Grid<ActiveDriver>;
/// A stack layout; see [`skene_views::Stack`].
pub type Stack = skene_views::Stack<ActiveDriver>;
/// The box every layout view is built from, accepting children of `Kind`; see
/// [`skene_views::LayoutBox`].
pub type LayoutBox<Kind> = skene_views::LayoutBox<ActiveDriver, Kind>;
/// A flex child's grow, shrink and initial size; see [`skene_views::Flexible`].
pub type Flexible<C = NoChild> = skene_views::Flexible<ActiveDriver, C>;
/// A stack child pinned to its edges; see [`skene_views::Pinned`].
pub type Pinned<C = NoChild> = skene_views::Pinned<ActiveDriver, C>;
/// A grid child's placement; see [`skene_views::GridCell`].
pub type GridCell<C = NoChild> = skene_views::GridCell<ActiveDriver, C>;
/// A run of text on its own line; see [`skene_views::Text`].
pub type Text = skene_views::Text<ActiveDriver>;
/// A run inside a paragraph; see [`skene_views::Span`].
pub type Span = skene_views::Span<ActiveDriver>;
/// A paragraph of spans; see [`skene_views::RichText`].
pub type RichText = skene_views::RichText<ActiveDriver>;
/// A branch shown or hidden by a condition; see [`skene_views::Show`].
pub type Show<C = NoChild> = skene_views::Show<ActiveDriver, C>;
/// One of several branches, by key; see [`skene_views::Switch`].
pub type Switch<K> = skene_views::Switch<ActiveDriver, K>;
/// Fallback content while a subtree's async work is pending; see
/// [`skene_views::Suspense`].
pub type Suspense<C = NoChild> = skene_views::Suspense<ActiveDriver, C>;
/// Fallback content when a subtree reports an error; see
/// [`skene_views::ErrorBoundary`].
pub type ErrorBoundary<C = NoChild> = skene_views::ErrorBoundary<ActiveDriver, C>;
/// The one theme view, defining the token set `P` beneath it; see
/// [`skene_views::Theme`].
pub type Theme<P, C = NoChild> = skene_views::Theme<ActiveDriver, P, C>;
/// The one control builder over a behavior; see [`skene_views::Control`].
pub type Control<B, C = NoChild> = skene_views::Control<ActiveDriver, B, C>;
/// A built control, for a behavior to act on; see [`skene_views::ControlHandle`].
pub type ControlHandle = skene_views::ControlHandle<ActiveDriver>;
/// A control's label; see [`skene_views::Label`].
pub type Label = skene_views::Label<ActiveDriver>;
/// A button; see [`skene_views::RawButton`].
pub type RawButton<C = NoChild> = skene_views::RawButton<ActiveDriver, C>;
/// A checkbox; see [`skene_views::RawCheckbox`].
pub type RawCheckbox<C = NoChild> = skene_views::RawCheckbox<ActiveDriver, C>;
/// A slider; see [`skene_views::RawSlider`].
pub type RawSlider<C = NoChild> = skene_views::RawSlider<ActiveDriver, C>;
/// A radio group; see [`skene_views::RawRadioGroup`].
pub type RawRadioGroup<C = NoChild> = skene_views::RawRadioGroup<ActiveDriver, C>;
/// A radio inside a group; see [`skene_views::RawRadio`].
pub type RawRadio<C = NoChild> = skene_views::RawRadio<ActiveDriver, C>;
/// A single-line text input; see [`skene_views::RawTextField`].
pub type RawTextField<C = NoChild> = skene_views::RawTextField<ActiveDriver, C>;
/// A multi-line text input; see [`skene_views::RawTextEditor`].
pub type RawTextEditor<C = NoChild> = skene_views::RawTextEditor<ActiveDriver, C>;
/// A listbox; see [`skene_views::RawListbox`].
pub type RawListbox<C = NoChild> = skene_views::RawListbox<ActiveDriver, C>;
/// An option inside a listbox or a picker; see [`skene_views::RawOption`].
pub type RawOption<C = NoChild> = skene_views::RawOption<ActiveDriver, C>;
/// A picker: a popup face, an overlay and a listbox; see [`skene_views::RawPicker`].
pub type RawPicker<F = NoChild> = skene_views::RawPicker<ActiveDriver, F>;
/// A picker's face; see [`skene_views::Face`].
pub type Face = skene_views::Face<ActiveDriver>;
/// Content above the app; see [`skene_views::Overlay`].
pub type Overlay<C = NoChild> = skene_views::Overlay<ActiveDriver, C>;
/// A handle moving focus to a built view; see [`skene_views::FocusHandle`].
pub type FocusHandle = skene_views::FocusHandle<ActiveDriver>;
/// A scrolling keyed list building only the slice in view; see
/// [`skene_views::LazyColumn`].
pub type LazyColumn = skene_views::LazyColumn<ActiveDriver>;
/// The closure building a view's child under the context the view provides; see
/// [`skene_views::ChildBuilder`].
pub type ChildBuilder = skene_views::ChildBuilder<ActiveDriver>;

/// The Simple design language's palette; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<f32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<f32>;
/// See [`skene_simple::SimpleButton`].
pub type SimpleButton = skene_simple::SimpleButton<ActiveDriver>;
/// See [`skene_simple::SimpleCard`].
pub type SimpleCard = skene_simple::SimpleCard<ActiveDriver>;
/// See [`skene_simple::SimpleCheckbox`].
pub type SimpleCheckbox = skene_simple::SimpleCheckbox<ActiveDriver>;
/// See [`skene_simple::SimplePicker`].
pub type SimplePicker = skene_simple::SimplePicker<ActiveDriver>;
/// See [`skene_simple::SimpleRadio`].
pub type SimpleRadio = skene_simple::SimpleRadio<ActiveDriver>;
/// See [`skene_simple::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = NoChild> = skene_simple::SimpleRadioGroup<ActiveDriver, C>;
/// See [`skene_simple::SimpleSlider`].
pub type SimpleSlider = skene_simple::SimpleSlider<ActiveDriver>;
/// See [`skene_simple::SimpleTextEditor`].
pub type SimpleTextEditor = skene_simple::SimpleTextEditor<ActiveDriver>;
/// See [`skene_simple::SimpleTextField`].
pub type SimpleTextField = skene_simple::SimpleTextField<ActiveDriver>;

/// A condition on the environment, in logical pixels; see [`skene_core::Media`].
pub type Media = skene_core::Media<f32>;
/// A condition a property varies under; see [`skene_core::Condition`].
pub type Condition = skene_core::Condition<f32>;
/// A test of a container's size; see [`skene_core::ContainerQuery`].
pub type ContainerQuery = skene_core::ContainerQuery<f32>;
/// A property's value: a leaf under conditions; see [`skene_core::Value`].
pub type Value<T> = skene_core::Value<T, Condition>;
/// A token's definition; see [`skene_core::Definition`].
pub type Definition<T> = skene_core::Definition<T, f32>;
/// A token as a palette holds it; see [`skene_core::Token`].
pub type Token<T> = skene_core::Token<T, f32>;
/// A border, in logical pixels; see [`skene_core::Border`].
pub type Border = skene_core::Border<f32>;
/// Spacing on the four edges of a box, in logical pixels.
pub type Inset<T = f32> = skene_core::Inset<T>;
/// A dimension, in logical pixels or as a fraction of the parent.
pub type Dimension = skene_core::Dimension<f32>;
/// Per-axis spacing between children, in logical pixels.
pub type Spacing = skene_core::Spacing<f32>;
/// A 2D size, in logical pixels.
pub type Size = skene_core::Size<f32>;
/// A point in the logical-pixel coordinate space.
pub type Point = skene_core::Point<f32>;
/// A positioned, sized rectangle, in logical pixels.
pub type Rect = skene_core::Rect<f32>;

/// Mounts a reactive subtree between anchors, rebuilt whenever a signal `build` reads
/// changes; see [`skene_core::derived`].
#[inline(always)]
pub fn derived(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::derived::<ActiveDriver>(build)
}

/// Reactively reconciles the content of the segment `start` begins from a keyed
/// collection; see [`skene_core::reconcile`].
#[inline(always)]
pub fn reconcile<T, I, K>(
  start: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + std::hash::Hash + 'static,
{
  skene_core::reconcile::<ActiveDriver, T, I, K>(start, items, key, build);
}

/// A view that renders nothing.
#[inline(always)]
#[must_use]
pub fn empty() -> impl View {
  skene_core::empty::<ActiveDriver>()
}

/// Wraps a view that only a client can realize, so it renders on an interactive
/// platform and falls back to [`empty`] where it cannot run.
///
/// The place for a low-level view that reaches a platform's raw handles, which have no
/// meaning off the client. On a server-rendering platform this yields nothing; the
/// real content arrives when the client hydrates. `build` returns the node it
/// realizes, exactly as a view's `build` does.
#[cfg(not(all(target_arch = "wasm32", target_os = "wasi")))]
#[inline(always)]
pub fn client_only(build: impl FnOnce() -> NodeId + 'static) -> impl View {
  AnyView::new(build)
}

/// On a server-rendering platform this yields nothing; the real content arrives when
/// the client hydrates.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[inline(always)]
pub fn client_only(_build: impl FnOnce() -> NodeId + 'static) -> impl View {
  empty()
}

/// Mounts and runs the application in the browser; a server-rendered page is hydrated
/// in place.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[inline(always)]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  #[cfg(feature = "snapshot")]
  skene_web::boot_with_dev_state(move || {
    skene_web::launch(app);

    tracing::info!("application mounted");
  });

  #[cfg(not(feature = "snapshot"))]
  {
    skene_web::launch(app);

    tracing::info!("application mounted");
  }
}

/// Hydrates a server-rendered page: adopts the document body and reads the embedded
/// state, so the client takes over the markup [`render_to_string`] produced without a
/// rebuild.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[inline(always)]
pub fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  #[cfg(feature = "snapshot")]
  skene_web::boot_with_dev_state(move || {
    skene_web::hydrate_page(app);

    tracing::info!("application hydrated");
  });

  #[cfg(not(feature = "snapshot"))]
  {
    skene_web::hydrate_page(app);

    tracing::info!("application hydrated");
  }
}

/// Builds the application against the headless driver, under the root owner.
///
/// The host has no platform to show it on; this is the entry a host build of an app
/// runs, so the same `#[main]` compiles everywhere.
#[cfg(not(target_arch = "wasm32"))]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_core::root_owner().enter(|| {
    let _ = skene_core::IntoView::into_view(app()).build();
  });

  tracing::info!("application built against the headless driver");
}

/// Renders the application, for a server-rendering platform: builds the tree, drives
/// its resources until they settle, and serializes the result plus the resource state
/// for hydration; see [`skene_web::render_to_string`].
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[inline(always)]
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_web::render_to_string(app)
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
  skene_web::set_request_path(path);

  render_to_string(app)
}

/// The environment variable carrying a render's client hints: the request's hint
/// headers, one `Name: value` per line, read into
/// [`RenderHints`](skene_web::RenderHints) before the render.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub const HINTS_VARIABLE: &str = "SKENE_HINTS";

/// The server entry `#[main]` expands to: renders the path given as the first argument
/// (default `/`) and writes the complete page to stdout—or, given `--routes`, writes
/// the statically-renderable routes one per line (see [`install_static_router`]), or,
/// given `--stream <path>`, writes the page as [`render_page_stream`] flushes it. The
/// client hints in [`HINTS_VARIABLE`], when set, are the environment the render
/// answers with.
///
/// This makes the app's server build a plain command—`wasmtime app.wasm /some/path`
/// prints the document—which is exactly the interface the runner embeds.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_main<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  use std::io::Write as _;

  if let Ok(hints) = std::env::var(HINTS_VARIABLE) {
    let headers = hints
      .lines()
      .filter_map(|line| line.split_once(':'))
      .map(|(name, value)| (name.trim(), value.trim()));

    skene_web::set_render_hints(skene_web::RenderHints::from_headers(headers));
  }

  let argument = std::env::args().nth(1).unwrap_or_else(|| "/".to_owned());

  if argument == "--routes" {
    let _ = render_to_string_at("/", app);

    for route in skene_web::recorded_static_routes().unwrap_or_else(|| vec!["/".to_owned()]) {
      println!("{route}");
    }

    return;
  }

  if argument == "--stream" {
    let path = std::env::args().nth(2).unwrap_or_else(|| "/".to_owned());

    skene_web::set_request_path(&path);
    render_page_stream(app, |piece| {
      print!("{piece}");

      let _ = std::io::stdout().flush();
    });

    return;
  }

  let rendered = render_to_string_at(&argument, app);

  print!("{}", rendered.page());

  let _ = std::io::stdout().flush();
}

/// A server render as an ordered sequence of HTML chunks, ready to flush one after
/// another.
///
/// The first chunk opens the document and carries the shell; each subsequent one
/// carries a [`Deferred`] boundary's content, streamed once it settled, behind a
/// `<style>` delta for the classes it introduces; the last closes the document. A
/// client with no JavaScript receives a correct, styled document progressively, since
/// the chunks concatenate, in order, into the markup [`render_to_string`] would
/// produce with the deltas beside the content they style.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[must_use]
pub struct Stream {
  chunks: Vec<String>,
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
impl Stream {
  /// The chunks, in flush order.
  #[inline(always)]
  #[must_use]
  pub fn chunks(&self) -> &[String] {
    &self.chunks
  }

  /// Consumes the stream into its chunks, in flush order.
  #[inline(always)]
  #[must_use]
  pub fn into_chunks(self) -> Vec<String> {
    self.chunks
  }

  /// The whole document, the chunks concatenated in order: what a no-JavaScript client
  /// ends up with once every chunk has arrived.
  #[inline(always)]
  #[must_use]
  pub fn document(&self) -> String {
    self.chunks.concat()
  }
}

/// The opening of a streamed document: the head with its styles, up to the body.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
fn page_opening(head: &str, styles: &str) -> String {
  format!("<!DOCTYPE html><html><head>{head}{styles}</head><body>")
}

/// The closing of a streamed document: the state script and the end of the body.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
fn page_closing(state: &str) -> String {
  format!(
    "<script id=\"{id}\" type=\"application/json\">{state}</script></body></html>",
    id = skene_web::STATE_SCRIPT_ID,
    // `<` keeps a value's `<` from closing the script early, and is valid JSON that
    // decodes back to `<`.
    state = state.replace('<', "\\u003c"),
  )
}

/// Renders the application as an in-order HTML stream, for a server-rendering
/// platform.
///
/// Like [`render_to_string`], but flushes the shell before every [`Deferred`]
/// boundary's slow content: the returned [`Stream`]'s chunks are written to the
/// response as they are produced, so the client sees the page progressively without
/// waiting on the slowest data, and, because the chunks are emitted in document order
/// with their styles, with no JavaScript required.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_to_stream<V, F>(app: F) -> Stream
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let (styles, head, mut chunks, state) = skene_web::render_to_stream(app);

  if let Some(first) = chunks.first_mut() {
    first.insert_str(0, &page_opening(&head, &styles));
  }

  if let Some(last) = chunks.last_mut() {
    last.push_str(&page_closing(&state));
  }

  Stream { chunks }
}

/// Renders the application as a progressive page, each flush handed to `sink` as it
/// is produced: the opening shell, a body chunk per settled [`Deferred`] boundary, and
/// the closing tail. The flushes concatenate to [`render_to_stream`]'s
/// [`document`](Stream::document), so writing each one to a response streams the page.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub fn render_page_stream<V, F>(app: F, mut sink: impl FnMut(&str))
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_web::render_to_stream_with(app, |piece| match piece {
    skene_web::StreamPiece::Shell { styles, head } => sink(&page_opening(head, styles)),
    skene_web::StreamPiece::Chunk(chunk) => sink(&chunk),
    skene_web::StreamPiece::Tail(state) => sink(&page_closing(&state)),
  });
}
