//! # skene-tui
//!
//! The terminal facade and its platform: a terminal app's single dependency, equal in
//! standing to `skene-gui`. It fixes the scalar (cells, `u32`), names the platform
//! ([`ActivePlatform`]: [`Tui`]) and aliases every built-in view to it, so app code names
//! no platform. The medium's own contribution is
//! [`Typography`], the [`media`] module, the terminal's decoration
//! ([`TuiDecoration`], [`TuiText`]), the escape trait a low-level view is
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
mod ext;
mod field;
mod frame;
mod interaction;
mod launch;
mod layout;
mod motion;
mod paint;
mod parse;
mod platform;
mod resolve;
mod routing;
mod selection;
mod store;
mod terminal;
pub mod testing;
mod text;
mod tty;

/// The terminal medium's shared vocabulary: the style extension, its glyphs and values,
/// and the typography set.
pub use skene_common::tui::{
  BOLD_WEIGHT,
  BorderGlyphs,
  ScrollbarGlyphs,
  TuiBoxFields,
  TuiProperty,
  TuiRunFields,
  TuiStyle,
  TuiValue,
  Typography,
  tui_fields,
};
pub use skene_common::{
  Activate,
  Behavior,
  Canvas,
  ControlChild,
  CornerRadius,
  IntoBrush,
  ItemExtent,
  Listbox,
  ListboxContext,
  ListboxOption,
  Menu,
  MenuContext,
  MenuEntry,
  MenuOption,
  NativePicker,
  Painter,
  Painting,
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
  kurbo,
  peniko,
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
pub use skene_reactive::{self as reactive, LoadState, Resource};

pub(crate) use crate::platform::{try_with_store, with_store};
pub use crate::{
  cells::{CellPainter, Cells},
  dev::{adopt_restoration, notice_view as dev_notice_view},
  device::{Device, Note, WindowSize},
  ext::{TuiDecoration, TuiText},
  launch::{Clipboard, Launch, MOUSE_KEY, install, launch},
  media::{derive_scheme, set_scheme, stored_scheme},
  paint::{CellDecoration, Picture},
  platform::{Tui, mount, uninstall},
  routing::install_router,
  text::CellScale,
};

/// The platform the build selected: the terminal platform, on every host target.
pub type ActivePlatform = Tui;

skene_common::facade!(ActivePlatform, u32, "cells");

/// Sets the terminal title, emitted via OSC 2; terminals without support ignore it.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  let title = title.into();

  with_store(move |store| {
    store.title = Some(title);
    store.title_dirty = true;
  });
}

/// Derives the terminal title, re-emitted whenever it changes.
#[inline(always)]
pub fn derive_title(title: impl Fn() -> String + 'static) {
  skene_reactive::Effect::new(move |_: Option<()>| set_title(title()));
}

/// Runs `future` as a task of the current scope until it finishes, the scope goes or
/// the returned handle cancels it; once it ends it is never polled again.
///
/// A task started while building a view ends with that view, or with any ancestor of
/// it. The future runs on the terminal thread, pumped by the event loop between events
/// and at once when woken from another thread, so it may touch signals and views
/// directly.
#[inline(always)]
pub fn spawn(future: impl Future<Output = ()> + 'static) -> skene_reactive::TaskHandle {
  skene_reactive::spawn(future)
}
