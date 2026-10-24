//! # skene-web
//!
//! The web's shell: a web app's single dependency. It fixes the scalar (logical pixels,
//! `f32`), names the platform it realizes ([`Active`], the web platform in the browser
//! and on a server render) and aliases every built-in view to it, so app code names no
//! platform. The medium's own contribution is [`Typography`], the [`media`] module and
//! the entry points: `launch` in the browser, `render_to_string`, `render_to_stream`
//! and `render_main` on the server leaf, and [`macro@main`] expanding to whichever the
//! build names. [`Platform`] is the build's platform as a const an app matches on, and
//! [`platform`] holds each platform's own vocabulary: [`platform::web`] is the web
//! platform itself, for a view written against the DOM or the server's markup.
//!
//! A build names the web by `--cfg skene_platform="web"`: the browser leaf on
//! `wasm32-unknown-unknown`, and the server leaf on any target with `--cfg skene_server`
//! beside it. Every other build fails with a `compile_error!` saying why, and a terminal
//! build compiles the crate empty.
//!
//! A length is a plain number of logical pixels, an `i32` or an `f32`, wherever one is
//! taken: a setter, a derived value, a condition's variant, a token's definition and
//! every constructor measured in lengths ([`IntoScalar`]).
//!
//! ```
//! use skene_web::{Length, prelude::*};
//!
//! let gutter = Token::<Length>::new("gutter", 8).define_when(Media::min_width(900), 16);
//! let _ = Column::new()
//!   .set_spacing(Spacing::from(8).when(Media::min_width(900), 16))
//!   .set_padding(Inset::new(0, 8, 0, 8))
//!   .set_children((
//!     Row::new()
//!       .set_spacing(Spacing::new(8, -4))
//!       .set_padding(gutter.reference()),
//!     Container::new().set_width(16).set_child(Text::new("Whole")),
//!     Container::new()
//!       .set_width(16.5)
//!       .set_child(Text::new("Fractional")),
//!     Container::new()
//!       .derive_width(|| 16)
//!       .set_child(Text::new("Derived")),
//!   ));
//! ```

#![cfg(not(skene_platform = "terminal"))]

#[cfg(not(any(
  skene_platform = "android",
  skene_platform = "ios",
  skene_platform = "linux",
  skene_platform = "macos",
  skene_platform = "web",
  skene_platform = "windows",
)))]
compile_error!(
  "skene-web builds for the platform its build names: `--cfg skene_platform=\"web\"` in \
   the rustflags of `--target wasm32-unknown-unknown` for the browser, with `--cfg \
   skene_server` beside it for the server leaf (`cargo check-web`, `cargo check-server`)"
);
#[cfg(all(
  skene_platform = "web",
  not(skene_server),
  not(all(target_arch = "wasm32", target_os = "unknown"))
))]
compile_error!(
  "skene_platform = \"web\" builds the browser on wasm32-unknown-unknown alone: the \
   server leaf, on any target, sets `--cfg skene_server` beside it"
);
#[cfg(skene_platform = "android")]
compile_error!("skene-web realizes no android yet: its builds name `skene_platform = \"web\"`");
#[cfg(skene_platform = "ios")]
compile_error!("skene-web realizes no ios yet: its builds name `skene_platform = \"web\"`");
#[cfg(skene_platform = "linux")]
compile_error!("skene-web realizes no linux yet: its builds name `skene_platform = \"web\"`");
#[cfg(skene_platform = "macos")]
compile_error!("skene-web realizes no macos yet: its builds name `skene_platform = \"web\"`");
#[cfg(skene_platform = "windows")]
compile_error!("skene-web realizes no windows yet: its builds name `skene_platform = \"web\"`");

#[cfg(skene_platform = "web")]
pub mod head;
#[cfg(skene_platform = "web")]
pub mod media;
#[cfg(skene_platform = "web")]
pub mod platform;
#[cfg(skene_platform = "web")]
pub mod prelude;
#[cfg(all(skene_platform = "web", not(skene_server)))]
pub mod split;
#[cfg(all(skene_platform = "web", not(skene_server)))]
pub mod testing;
#[cfg(skene_platform = "web")]
pub mod view_transitions;
#[cfg(skene_platform = "web")]
pub mod window;

#[cfg(all(skene_platform = "web", not(skene_server)))]
mod browser;
#[cfg(skene_platform = "web")]
mod compile;
#[cfg(skene_platform = "web")]
mod condition;
#[cfg(skene_platform = "web")]
mod declarations;
#[cfg(skene_platform = "web")]
mod expr;
#[cfg(all(skene_platform = "web", not(skene_server), feature = "gpu"))]
mod gpu;
#[cfg(skene_platform = "web")]
mod hints;
#[cfg(skene_platform = "web")]
mod interner;
#[cfg(skene_platform = "web")]
mod link_policy;
#[cfg(skene_platform = "web")]
mod logging;
#[cfg(skene_platform = "web")]
mod low_level;
#[cfg(skene_platform = "web")]
mod markup;
#[cfg(skene_platform = "web")]
mod motion;
#[cfg(skene_platform = "web")]
mod realize;
#[cfg(skene_platform = "web")]
mod resource;
#[cfg(skene_platform = "web")]
mod resource_state;
#[cfg(skene_platform = "web")]
mod routing;
#[cfg(all(skene_platform = "web", skene_server))]
mod server;
#[cfg(skene_platform = "web")]
mod style_book;
#[cfg(skene_platform = "web")]
mod task;
#[cfg(skene_platform = "web")]
mod token;
#[cfg(skene_platform = "web")]
mod views;

/// The web medium's shared vocabulary: the style extension, the typography set and the
/// painting path.
pub use skene_foundation::web::{
  Canvas,
  IntoBrush,
  Painter,
  Painting,
  Typography,
  WebProperty,
  WebStyle,
  kurbo,
  peniko,
};
pub use skene_foundation::{
  Activate,
  AnyKind,
  BasicPickerBase,
  Behavior,
  Bubbling,
  Children,
  ClickEvent,
  Code,
  Color,
  ColorScheme,
  ColumnBase,
  ContainerBase,
  ContainerName,
  ContextMenuBase,
  ContextMenuWithCornerRadius,
  ControlBase,
  ControlState,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Definitions,
  DragData,
  EnhanceBase,
  ErrorBoundaryBase,
  Errors,
  Expr,
  FlexBase,
  FlexibleBase,
  FocusOrder,
  GridBase,
  GridCellBase,
  Handler,
  HasBoxStyle,
  HasChild,
  HasChildren,
  HasLayoutBox,
  HasRun,
  IntoChildOf,
  IntoChildren,
  IntoScalar,
  ItemExtent,
  Key,
  KeyEvent,
  KeymapBase,
  LazyColumnBase,
  LazyRowBase,
  Leaf,
  Length,
  LinkBase,
  Listbox,
  ListboxContext,
  ListboxOption,
  Menu,
  MenuContext,
  MenuEntry,
  MenuOption,
  Modifiers,
  MotionBase,
  NamedKey,
  NativeElement,
  NativePicker,
  NodeId,
  Number,
  OneOfBase,
  OverlayBase,
  PaintedBase,
  Part,
  PinnedBase,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerId,
  PointerInfo,
  PointerPhase,
  PointerType,
  Popup,
  Prevent,
  Prevented,
  Radio,
  RadioGroup,
  RadioGroupContext,
  Realization,
  RichTextBase,
  Role,
  Roster,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  Scene,
  ScrollEvent,
  Shortcut,
  Slider,
  SliderRange,
  SpanBase,
  StackBase,
  State,
  StaticLeaf,
  SuspenseBase,
  TOOLTIP_DELAY,
  TOOLTIP_GRACE,
  TextBase,
  TextDecoration,
  TextDecorationStyle,
  TextEntry,
  TextSelection,
  ThemeBase,
  Toggle,
  TokenName,
  TokenRef,
  TokenSet,
  TooltipBase,
  TypeMetrics,
  VisibilityBase,
  WheelEvent,
  WheelUnit,
  When,
  Wraps,
  routing as router,
  typography,
  web::{AsyncViewBase, DeferredBase},
};
pub use skene_macros::{lazy, web_main as main};
pub use skene_reactive as reactive;
pub use tracing;
#[cfg(all(skene_platform = "web", not(skene_server)))]
#[doc(hidden)]
pub use wasm_bindgen;

#[cfg(all(skene_platform = "web", skene_server))]
pub use self::platform::web::Rendered;
#[cfg(skene_platform = "web")]
use crate::platform::web;
#[cfg(skene_platform = "web")]
pub use crate::{
  link_policy::{LinkAnnotation, LinkPolicy},
  media::{chosen_scheme, derive_scheme, set_scheme},
  platform::Platform,
  resource::{LoadState, Resource},
  routing::{install_router, install_static_router},
  task::spawn,
  views::{
    AsyncView,
    BasicButton,
    BasicCheckbox,
    BasicListbox,
    BasicListboxOption,
    BasicMenu,
    BasicMenuItem,
    BasicPicker,
    BasicRadio,
    BasicRadioGroup,
    BasicSlider,
    BasicTextEditor,
    BasicTextField,
    Column,
    Container,
    ContextMenu,
    Control,
    Deferred,
    Enhance,
    ErrorBoundary,
    Flexible,
    Grid,
    GridCell,
    Keymap,
    LazyColumn,
    LazyRow,
    Link,
    OneOf,
    Overlay,
    Painted,
    Pinned,
    RichText,
    Row,
    Span,
    Stack,
    Suspense,
    Text,
    Theme,
    Tooltip,
    Visibility,
  },
};

/// The realization the build selected: the web's, in the browser and on a server
/// render (`--cfg skene_server`).
#[cfg(skene_platform = "web")]
pub type Active = web::Web;

#[cfg(skene_platform = "web")]
skene_foundation::facade!(Active, f32, "logical pixels");

/// Wraps a view that only a client can realize.
///
/// It builds once the page stands [`hydrated`](media::hydrated), so a server renders
/// nothing in its place and a hydrating client adopts that empty place before building
/// it.
///
/// The place for a low-level view that reaches a platform's raw handles. `build`
/// returns the node it realizes, exactly as a view's `build` does.
#[cfg(skene_platform = "web")]
pub fn client_only(build: impl FnOnce() -> NodeId + 'static) -> impl View {
  Enhance::new(media::hydrated()).set_child_builder(build)
}

/// Mounts and runs the application in the browser; a server-rendered page is hydrated
/// in place.
#[cfg(all(skene_platform = "web", not(skene_server)))]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  web::boot_with_dev_state(move || {
    web::launch(app);

    tracing::info!("application mounted");
  });
}

/// Renders the application, for a server-rendering platform.
///
/// It builds the tree, drives its resources until they settle, and serializes the
/// result plus the resource state for hydration; see [`platform::web::render_to_string`].
#[cfg(all(skene_platform = "web", skene_server))]
pub fn render_to_string<V, F>(app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  web::render_to_string(app)
}

/// Renders the application for the given request path: the router (when installed via
/// [`install_router`]) resolves against `path`, and the render proceeds as
/// [`render_to_string`].
#[cfg(all(skene_platform = "web", skene_server))]
pub fn render_to_string_at<V, F>(path: &str, app: F) -> Rendered
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  web::set_request_path(path);

  render_to_string(app)
}

#[cfg(all(skene_platform = "web", skene_server))]
pub use skene_dev::{DEV_VARIABLE, HINTS_VARIABLE, NONCE_VARIABLE, ROUTES_MARKERS};

/// What `app-server --help` prints.
#[cfg(all(skene_platform = "web", skene_server))]
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
/// The app's server build is a plain command: `app-server /some/path` prints the
/// document.
#[cfg(all(skene_platform = "web", skene_server))]
pub fn render_main<V, F>(bundle: &str, app: F)
where
  F: Fn() -> V + Clone + 'static,
  V: View,
{
  use std::io::Write as _;

  web::install_logging();
  web::set_client_bundle(Some(bundle));
  web::set_dev_loop(std::env::var_os(DEV_VARIABLE).is_some());

  if matches!(std::env::args().nth(1).as_deref(), Some("--help" | "-h")) {
    println!("{USAGE}");

    return;
  }

  if std::env::args().nth(1).as_deref() == Some("--serve") {
    serve_renders(app);

    return;
  }

  web::set_script_nonce(std::env::var(NONCE_VARIABLE).ok().as_deref());

  if let Ok(hints) = std::env::var(HINTS_VARIABLE) {
    let headers = hints
      .lines()
      .filter_map(|line| line.split_once(':'))
      .map(|(name, value)| (name.trim(), value.trim()));

    web::set_render_hints(web::RenderHints::from_headers(headers));
  }

  let argument = std::env::args().nth(1).unwrap_or_else(|| "/".to_owned());

  if argument == "--routes" {
    let mut output = protocol_output();
    let _ = render_to_string_at("/", app);
    let [first, last] = ROUTES_MARKERS;
    let routes = web::recorded_static_routes().unwrap_or_else(|| vec!["/".to_owned()]);

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

    web::set_request_path(&path);
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
/// `<style>` delta for the classes it introduces; the last closes the document. The
/// chunks concatenate, in order, into the markup [`render_to_string`] would produce,
/// with the deltas beside the content they style: a client with no JavaScript receives
/// a correct, styled document progressively.
#[derive(Debug)]
#[cfg(all(skene_platform = "web", skene_server))]
#[must_use]
pub struct Stream {
  chunks: Vec<String>,
}

#[cfg(all(skene_platform = "web", skene_server))]
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
/// boundary's slow content: the returned [`Stream`]'s chunks, in document order with
/// their styles, are written to the response as they are produced, so the client sees
/// the page progressively, without waiting on the slowest data and without JavaScript.
#[cfg(all(skene_platform = "web", skene_server))]
pub fn render_to_stream<V, F>(app: F) -> Stream
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  let mut chunks = Vec::new();
  let mut opening = String::new();
  let mut closing = String::new();

  web::render_to_stream_with(app, |piece| match piece {
    web::StreamPiece::Shell {
      styles,
      head,
      language,
    } => opening = web::document_opening(language, head, styles),
    web::StreamPiece::Chunk(chunk) => chunks.push(chunk),
    web::StreamPiece::Tail(state) => closing = web::document_closing(&state),
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
#[cfg(all(skene_platform = "web", skene_server))]
pub fn render_page_stream<V, F>(app: F, mut sink: impl FnMut(&str))
where
  F: FnOnce() -> V + 'static,
  V: View,
{
  web::render_to_stream_with(app, |piece| match piece {
    web::StreamPiece::Shell {
      styles,
      head,
      language,
    } => sink(&web::document_opening(language, head, styles)),
    web::StreamPiece::Chunk(chunk) => sink(&chunk),
    web::StreamPiece::Tail(state) => sink(&web::document_closing(&state)),
  });
}

/// Renders requests read from stdin until it closes or sends one malformed, one page
/// per request to stdout.
///
/// A request and a response are each `Content-Length: <bytes>` on a line, an empty
/// line, and that many bytes. A request's bytes are the path on a line, percent-encoded
/// as a URL carries it, then one `Name: value` per line: the client hints, and the
/// nonce the page's inline scripts carry as `Script-Nonce`; a response's are the
/// complete page. Each is for that request alone, and a request without them renders
/// against the defaults.
#[cfg(all(skene_platform = "web", skene_server))]
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

    web::set_script_nonce(nonce);
    web::set_render_hints(web::RenderHints::from_headers(fields()));

    let page = render_to_string_at(path, app.clone()).page();

    let _ = write!(output, "Content-Length: {}\n\n{page}", page.len());
    let _ = output.flush();
  }
}

/// The stream a protocol writes to: the process's standard output, taken aside, and
/// standard output itself pointed at standard error, so what the app prints stays out of
/// the protocol's way. Standard output as it is where it cannot be taken aside.
#[cfg(all(skene_platform = "web", skene_server))]
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
#[cfg(all(skene_platform = "web", skene_server))]
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
