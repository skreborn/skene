//! # skene-web
//!
//! The web's shell: a web app's single dependency. It fixes the scalar (logical pixels,
//! `f32`), names the platform the build selects ([`ActivePlatform`]: the web platform
//! in the browser and on a server render, the headless host platform elsewhere) and
//! aliases every built-in view to it, so app code names no platform. The medium's own
//! contribution is [`Typography`], the [`media`] module and the entry points: `launch`
//! in the browser, `render_to_string`, `render_to_stream` and `render_main` on the
//! server leaf, and [`macro@main`] expanding to whichever the target has. [`platform`]
//! is the web platform itself, for a view written against the DOM or the server's
//! markup.

pub mod head;
pub mod media;
pub mod platform;
pub mod prelude;
#[cfg(skene_web_platform = "browser")]
pub mod split;
#[cfg(skene_web_platform = "browser")]
pub mod testing;
pub mod view_transitions;

mod async_view;
#[cfg(skene_web_platform = "browser")]
mod browser;
#[cfg_attr(
  skene_web_platform = "headless",
  expect(dead_code, reason = "the compiler serves the leaves alone")
)]
mod compile;
mod condition;
mod declarations;
mod deferred;
#[cfg(not(skene_web_platform = "browser"))]
mod document;
mod expr;
#[cfg(skene_web_platform = "browser")]
mod gpu;
mod hints;
#[cfg_attr(
  all(skene_web_platform = "headless", not(test)),
  expect(dead_code, reason = "the interner serves the leaves alone")
)]
mod interner;
mod link;
#[cfg(not(skene_web_platform = "headless"))]
mod logging;
mod low_level;
#[cfg(not(skene_web_platform = "headless"))]
mod markup;
mod motion;
#[cfg(not(skene_web_platform = "headless"))]
mod realize;
mod resource;
mod resource_state;
mod routing;
#[cfg(skene_web_platform = "server")]
mod server;
#[cfg(not(skene_web_platform = "headless"))]
mod style_book;
mod task;
#[cfg_attr(
  skene_web_platform = "headless",
  expect(dead_code, reason = "tokens compile for the leaves alone")
)]
mod token;

/// The pixel medium's shared vocabulary: the style extension, the typography set and
/// the painting path.
pub use skene_common::gui::{
  Canvas,
  GuiProperty,
  GuiStyle,
  IntoBrush,
  Painter,
  Painting,
  Typography,
  kurbo,
  peniko,
};
pub use skene_common::{
  Activate,
  Behavior,
  ControlChild,
  CornerRadius,
  ItemExtent,
  Listbox,
  ListboxContext,
  ListboxOption,
  Menu,
  MenuContext,
  MenuEntry,
  MenuOption,
  NativePicker,
  PointerPhase,
  Popup,
  Radio,
  RadioGroup,
  RadioGroupContext,
  Roster,
  Slider,
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
  ControlState,
  Definitions,
  DragData,
  Errors,
  Expr,
  FocusOrder,
  Handler,
  HasChild,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  Key,
  KeyEvent,
  Leaf,
  Length,
  Modifiers,
  NamedKey,
  NativeElement,
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
#[cfg(skene_web_platform = "headless")]
pub use skene_headless as headless;
pub use skene_macros::{lazy, main};
pub use skene_reactive as reactive;
pub use tracing;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_bindgen;

#[cfg(skene_web_platform = "server")]
pub use self::platform::Rendered;
pub use crate::{
  async_view::AsyncView,
  deferred::Deferred,
  link::Link,
  media::{chosen_scheme, derive_scheme, set_scheme},
  resource::{LoadState, Resource},
  routing::{install_router, install_static_router},
  task::spawn,
};

/// The platform the build selected: the web platform in the browser
/// (`wasm32-unknown-unknown`) and on a server render (`--cfg skene_platform="server"`),
/// the headless host platform elsewhere.
#[cfg(not(skene_web_platform = "headless"))]
pub type ActivePlatform = platform::Web;
/// The platform the build selected: the web platform in the browser
/// (`wasm32-unknown-unknown`) and on a server render (`--cfg skene_platform="server"`),
/// the headless host platform elsewhere.
#[cfg(skene_web_platform = "headless")]
pub type ActivePlatform = skene_headless::Headless;

skene_common::facade!(ActivePlatform, f32, "logical pixels");

/// Wraps a view that only a client can realize.
///
/// It builds once the page stands [`hydrated`](media::hydrated), so a server renders
/// nothing in its place and a hydrating client adopts that empty place before building
/// it.
///
/// The place for a low-level view that reaches a platform's raw handles, which have no
/// meaning off the client. `build` returns the node it realizes, exactly as a view's
/// `build` does.
pub fn client_only(build: impl FnOnce() -> NodeId + 'static) -> impl View {
  Enhance::new(media::hydrated()).set_child_builder(build)
}

/// Mounts and runs the application in the browser; a server-rendered page is hydrated
/// in place.
#[cfg(skene_web_platform = "browser")]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  platform::boot_with_dev_state(move || {
    platform::launch(app);

    tracing::info!("application mounted");
  });
}

/// Builds the application against the headless platform, under the root owner.
///
/// The host has no platform to show it on; this is the entry a host build of an app
/// runs, so the same `#[main]` compiles everywhere.
#[cfg(skene_web_platform = "headless")]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  skene_headless::install_executor();

  skene_core::root_owner().enter(|| {
    let _ = skene_core::View::into_any(app()).build();
  });

  tracing::info!("application built against the headless platform");
}

/// Renders the application, for a server-rendering platform.
///
/// It builds the tree, drives its resources until they settle, and serializes the
/// result plus the resource state for hydration; see [`platform::render_to_string`].
#[cfg(skene_web_platform = "server")]
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  platform::render_to_string(app)
}

/// Renders the application for the given request path: the router (when installed via
/// [`install_router`]) resolves against `path`, and the render proceeds as
/// [`render_to_string`].
#[cfg(skene_web_platform = "server")]
pub fn render_to_string_at<V, F>(path: &str, app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  platform::set_request_path(path);

  render_to_string(app)
}

#[cfg(skene_web_platform = "server")]
pub use skene_dev::{DEV_VARIABLE, HINTS_VARIABLE, NONCE_VARIABLE, ROUTES_MARKERS};

/// What `app-server --help` prints.
#[cfg(skene_web_platform = "server")]
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
#[cfg(skene_web_platform = "server")]
pub fn render_main<V, F>(bundle: &str, app: F)
where
  F: Fn() -> V + Clone + 'static,
  V: View,
{
  use std::io::Write as _;

  platform::install_logging();
  platform::set_client_bundle(Some(bundle));
  platform::set_dev_loop(std::env::var_os(DEV_VARIABLE).is_some());

  if matches!(std::env::args().nth(1).as_deref(), Some("--help" | "-h")) {
    println!("{USAGE}");

    return;
  }

  if std::env::args().nth(1).as_deref() == Some("--serve") {
    serve_renders(app);

    return;
  }

  platform::set_script_nonce(std::env::var(NONCE_VARIABLE).ok().as_deref());

  if let Ok(hints) = std::env::var(HINTS_VARIABLE) {
    let headers = hints
      .lines()
      .filter_map(|line| line.split_once(':'))
      .map(|(name, value)| (name.trim(), value.trim()));

    platform::set_render_hints(platform::RenderHints::from_headers(headers));
  }

  let argument = std::env::args().nth(1).unwrap_or_else(|| "/".to_owned());

  if argument == "--routes" {
    let mut output = protocol_output();
    let _ = render_to_string_at("/", app);
    let [first, last] = ROUTES_MARKERS;
    let routes = platform::recorded_static_routes().unwrap_or_else(|| vec!["/".to_owned()]);

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

    platform::set_request_path(&path);
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
#[cfg(skene_web_platform = "server")]
#[must_use]
pub struct Stream {
  chunks: Vec<String>,
}

#[cfg(skene_web_platform = "server")]
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
#[cfg(skene_web_platform = "server")]
pub fn render_to_stream<V, F>(app: F) -> Stream
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  let mut chunks = Vec::new();
  let mut opening = String::new();
  let mut closing = String::new();

  platform::render_to_stream_with(app, |piece| match piece {
    platform::StreamPiece::Shell {
      styles,
      head,
      language,
    } => opening = platform::document_opening(language, head, styles),
    platform::StreamPiece::Chunk(chunk) => chunks.push(chunk),
    platform::StreamPiece::Tail(state) => closing = platform::document_closing(&state),
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
#[cfg(skene_web_platform = "server")]
pub fn render_page_stream<V, F>(app: F, mut sink: impl FnMut(&str))
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  platform::render_to_stream_with(app, |piece| match piece {
    platform::StreamPiece::Shell {
      styles,
      head,
      language,
    } => sink(&platform::document_opening(language, head, styles)),
    platform::StreamPiece::Chunk(chunk) => sink(&chunk),
    platform::StreamPiece::Tail(state) => sink(&platform::document_closing(&state)),
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
#[cfg(skene_web_platform = "server")]
pub fn serve_renders<V, F>(app: F)
where
  F: Fn() -> V + Clone + 'static,
  V: View,
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

    platform::set_script_nonce(nonce);
    platform::set_render_hints(platform::RenderHints::from_headers(fields()));

    let page = render_to_string_at(path, app.clone()).page();

    let _ = write!(output, "Content-Length: {}\n\n{page}", page.len());
    let _ = output.flush();
  }
}

/// The stream a protocol writes to: the process's standard output, taken aside, and
/// standard output itself pointed at standard error, so what the app prints stays out of
/// the protocol's way. Standard output as it is where it cannot be taken aside.
#[cfg(skene_web_platform = "server")]
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
#[cfg(skene_web_platform = "server")]
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
