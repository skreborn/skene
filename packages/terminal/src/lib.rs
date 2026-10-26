//! # skene-terminal
//!
//! The terminal facade and its platform: a terminal app's single dependency, equal in
//! standing to `skene-web`. It fixes the scalar (cells, `i32`), names the platform
//! ([`Active`]: [`Terminal`]) and aliases every built-in view to it, so app code names
//! no platform. The medium's own contribution is [`Typography`], the [`media`] module,
//! the terminal's decoration on its decorated views (the border's glyphs, a shadow,
//! scrollbars) and the entry points: [`launch`] in a terminal—[`Launch`] configuring
//! what the app takes from it, the clipboard operations and the mouse—and [`testing`]
//! for an in-process test. [`Platform`] is the build's platform as a const, and
//! [`platform::terminal`] the terminal's own vocabulary: the escape trait a low-level
//! view is written against ([`CellPainter`](platform::terminal::CellPainter), painting
//! through [`Cells`](platform::terminal::Cells)). The [`behavior`] module holds what a
//! [`Control`] runs over: the shipped behaviors, the trait a library's own implements and
//! the handle it acts through.
//!
//! Layout is taffy at `1.0 = 1 cell`; frames paint into a cell buffer. Interaction
//! follows a browser: `Tab`/`Shift-Tab` move focus across the controls, the behaviors
//! own their keys, a click fires on press and release over one target, `Ctrl-C`,
//! `Ctrl-X` and `Ctrl-V` copy, cut and paste where the launch offers them, and `Ctrl-Q`
//! quits. Every dispatch runs inside a reactive batch.
//!
//! A length is a plain number of cells, an `i32` or an `f32`, wherever one is taken: a
//! setter, a derived value, a condition's variant, a token's definition and every
//! constructor measured in lengths ([`IntoScalar`]). An `f32` rounds to the nearest cell
//! at the setter, and a run's font size keeps its fraction of a cell.
//!
//! ```
//! use skene_terminal::{Length, prelude::*};
//!
//! let gutter =
//!   Token::<Length>::new(TokenRef::new("gutter"), 8).define_when(Media::min_width(90), 16);
//! let _ = Column::new()
//!   .set_spacing(Spacing::from(8).when(Media::min_width(90), 16))
//!   .set_padding(Inset::new(0, 8, 0, 8))
//!   .set_children((
//!     Row::new()
//!       .set_spacing(Spacing::new(8, -4))
//!       .set_padding(gutter.reference()),
//!     Container::new().set_width(16).set_child(Text::new("Whole")),
//!     Container::new()
//!       .set_width(16.5)
//!       .set_child(Text::new("Rounded")),
//!     Container::new()
//!       .derive_width(|| 16)
//!       .set_child(Text::new("Derived")),
//!   ));
//!
//! assert_eq!(Spacing::all(1.5), Spacing::all(2));
//! assert_eq!(Dimension::from(1.5), Dimension::Length(2));
//! ```
//!
//! A build names the terminal by `--cfg skene_platform="terminal"`, on the host's own
//! target. A build that names no platform fails with a `compile_error!` saying so, and
//! every other platform's build compiles the crate empty.

#![cfg(any(
  skene_platform = "terminal",
  not(any(
    skene_platform = "android",
    skene_platform = "ios",
    skene_platform = "linux",
    skene_platform = "macos",
    skene_platform = "terminal",
    skene_platform = "web",
    skene_platform = "windows",
  )),
))]

#[cfg(not(skene_platform = "terminal"))]
compile_error!(
  "skene-terminal builds for the platform its build names: `--cfg \
   skene_platform=\"terminal\"` in the rustflags of the host's target; the \
   `.cargo/config.toml` `skene new` writes names it for plain cargo, and `skene build` \
   and `skene run` name it themselves"
);

#[cfg(skene_platform = "terminal")]
pub mod behavior;
#[cfg(skene_platform = "terminal")]
pub mod media;
#[cfg(skene_platform = "terminal")]
pub mod platform;
#[cfg(skene_platform = "terminal")]
pub mod prelude;
#[cfg(skene_platform = "terminal")]
pub mod testing;
#[cfg(skene_platform = "terminal")]
pub mod window;

#[cfg(skene_platform = "terminal")]
mod cells;
#[cfg(skene_platform = "terminal")]
mod detect;
#[cfg(skene_platform = "terminal")]
mod dev;
#[cfg(skene_platform = "terminal")]
mod device;
#[cfg(skene_platform = "terminal")]
mod engine;
#[cfg(skene_platform = "terminal")]
mod escape;
#[cfg(skene_platform = "terminal")]
mod event_loop;
#[cfg(skene_platform = "terminal")]
mod field;
#[cfg(skene_platform = "terminal")]
mod frame;
#[cfg(skene_platform = "terminal")]
mod interaction;
#[cfg(skene_platform = "terminal")]
mod launch;
#[cfg(skene_platform = "terminal")]
mod layout;
#[cfg(skene_platform = "terminal")]
mod motion;
#[cfg(skene_platform = "terminal")]
mod paint;
#[cfg(skene_platform = "terminal")]
mod parse;
#[cfg(skene_platform = "terminal")]
mod realize;
#[cfg(skene_platform = "terminal")]
mod records;
#[cfg(skene_platform = "terminal")]
mod resolve;
#[cfg(skene_platform = "terminal")]
mod routing;
#[cfg(skene_platform = "terminal")]
mod selection;
#[cfg(skene_platform = "terminal")]
mod text;
#[cfg(skene_platform = "terminal")]
mod tty;
#[cfg(skene_platform = "terminal")]
mod views;

extern crate self as skene_terminal;

/// The terminal medium's shared vocabulary: the style extension, its glyphs and values,
/// and the typography set.
#[cfg(skene_platform = "terminal")]
pub use skene_foundation::terminal::{
  BOLD_WEIGHT,
  BorderGlyphs,
  ScrollbarGlyphs,
  TerminalBoxFields,
  TerminalProperty,
  TerminalRunFields,
  TerminalStyle,
  TerminalValue,
  Typography,
  terminal_fields,
};
#[cfg(skene_platform = "terminal")]
pub use skene_foundation::{
  AnyChoice,
  AnyChosen,
  AnyKind,
  Autofill,
  BasicListboxBase,
  BasicPickerBase,
  BasicRadioGroupBase,
  Bubbling,
  Canvas,
  Capitalization,
  Children,
  Choice,
  Chosen,
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
  HasControl,
  HasLayoutBox,
  HasRun,
  IntoBrush,
  IntoChildOf,
  IntoChildren,
  IntoScalar,
  ItemExtent,
  Key,
  KeyEvent,
  Keyboard,
  KeyboardType,
  KeymapBase,
  LazyColumnBase,
  LazyRowBase,
  Leaf,
  Length,
  LinkBase,
  MenuEntry,
  Modifiers,
  MotionBase,
  NamedKey,
  NativeElement,
  NodeId,
  Number,
  OneOfBase,
  OverlayBase,
  PaintedBase,
  Painter,
  Painting,
  Part,
  PinnedBase,
  PointerButton,
  PointerButtons,
  PointerEvent,
  PointerId,
  PointerInfo,
  PointerPhase,
  PointerType,
  Prevent,
  Prevented,
  RichTextBase,
  Role,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  Scene,
  ScrollEvent,
  Shortcut,
  SliderBase,
  SpanBase,
  StackBase,
  State,
  StaticLeaf,
  SubmitLabel,
  SuspenseBase,
  TOOLTIP_DELAY,
  TOOLTIP_GRACE,
  TextBase,
  TextDecoration,
  TextDecorationStyle,
  TextEntryBase,
  TextSelection,
  ThemeBase,
  Timer,
  ToggleBase,
  TokenName,
  TokenRef,
  TokenScope,
  TokenSet,
  TooltipBase,
  TypeMetrics,
  VisibilityBase,
  WheelEvent,
  WheelUnit,
  When,
  Wraps,
  kurbo,
  peniko,
  routing as router,
  typography,
};
#[cfg(skene_platform = "terminal")]
pub use skene_macros::terminal_main as main;
#[cfg(skene_platform = "terminal")]
pub use skene_reactive::{self as reactive, LoadState, Resource};

#[cfg(skene_platform = "terminal")]
pub use crate::{
  dev::{adopt_restoration, notice_view as dev_notice_view},
  device::{Device, Note, WindowSize},
  launch::{Clipboard, Launch, MOUSE_KEY, install, launch},
  media::{chosen_scheme, derive_scheme, set_scheme},
  paint::{CellDecoration, Picture},
  realize::{mount, uninstall},
  routing::install_router,
  text::CellScale,
  views::{
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
    Button,
    Checkbox,
    Column,
    Container,
    ContextMenu,
    Control,
    Enhance,
    ErrorBoundary,
    Flexible,
    Grid,
    GridCell,
    Keymap,
    LazyColumn,
    LazyRow,
    Link,
    Listbox,
    ListboxOption,
    OneOf,
    Overlay,
    Painted,
    Picker,
    Pinned,
    Radio,
    RadioGroup,
    RichText,
    Row,
    Slider,
    Span,
    Stack,
    Suspense,
    Text,
    TextEditor,
    TextField,
    Theme,
    Toggle,
    Tooltip,
    Visibility,
  },
};
#[cfg(skene_platform = "terminal")]
pub(crate) use crate::{
  platform::terminal::Terminal,
  realize::{try_with_records, with_records},
};

/// The realization the build names: the terminal's.
#[cfg(skene_platform = "terminal")]
pub type Active = Terminal;

#[cfg(skene_platform = "terminal")]
skene_foundation::facade!(Active, i32, "cells");

/// A terminal platform: what a build of this facade names by
/// `--cfg skene_platform="<name>"`.
///
/// ```
/// use skene_terminal::Platform;
///
/// const NAME: &str = Platform::CURRENT.name();
///
/// assert_eq!(NAME, "terminal");
/// ```
#[cfg(skene_platform = "terminal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Platform {
  /// A terminal, through its device.
  Terminal,
}

#[cfg(skene_platform = "terminal")]
impl Platform {
  /// The platform the build names.
  pub const CURRENT: Self = Self::Terminal;

  /// The platform's name, as its build names it in `skene_platform`.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Terminal => "terminal",
    }
  }
}

/// Runs `future` as a task of the current scope until it finishes, the scope goes or
/// the returned handle cancels it; once it ends it is never polled again.
///
/// A task started while building a view ends with that view, or with any ancestor of
/// it. The future runs on the terminal thread, pumped by the event loop between events
/// and at once when woken from another thread, so it may touch signals and views
/// directly.
#[cfg(skene_platform = "terminal")]
pub fn spawn(future: impl Future<Output = ()> + 'static) -> skene_reactive::TaskHandle {
  skene_reactive::spawn(future)
}

/// Runs `action` once `delay` has passed on the platform's clock, unless the returned
/// [`Timer`] is [`cancel`]led first.
///
/// The platform's clock as an app reaches it, the one a built-in's delay waits on (a
/// tooltip's hover delay): the event loop's, the action running on the terminal thread
/// once a frame reads it due, so it may touch signals and views directly. An in-process
/// test steps the clock by hand ([`trigger_frame`](testing::trigger_frame)).
#[cfg(skene_platform = "terminal")]
pub fn schedule(delay: std::time::Duration, action: impl FnOnce() + 'static) -> Timer {
  <Active as skene_foundation::Realization>::schedule(delay, Box::new(action))
}

/// Drops the action `timer` names, if it has not run.
#[cfg(skene_platform = "terminal")]
#[inline(always)]
pub fn cancel(timer: Timer) {
  <Active as skene_foundation::Realization>::cancel(timer);
}
