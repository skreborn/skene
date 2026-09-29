//! # skene-gui
//!
//! The GUI facade: a GUI app's single dependency. It fixes the scalar (logical pixels,
//! `f32`), names the platform the build selects ([`ActivePlatform`]: the web on `wasm32`,
//! the headless host platform elsewhere) and aliases every built-in view to it, so app
//! code names no platform. The medium's own contribution
//! is [`Typography`], the [`media`] module and the entry points: `launch` in the
//! browser, `render_to_string`, `render_to_stream` and `render_main` on the server
//! leaf, and [`macro@main`] expanding to whichever the target has.

pub mod media;
pub mod prelude;
#[cfg(skene_leaf = "browser")]
pub mod split;
pub mod view_transitions;

mod deferred;
pub mod head;
mod lazy;
mod link;
mod resource;
mod routing;
mod task;

/// The pixel medium's shared vocabulary: the style extension, the typography set and
/// the painting path.
pub use skene_common::gui::{
  Canvas,
  GuiExt,
  GuiProperty,
  IntoBrush,
  Painter,
  Painting,
  Typography,
  kurbo,
  peniko,
};
pub use skene_common::{
  Activate,
  Anchor,
  Behavior,
  ControlChild,
  CornerRadius,
  ItemExtent,
  ListItem,
  Listbox,
  ListboxGroup,
  Menu,
  MenuGroup,
  MenuItem,
  MenuOption,
  NativePicker,
  PointerPhase,
  Popup,
  RadioMember,
  Range,
  Roster,
  Roving,
  RovingGroup,
  TOOLTIP_DELAY,
  TOOLTIP_GRACE,
  TextEntry,
  Toggle,
  TypeMetrics,
  routing as router,
  typography,
};
pub use skene_core as core;
pub use skene_core::{
  AnyKind,
  Bubbling,
  Children,
  ClickEvent,
  Code,
  Color,
  ColorScheme,
  ContainerName,
  ControlKind,
  ControlState,
  Definitions,
  DragData,
  Errors,
  Expr,
  FocusOrder,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  Key,
  KeyEvent,
  Leaf,
  Length,
  Modifiers,
  NamedKey,
  NoChild,
  NodeId,
  Number,
  Part,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerId,
  PointerInfo,
  PointerType,
  Prevent,
  Prevented,
  Role,
  Scene,
  ScrollEvent,
  Shortcut,
  State,
  StaticLeaf,
  TextDecoration,
  TextDecorationStyle,
  TextSelection,
  TokenName,
  TokenRef,
  TokenSet,
  WheelEvent,
  WheelUnit,
  When,
};
/// The host platform's test seams (`trigger_` events, tree inspection), for headless
/// view tests on the host.
#[cfg(skene_leaf = "host")]
pub use skene_headless as headless;
pub use skene_macros::{lazy, main};
pub use skene_reactive as reactive;
/// The web platform, for a library author writing a view directly against the browser
/// DOM or the server's markup.
///
/// The only web-specific names the facade exposes, kept out of [`prelude`] so portable
/// app code never names a platform.
#[cfg(not(skene_leaf = "host"))]
pub use skene_web as web;
#[cfg(skene_leaf = "server")]
pub use skene_web::Rendered;
pub use tracing;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_bindgen;

pub use crate::{
  deferred::Deferred,
  lazy::Lazy,
  link::Link,
  media::{derive_scheme, set_scheme, stored_scheme},
  resource::{LoadState, Resource},
  routing::{install_router, install_static_router},
  task::spawn,
};

/// The platform the build selected: the web platform in the browser
/// (`wasm32-unknown-unknown`) and on a server render (`--cfg skene_platform="server"`),
/// the headless host platform elsewhere.
#[cfg(not(skene_leaf = "host"))]
pub type ActivePlatform = skene_web::Web;
/// The platform the build selected: the web platform in the browser
/// (`wasm32-unknown-unknown`) and on a server render (`--cfg skene_platform="server"`),
/// the headless host platform elsewhere.
#[cfg(skene_leaf = "host")]
pub type ActivePlatform = skene_headless::Headless;

/// The erased token definition the active platform stores per token.
pub type TokenValue = <ActivePlatform as skene_core::Platform>::TokenValue;

/// A view on the active platform; see [`skene_core::View`].
pub trait View: skene_core::View<ActivePlatform> {
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

impl<V: skene_core::View<ActivePlatform>> View for V {}

/// A value that can be turned into a view on the active platform, accepted as a child
/// of every kind; see [`skene_core::IntoView`] and [`AnyKind`].
pub trait IntoView: skene_core::IntoView<ActivePlatform> + AnyKind {
  /// Converts this value into a view.
  #[inline(always)]
  fn into_view(self) -> AnyView
  where
    Self: Sized,
  {
    skene_core::IntoView::into_view(self)
  }
}

impl<V: skene_core::IntoView<ActivePlatform> + AnyKind> IntoView for V {}

/// A token set read from the theme in force on the active platform; see
/// [`skene_common::ThemedIn`].
///
/// Every [`TokenSet`] with a default has it: `SimplePalette::current()` reads the
/// nearest theme's palette, or the default when no theme is above.
pub trait ThemedIn: skene_common::ThemedIn<ActivePlatform> {
  /// The set the nearest theme provides, or the default without one.
  #[inline(always)]
  #[must_use]
  fn current() -> Self {
    <Self as skene_common::ThemedIn<ActivePlatform>>::current()
  }

  /// The nearest theme's set as a memo, or a memo of the default without one.
  #[inline(always)]
  fn watch() -> skene_reactive::Memo<Self> {
    <Self as skene_common::ThemedIn<ActivePlatform>>::watch()
  }
}

impl<S: skene_common::ThemedIn<ActivePlatform>> ThemedIn for S {}

/// A type-erased view.
pub type AnyView = skene_core::AnyView<ActivePlatform>;
/// The one freely-decorated box around exactly one child; see
/// [`skene_common::Container`].
pub type Container = skene_common::Container<ActivePlatform>;
/// A horizontal flex layout; see [`skene_common::Row`].
pub type Row = skene_common::Row<ActivePlatform>;
/// A vertical flex layout; see [`skene_common::Column`].
pub type Column = skene_common::Column<ActivePlatform>;
/// A grid layout; see [`skene_common::Grid`].
pub type Grid = skene_common::Grid<ActivePlatform>;
/// A stack layout; see [`skene_common::Stack`].
pub type Stack = skene_common::Stack<ActivePlatform>;
/// The box every layout view is built from, accepting children of `Kind`; see
/// [`skene_common::LayoutBox`].
pub type LayoutBox<Kind> = skene_common::LayoutBox<ActivePlatform, Kind>;
/// A flex child's grow, shrink and initial size; see [`skene_common::Flexible`].
pub type Flexible<C = NoChild> = skene_common::Flexible<ActivePlatform, C>;
/// A stack child pinned to its edges; see [`skene_common::Pinned`].
pub type Pinned<C = NoChild> = skene_common::Pinned<ActivePlatform, C>;
/// A grid child's placement; see [`skene_common::GridCell`].
pub type GridCell<C = NoChild> = skene_common::GridCell<ActivePlatform, C>;
/// A run of text on its own line; see [`skene_common::Text`].
pub type Text = skene_common::Text<ActivePlatform>;
/// A run inside a paragraph; see [`skene_common::Span`].
pub type Span = skene_common::Span<ActivePlatform>;
/// A paragraph of spans; see [`skene_common::RichText`].
pub type RichText = skene_common::RichText<ActivePlatform>;
/// A branch shown or hidden by a condition; see [`skene_common::Show`].
pub type Show<C = NoChild> = skene_common::Show<ActivePlatform, C>;
/// The view a `set_key` named, built under a keyed scope; see [`skene_common::Keyed`].
pub type Keyed<V> = skene_common::Keyed<ActivePlatform, V>;
/// One of several branches, by key; see [`skene_common::Switch`].
pub type Switch<K> = skene_common::Switch<ActivePlatform, K>;
/// Fallback content while a subtree's async work is pending; see
/// [`skene_common::Suspense`].
pub type Suspense<C = NoChild> = skene_common::Suspense<ActivePlatform, C>;
/// The form a signal makes possible over the one that needs none; see
/// [`skene_common::Enhance`].
pub type Enhance<C = NoChild> = skene_common::Enhance<ActivePlatform, C>;
/// Fallback content when a subtree reports an error; see
/// [`skene_common::ErrorBoundary`].
pub type ErrorBoundary<C = NoChild> = skene_common::ErrorBoundary<ActivePlatform, C>;
/// The one theme view, defining the token set `P` beneath it; see
/// [`skene_common::Theme`].
pub type Theme<S, C = NoChild> = skene_common::Theme<ActivePlatform, S, C>;
/// The one control builder over a behavior; see [`skene_common::Control`].
pub type Control<B, C = NoChild> = skene_common::Control<ActivePlatform, B, C>;
/// A built control, for a behavior to act on; see [`skene_common::ControlHandle`].
pub type ControlHandle = skene_common::ControlHandle<ActivePlatform>;
/// A control's label; see [`skene_common::Label`].
pub type Label = skene_common::Label<ActivePlatform>;
/// A button; see [`skene_common::RawButton`].
pub type RawButton<C = NoChild> = skene_common::RawButton<ActivePlatform, C>;
/// A checkbox; see [`skene_common::RawCheckbox`].
pub type RawCheckbox<C = NoChild> = skene_common::RawCheckbox<ActivePlatform, C>;
/// A slider; see [`skene_common::RawSlider`].
pub type RawSlider<C = NoChild> = skene_common::RawSlider<ActivePlatform, C>;
/// A radio group; see [`skene_common::RawRadioGroup`].
pub type RawRadioGroup<C = NoChild> = skene_common::RawRadioGroup<ActivePlatform, C>;
/// A radio inside a group; see [`skene_common::RawRadio`].
pub type RawRadio<C = NoChild> = skene_common::RawRadio<ActivePlatform, C>;
/// A single-line text input; see [`skene_common::RawTextField`].
pub type RawTextField<C = NoChild> = skene_common::RawTextField<ActivePlatform, C>;
/// A multi-line text input; see [`skene_common::RawTextEditor`].
pub type RawTextEditor<C = NoChild> = skene_common::RawTextEditor<ActivePlatform, C>;
/// A listbox; see [`skene_common::RawListbox`].
pub type RawListbox<C = NoChild> = skene_common::RawListbox<ActivePlatform, C>;
/// An option inside a listbox or a picker; see [`skene_common::RawOption`].
pub type RawOption<C = NoChild> = skene_common::RawOption<ActivePlatform, C>;
/// A picker: a popup face, an overlay and a listbox; see [`skene_common::RawPicker`].
pub type RawPicker<F = NoChild> = skene_common::RawPicker<ActivePlatform, F>;
/// A picker's face; see [`skene_common::Face`].
pub type Face = skene_common::Face<ActivePlatform>;
/// Content above the app; see [`skene_common::Overlay`].
pub type Overlay<C = NoChild> = skene_common::Overlay<ActivePlatform, C>;
/// Content shown beside the view it describes; see [`skene_common::Tooltip`].
pub type Tooltip<C = NoChild> = skene_common::Tooltip<ActivePlatform, C>;
/// Commands shown beside the pointer on a secondary press over the view it serves;
/// see [`skene_common::ContextMenu`].
pub type ContextMenu<C = NoChild> = skene_common::ContextMenu<ActivePlatform, C>;
/// A menu; see [`skene_common::RawMenu`].
pub type RawMenu<C = NoChild> = skene_common::RawMenu<ActivePlatform, C>;
/// One item within a menu; see [`skene_common::RawMenuItem`].
pub type RawMenuItem<C = NoChild> = skene_common::RawMenuItem<ActivePlatform, C>;
/// A handle moving focus to a built view; see [`skene_common::FocusHandle`].
pub type FocusHandle = skene_common::FocusHandle<ActivePlatform>;
/// A handle reporting a built view's measured size; see [`skene_common::SizeHandle`].
pub type SizeHandle = skene_common::SizeHandle<ActivePlatform>;
/// A box drawn by a paint callback; see [`skene_common::Painted`].
pub type Painted = skene_common::gui::Painted<ActivePlatform>;
/// A scrolling keyed list building only the slice in view; see
/// [`skene_common::LazyColumn`].
pub type LazyColumn = skene_common::LazyColumn<ActivePlatform>;
/// A scrolling keyed row building only the slice in view; see
/// [`skene_common::LazyRow`].
pub type LazyRow = skene_common::LazyRow<ActivePlatform>;
/// The closure building a view's child under the context the view provides; see
/// [`skene_common::ChildBuilder`].
pub type ChildBuilder = skene_common::ChildBuilder<ActivePlatform>;

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
  skene_core::derived::<ActivePlatform>(build)
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
  skene_core::reconcile::<ActivePlatform, T, I, K>(start, items, key, build);
}

/// A view that renders nothing.
#[inline(always)]
#[must_use]
pub fn empty() -> impl View {
  skene_core::empty::<ActivePlatform>()
}

/// Wraps a view that only a client can realize.
///
/// It builds once the page stands [`hydrated`](media::hydrated), so a server renders
/// nothing in its place and a hydrating client adopts that empty place before building
/// it.
///
/// The place for a low-level view that reaches a platform's raw handles, which have no
/// meaning off the client. `build` returns the node it realizes, exactly as a view's
/// `build` does.
#[inline(always)]
pub fn client_only(build: impl FnOnce() -> NodeId + 'static) -> impl View {
  Enhance::new(media::hydrated()).set_child_builder(build)
}

/// Mounts and runs the application in the browser; a server-rendered page is hydrated
/// in place.
#[cfg(skene_leaf = "browser")]
#[inline(always)]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_web::boot_with_dev_state(move || {
    skene_web::launch(app);

    tracing::info!("application mounted");
  });
}

/// Hydrates a server-rendered page: adopts the document body and reads the embedded
/// state, so the client takes over the server's markup without a rebuild.
#[cfg(skene_leaf = "browser")]
#[inline(always)]
pub fn hydrate_page<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_web::boot_with_dev_state(move || {
    skene_web::hydrate_page(app);

    tracing::info!("application hydrated");
  });
}

/// Builds the application against the headless platform, under the root owner.
///
/// The host has no platform to show it on; this is the entry a host build of an app
/// runs, so the same `#[main]` compiles everywhere.
#[cfg(skene_leaf = "host")]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_headless::install_executor();

  skene_core::root_owner().enter(|| {
    let _ = skene_core::IntoView::into_view(app()).build();
  });

  tracing::info!("application built against the headless platform");
}

/// Renders the application, for a server-rendering platform.
///
/// It builds the tree, drives its resources until they settle, and serializes the
/// result plus the resource state for hydration; see [`skene_web::render_to_string`].
#[cfg(skene_leaf = "server")]
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
#[cfg(skene_leaf = "server")]
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
#[cfg(skene_leaf = "server")]
pub const HINTS_VARIABLE: &str = "SKENE_HINTS";

/// The environment variable carrying the nonce a render's inline scripts carry, as the
/// response's Content Security Policy names it; see
/// [`set_script_nonce`](skene_web::set_script_nonce).
#[cfg(skene_leaf = "server")]
pub const NONCE_VARIABLE: &str = "SKENE_SCRIPT_NONCE";

/// The environment variable that, set, makes the pages join the dev loop of
/// `skene run web`; see [`set_dev_loop`](skene_web::set_dev_loop).
#[cfg(skene_leaf = "server")]
pub const DEV_VARIABLE: &str = "SKENE_DEV";

/// The first line and the last of `--routes` output, the routes one per line between.
#[cfg(skene_leaf = "server")]
pub const ROUTES_MARKERS: [&str; 2] = ["--skene-routes--", "--skene-routes-end--"];

/// What `app-server --help` prints.
#[cfg(skene_leaf = "server")]
const USAGE: &str = "usage: app-server [path | --stream path | --routes | --serve | --help]

  path            render the page at path (default /) to stdout
  --stream path   render the page at path to stdout as it streams
  --routes        list the statically renderable routes between markers
  --serve         render one length-framed request after another from stdin
  --help          print this

SKENE_HINTS names the client hints a render answers with, one `Name: value` per
line; SKENE_SCRIPT_NONCE the nonce its inline scripts carry; SKENE_DEV, set, makes
the pages join the dev loop.";

/// The server entry `#[main]` expands to.
///
/// It renders the path given as the first argument (default `/`) and writes the
/// complete page to stdout—or, given `--routes`, writes the statically-renderable
/// routes one per line between [`ROUTES_MARKERS`] (see [`install_static_router`]), or,
/// given `--stream <path>`, writes the page as [`render_page_stream`] flushes it, or,
/// given `--serve`, renders one request after another read from stdin (see
/// [`serve_renders`]), or, given `--help`, prints its usage. Every page boots and
/// preloads the client `bundle`, the name wasm-bindgen gave the app's files. The client
/// hints in [`HINTS_VARIABLE`], when set, are the environment the render answers with,
/// the nonce in [`NONCE_VARIABLE`] its inline scripts carry, and [`DEV_VARIABLE`], set,
/// makes the pages join the dev loop; `tracing` events, a render's deadline report
/// among them, go to stderr.
///
/// This makes the app's server build a plain command—`app-server /some/path` prints
/// the document—which is exactly the interface a server or a generator embeds.
#[cfg(skene_leaf = "server")]
pub fn render_main<V, F>(bundle: &str, app: F)
where
  F: Fn() -> V + Clone + 'static,
  V: IntoView,
{
  use std::io::Write as _;

  skene_web::install_logging();
  skene_web::set_client_bundle(Some(bundle));
  skene_web::set_dev_loop(std::env::var_os(DEV_VARIABLE).is_some());

  if matches!(std::env::args().nth(1).as_deref(), Some("--help" | "-h")) {
    println!("{USAGE}");

    return;
  }

  if std::env::args().nth(1).as_deref() == Some("--serve") {
    serve_renders(app);

    return;
  }

  skene_web::set_script_nonce(std::env::var(NONCE_VARIABLE).ok().as_deref());

  if let Ok(hints) = std::env::var(HINTS_VARIABLE) {
    let headers = hints
      .lines()
      .filter_map(|line| line.split_once(':'))
      .map(|(name, value)| (name.trim(), value.trim()));

    skene_web::set_render_hints(skene_web::RenderHints::from_headers(headers));
  }

  let argument = std::env::args().nth(1).unwrap_or_else(|| "/".to_owned());

  if argument == "--routes" {
    let mut output = protocol_output();
    let _ = render_to_string_at("/", app);
    let [first, last] = ROUTES_MARKERS;
    let routes = skene_web::recorded_static_routes().unwrap_or_else(|| vec!["/".to_owned()]);

    let _ = writeln!(output, "{first}");

    for route in routes {
      let _ = writeln!(output, "{route}");
    }

    let _ = writeln!(output, "{last}");
    let _ = output.flush();

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
#[derive(Debug)]
#[cfg(skene_leaf = "server")]
#[must_use]
pub struct Stream {
  chunks: Vec<String>,
}

#[cfg(skene_leaf = "server")]
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

/// Renders the application as an in-order HTML stream, for a server-rendering
/// platform.
///
/// Like [`render_to_string`], but flushes the shell before every [`Deferred`]
/// boundary's slow content: the returned [`Stream`]'s chunks are written to the
/// response as they are produced, so the client sees the page progressively without
/// waiting on the slowest data, and, because the chunks are emitted in document order
/// with their styles, with no JavaScript required.
#[cfg(skene_leaf = "server")]
pub fn render_to_stream<V, F>(app: F) -> Stream
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let mut chunks = Vec::new();
  let mut opening = String::new();
  let mut closing = String::new();

  skene_web::render_to_stream_with(app, |piece| match piece {
    skene_web::StreamPiece::Shell {
      styles,
      head,
      language,
    } => opening = skene_web::document_opening(language, head, styles),
    skene_web::StreamPiece::Chunk(chunk) => chunks.push(chunk),
    skene_web::StreamPiece::Tail(state) => closing = skene_web::document_closing(&state),
  });

  if let Some(first) = chunks.first_mut() {
    first.insert_str(0, &opening);
  }

  if let Some(last) = chunks.last_mut() {
    last.push_str(&closing);
  }

  Stream { chunks }
}

/// Renders the application as a progressive page, each flush handed to `sink` as it is
/// produced.
///
/// The flushes are the opening shell, a body chunk per settled [`Deferred`] boundary,
/// and the closing tail, and they concatenate to [`render_to_stream`]'s
/// [`document`](Stream::document), so writing each one to a response streams the page.
#[cfg(skene_leaf = "server")]
pub fn render_page_stream<V, F>(app: F, mut sink: impl FnMut(&str))
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  skene_web::render_to_stream_with(app, |piece| match piece {
    skene_web::StreamPiece::Shell {
      styles,
      head,
      language,
    } => sink(&skene_web::document_opening(language, head, styles)),
    skene_web::StreamPiece::Chunk(chunk) => sink(&chunk),
    skene_web::StreamPiece::Tail(state) => sink(&skene_web::document_closing(&state)),
  });
}

/// Renders requests read from stdin until it closes or sends one malformed, one page per
/// request to stdout, so a dev server or a generator keeps one process rendering.
///
/// A request and a response are each `Content-Length: <bytes>` on a line, an empty
/// line, and that many bytes. A request's bytes are the path on a line, percent-encoded
/// as a URL carries it, then one `Name: value` per line: the client hints, and the
/// nonce the page's inline scripts carry as `Script-Nonce`; a response's are the
/// complete page. Each is for that request alone, and a request without them renders
/// against the defaults.
#[cfg(skene_leaf = "server")]
pub fn serve_renders<V, F>(app: F)
where
  F: Fn() -> V + Clone + 'static,
  V: IntoView,
{
  use std::io::Write as _;

  let mut output = protocol_output();
  let stdin = std::io::stdin();
  let mut stdin = stdin.lock();

  while let Some(request) = read_framed(&mut stdin) {
    let (path, fields) = request.split_once('\n').unwrap_or((&request, ""));
    let fields = || {
      fields
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim(), value.trim()))
    };
    let nonce = fields()
      .find(|(name, _)| name.eq_ignore_ascii_case("script-nonce"))
      .map(|(_, nonce)| nonce);

    skene_web::set_script_nonce(nonce);
    skene_web::set_render_hints(skene_web::RenderHints::from_headers(fields()));

    let page = render_to_string_at(path, app.clone()).page();

    let _ = write!(output, "Content-Length: {}\n\n{page}", page.len());
    let _ = output.flush();
  }
}

/// The stream a protocol writes to: the process's standard output, taken aside, and
/// standard output itself pointed at standard error, so what the app prints stays out of
/// the protocol's way. Standard output as it is where it cannot be taken aside.
#[cfg(skene_leaf = "server")]
fn protocol_output() -> Box<dyn std::io::Write> {
  #[cfg(unix)]
  {
    use std::os::fd::AsFd as _;

    if let Ok(kept) = rustix::io::dup(std::io::stdout().as_fd())
      && rustix::stdio::dup2_stdout(std::io::stderr().as_fd()).is_ok()
    {
      return Box::new(std::fs::File::from(kept));
    }
  }

  Box::new(std::io::stdout())
}

/// Reads one length-framed message: `Content-Length: <bytes>` on a line, an empty
/// line, and that many bytes of UTF-8; `None` at the end of the input or for a
/// malformed message.
#[cfg(skene_leaf = "server")]
fn read_framed(input: &mut impl std::io::BufRead) -> Option<String> {
  let mut header = String::new();

  if input.read_line(&mut header).ok()? == 0 {
    return None;
  }

  let length: usize = header
    .trim()
    .strip_prefix("Content-Length:")?
    .trim()
    .parse()
    .ok()?;
  let mut blank = String::new();

  input.read_line(&mut blank).ok()?;

  if !blank.trim().is_empty() {
    return None;
  }

  let mut message = vec![0; length];

  input.read_exact(&mut message).ok()?;

  String::from_utf8(message).ok()
}
