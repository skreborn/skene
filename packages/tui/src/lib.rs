//! # skene-tui
//!
//! The terminal facade and its driver: a terminal app's single dependency, equal in
//! standing to `skene-gui`. It fixes the scalar (cells, `u32`), names the driver
//! ([`ActiveDriver`]: [`Tui`]) and aliases every built-in view to it, so app code names
//! no platform. The medium's own contribution is
//! [`Typography`], the [`media`] module, the terminal's decoration
//! ([`TuiDecoration`], [`TuiText`]), the escape trait a low-level view is
//! written against ([`CellPainter`], painting through [`Cells`]) and the entry points:
//! [`launch`] in a terminal, [`render_to_text`] for a headless test.
//!
//! Layout is taffy at `1.0 = 1 cell`, the effective style cached per node and written
//! to the layout tree only on change; frames paint into a cell buffer. Interaction
//! follows a browser: `Tab`/`Shift-Tab` move focus across the controls, the behaviors
//! own their keys, a click fires on press and release over one target, and `Ctrl-C`
//! quits. Every dispatch runs inside a reactive batch.
//!
//! Host-only: on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

pub mod media;
pub mod prelude;

mod cells;
mod detect;
mod dev;
mod driver;
mod ext;
mod field;
mod frame;
mod interaction;
mod layout;
mod motion;
mod paint;
mod resolve;
mod routing;
mod selection;
mod store;
mod terminal;

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
  PointerButton,
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
pub use skene_reactive::{self as reactive, LoadState, Resource};
pub use skene_router as router;
/// The terminal medium's shared vocabulary: the style extension, its glyphs and values,
/// and the typography set.
pub use skene_tui_core::{
  BOLD_WEIGHT,
  BorderGlyphs,
  TuiBoxFields,
  TuiExt,
  TuiProperty,
  TuiRunFields,
  TuiValue,
  Typography,
  tui_fields,
};
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

pub(crate) use crate::driver::{try_with_store, with_store};
pub use crate::{
  cells::{CellPainter, Cells},
  dev::{adopt_restoration, notice_view as dev_notice_view},
  driver::{Tui, handle, install, mount, uninstall},
  ext::{TuiDecoration, TuiText},
  media::{derive_scheme, set_scheme, stored_scheme},
  routing::install_router,
  store::Handle,
  terminal::{
    current_cursor,
    current_title,
    focused_node,
    launch,
    layout_generation,
    repainted_rows,
    set_style_calls,
    style_resolutions,
    take_copied,
    take_opened,
    trigger_clipboard_support,
    trigger_color_scheme,
    trigger_frame,
    trigger_key,
    trigger_mouse_click,
    trigger_mouse_drag,
    trigger_mouse_move,
    trigger_mouse_press,
    trigger_mouse_release,
    trigger_mouse_right_click,
    trigger_mouse_scroll,
    trigger_reduced_motion,
    trigger_resize,
  },
};

/// The driver the build selected: the terminal driver, on every host target.
pub type ActiveDriver = Tui;

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
/// A single-line text entry; see [`skene_views::RawTextField`].
pub type RawTextField<C = NoChild> = skene_views::RawTextField<ActiveDriver, C>;
/// A multi-line text entry; see [`skene_views::RawTextEditor`].
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
/// A handle reporting a built view's measured size; see [`skene_views::SizeHandle`].
pub type SizeHandle = skene_views::SizeHandle<ActiveDriver>;
/// A scrolling keyed list building only the slice in view; see
/// [`skene_views::LazyColumn`].
pub type LazyColumn = skene_views::LazyColumn<ActiveDriver>;
/// The closure building a view's child under the context the view provides; see
/// [`skene_views::ChildBuilder`].
pub type ChildBuilder = skene_views::ChildBuilder<ActiveDriver>;

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
/// A border: one cell of glyphs in a color; see [`skene_core::Border`].
pub type Border = skene_core::Border<u32>;
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
#[expect(clippy::type_complexity)]
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

/// Polls every ready task, as the interactive loop does between events; `true` when any
/// ran. For a headless test driving async work.
#[must_use]
pub fn pump_tasks() -> bool {
  skene_core::executor::pump()
}

/// Runs `future` as a task, until it finishes or the current scope is disposed.
///
/// Owner-bound on purpose: a task started while building a view ends with that view—or
/// with any ancestor of it—rather than waking later to write into a tree that is gone.
/// The future runs on the terminal thread, pumped by the event loop between events, so
/// it may touch signals and views directly; cancellation takes effect at its next poll.
#[inline(always)]
pub fn spawn(future: impl Future<Output = ()> + 'static) {
  skene_reactive::spawn(future);
}

/// The erased token definition the terminal stores per token; see
/// [`skene_core::TokenValue`].
pub type TokenValue = skene_core::TokenValue<u32>;
