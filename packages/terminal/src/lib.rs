//! # skene-terminal
//!
//! The terminal facade and its platform: a terminal app's single dependency, equal in
//! standing to `skene-web`. It fixes the scalar (cells, `u32`), names the platform
//! ([`ActivePlatform`]: [`Terminal`]) and aliases every built-in view to it, so app code names
//! no platform. The medium's own contribution is
//! [`Typography`], the [`media`] module, the terminal's decoration on its decorated
//! views (the border's glyphs, a shadow, scrollbars), the escape trait a low-level view is
//! written against ([`CellPainter`], painting through [`Cells`]) and the entry points:
//! [`launch`] in a terminal—[`Launch`] configuring what the app takes from it, the
//! clipboard operations and the mouse—and [`testing`] for a headless test.
//!
//! Layout is taffy at `1.0 = 1 cell`, the effective style cached per node and written
//! to the layout tree only on change; frames paint into a cell buffer. Interaction
//! follows a browser: `Tab`/`Shift-Tab` move focus across the controls, the behaviors
//! own their keys, a click fires on press and release over one target, `Ctrl-C`,
//! `Ctrl-X` and `Ctrl-V` copy, cut and paste where the launch offers them, and
//! `Ctrl-Q` quits. Every dispatch runs inside a reactive batch.
//!
//! Host-only: on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

pub mod media;
pub mod prelude;

mod cells;
mod detect;
mod dev;
mod device;
mod escape;
mod event_loop;
mod field;
mod frame;
mod interaction;
mod launch;
mod layout;
mod motion;
mod paint;
mod parse;
mod platform;
mod records;
mod resolve;
mod routing;
mod selection;
pub mod testing;
mod text;
mod tty;
mod views;
pub mod window;

/// The terminal medium's shared vocabulary: the style extension, its glyphs and values,
/// and the typography set.
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
pub use skene_foundation::{
  Activate,
  AnyKind,
  BasicPickerBase,
  Behavior,
  Bubbling,
  Canvas,
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
  ControlChild,
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
  IntoBrush,
  IntoChildOf,
  IntoChildren,
  ItemExtent,
  Key,
  KeyEvent,
  KeymapBase,
  LazyColumnBase,
  LazyRowBase,
  Leaf,
  Length,
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
  NoChild,
  NodeId,
  Number,
  OverlayBase,
  PaintedBase,
  Painter,
  Painting,
  Part,
  PinnedBase,
  Platform,
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
  RichTextBase,
  Role,
  Roster,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  Scene,
  ScrollEvent,
  Shortcut,
  ShowBase,
  Slider,
  SpanBase,
  StackBase,
  State,
  StaticLeaf,
  SuspenseBase,
  SwitchBase,
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
  WheelEvent,
  WheelUnit,
  When,
  Wraps,
  kurbo,
  peniko,
  routing as router,
  typography,
};
pub use skene_reactive::{self as reactive, LoadState, Resource};

pub(crate) use crate::platform::{try_with_records, with_records};
pub use crate::{
  cells::{CellPainter, Cells},
  dev::{adopt_restoration, notice_view as dev_notice_view},
  device::{Device, Note, WindowSize},
  launch::{Clipboard, Launch, MOUSE_KEY, install, launch},
  media::{chosen_scheme, derive_scheme, set_scheme},
  paint::{CellDecoration, Picture},
  platform::{Terminal, mount, uninstall},
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
    Overlay,
    Painted,
    Pinned,
    RichText,
    Row,
    Show,
    Span,
    Stack,
    Suspense,
    Switch,
    Text,
    Theme,
    Tooltip,
  },
};

/// The platform the build selected: the terminal platform, on every host target.
pub type ActivePlatform = Terminal;

skene_foundation::facade!(ActivePlatform, u32, "cells");

/// Runs `future` as a task of the current scope until it finishes, the scope goes or
/// the returned handle cancels it; once it ends it is never polled again.
///
/// A task started while building a view ends with that view, or with any ancestor of
/// it. The future runs on the terminal thread, pumped by the event loop between events
/// and at once when woken from another thread, so it may touch signals and views
/// directly.
pub fn spawn(future: impl Future<Output = ()> + 'static) -> skene_reactive::TaskHandle {
  skene_reactive::spawn(future)
}
