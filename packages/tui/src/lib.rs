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
//! clipboard operations and the mouse—and [`render_to_text`] for a headless test.
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
mod text;
mod tty;
pub mod user;

/// The terminal medium's shared vocabulary: the style extension, its glyphs and values,
/// and the typography set.
pub use skene_common::tui::{
  BOLD_WEIGHT,
  BorderGlyphs,
  ScrollbarGlyphs,
  TuiBoxFields,
  TuiExt,
  TuiProperty,
  TuiRunFields,
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
  ListItem,
  Listbox,
  ListboxGroup,
  Menu,
  MenuGroup,
  MenuItem,
  MenuOption,
  NativePicker,
  Painter,
  Painting,
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
  ext::{TuiDecoration, TuiText},
  launch::{Clipboard, Launch, MOUSE_KEY, install, launch},
  media::{derive_scheme, set_scheme, stored_scheme},
  paint::{CellDecoration, Picture},
  platform::{Tui, mount, uninstall},
  routing::install_router,
  terminal::{
    accessible_name,
    current_cursor,
    current_title,
    find_all_by_role,
    find_all_unnamed,
    find_by_role,
    find_by_text,
    focused_node,
    frame_pending,
    layout_generation,
    quit_requested,
    repainted_rows,
    set_style_calls,
    style_resolutions,
    take_copied,
    take_opened,
    take_paste_requested,
    trigger_answering_clipboard,
    trigger_clipboard_reply,
    trigger_color_scheme,
    trigger_frame,
    trigger_grapheme_clustering,
    trigger_graphics,
    trigger_key,
    trigger_key_event,
    trigger_key_releases,
    trigger_key_up,
    trigger_mouse_click,
    trigger_mouse_drag,
    trigger_mouse_drag_within,
    trigger_mouse_move,
    trigger_mouse_press,
    trigger_mouse_release,
    trigger_mouse_right_click,
    trigger_mouse_scroll,
    trigger_paste,
    trigger_reduced_motion,
    trigger_resize,
    trigger_terminal_colors,
    trigger_text_sizing,
  },
  text::CellScale,
};

/// The platform the build selected: the terminal platform, on every host target.
pub type ActivePlatform = Tui;

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
/// A single-line text entry; see [`skene_common::RawTextField`].
pub type RawTextField<C = NoChild> = skene_common::RawTextField<ActivePlatform, C>;
/// A multi-line text entry; see [`skene_common::RawTextEditor`].
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
/// Keyboard shortcuts over the view it serves; see [`skene_common::Keymap`].
pub type Keymap<C = NoChild> = skene_common::Keymap<ActivePlatform, C>;
/// A menu; see [`skene_common::RawMenu`].
pub type RawMenu<C = NoChild> = skene_common::RawMenu<ActivePlatform, C>;
/// One item within a menu; see [`skene_common::RawMenuItem`].
pub type RawMenuItem<C = NoChild> = skene_common::RawMenuItem<ActivePlatform, C>;
/// A handle naming the node a view built; see [`skene_common::NodeHandle`].
pub type NodeHandle = skene_common::NodeHandle<ActivePlatform>;
/// A box drawn by a paint callback, through the terminal's graphics protocol where
/// the terminal proved one; see [`skene_common::Painted`].
pub type Painted = skene_common::Painted<ActivePlatform>;
/// A scrolling keyed list building only the slice in view; see
/// [`skene_common::LazyColumn`].
pub type LazyColumn = skene_common::LazyColumn<ActivePlatform>;
/// A scrolling keyed row building only the slice in view; see
/// [`skene_common::LazyRow`].
pub type LazyRow = skene_common::LazyRow<ActivePlatform>;
/// The closure building a view's child under the context the view provides; see
/// [`skene_common::ChildBuilder`].
pub type ChildBuilder = skene_common::ChildBuilder<ActivePlatform>;

/// A condition on the environment, in cells; see [`skene_core::Media`].
pub type Media = skene_core::Media<u32>;
/// A condition a property varies under; see [`skene_core::Condition`].
pub type Condition = skene_core::Condition<u32>;
/// A test of a container's size, in cells; see [`skene_core::ContainerQuery`].
pub type ContainerQuery = skene_core::ContainerQuery<u32>;
/// A property's value: a leaf under conditions; see [`skene_core::Value`].
pub type Value<T> = skene_core::Value<T, Condition>;
/// A token's definition; see [`skene_core::Definition`].
pub type Definition<T> = skene_core::Definition<T, u32>;
/// A token as a palette holds it; see [`skene_core::Token`].
pub type Token<T> = skene_core::Token<T, u32>;
/// Spacing on the four edges of a box, in cells.
pub type Inset<T = u32> = skene_core::Inset<T>;
/// A dimension, in cells or as a fraction of the parent.
pub type Dimension = skene_core::Dimension<u32>;
/// Per-axis spacing between children, in cells.
pub type Spacing = skene_core::Spacing<u32>;
/// A 2D size, in cells.
pub type Size = skene_core::Size<u32>;
/// A point in the cell coordinate space.
pub type Point = skene_core::Point<u32>;
/// A positioned, sized rectangle, in cells.
pub type Rect = skene_core::Rect<u32>;

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

/// Lays the mounted tree out at `width` x `height` cells and paints it to plain text:
/// the headless render a test asserts against.
#[must_use]
pub fn render_to_text(width: u16, height: u16) -> String {
  frame::render(width, height).to_text()
}

/// The painted frame's caret cell at `width` x `height`—where the terminal's bar
/// cursor shows—for a headless test asserting caret placement.
#[must_use]
pub fn render_to_caret(width: u16, height: u16) -> Option<(u16, u16)> {
  frame::render(width, height).caret
}

/// The painted frame's colors at `width` x `height`: each cell's foreground and
/// background, row-major, for a headless test asserting paint.
#[must_use]
#[expect(
  clippy::type_complexity,
  reason = "a cell's two colors, plainer spelled out than behind an alias"
)]
pub fn render_to_colors(
  width: u16,
  height: u16,
) -> Vec<(Option<(u8, u8, u8)>, Option<(u8, u8, u8)>)> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| (cell.foreground, cell.background))
    .collect()
}

/// The painted frame's bold cells at `width` x `height`, row-major, for a headless test
/// asserting a run's or an entry's weight.
#[must_use]
pub fn render_to_bold(width: u16, height: u16) -> Vec<bool> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.bold)
    .collect()
}

/// The painted frame's reverse-video cells at `width` x `height`, row-major, for a
/// headless test asserting the emphasis a colorless selection or a colorless focused
/// link paints in.
#[must_use]
pub fn render_to_reverse(width: u16, height: u16) -> Vec<bool> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.reverse)
    .collect()
}

/// The painted frame's decoration lines at `width` x `height`, row-major, for a
/// headless test asserting an underline's style and color, an overline or a line
/// through.
#[must_use]
pub fn render_to_decorations(width: u16, height: u16) -> Vec<CellDecoration> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.decoration)
    .collect()
}

/// The pictures the painted frame at `width` x `height` places, for a headless test
/// asserting what a terminal with graphics shows; none until `trigger_graphics`.
#[must_use]
pub fn render_to_pictures(width: u16, height: u16) -> Vec<Picture> {
  frame::render(width, height)
    .pictures
    .iter()
    .map(Picture::from)
    .collect()
}

/// The painted frame's text scales at `width` x `height`, row-major, for a headless
/// test asserting a run's size: a glyph's cell carries its scale, and the other cells
/// of its block carry one.
#[must_use]
pub fn render_to_scales(width: u16, height: u16) -> Vec<CellScale> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.scale)
    .collect()
}

/// Polls every ready task as one turn, as the interactive loop does between events;
/// `true` when any ran. For a headless test driving async work.
#[must_use]
pub fn pump_tasks() -> bool {
  platform::end_turn();

  let ran = skene_core::executor::pump();

  platform::end_turn();

  ran
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

/// The erased token definition the terminal stores per token; see
/// [`skene_core::TokenValue`].
pub type TokenValue = skene_core::TokenValue<u32>;
