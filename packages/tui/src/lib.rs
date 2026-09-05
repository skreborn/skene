//! # skene-tui
//!
//! The terminal facade and its driver: a terminal app's single dependency, equal in
//! standing to `skene-gui`. It fixes the scalar (cells, `u32`), names the driver
//! ([`ActiveDriver`]: [`Tui`]) and aliases every built-in view and every `Simple*` view
//! to it, so app code names no platform. The medium's own contribution is
//! [`Typography`], [`Glyphs`], the [`media`] module, the terminal's decoration
//! ([`TerminalDecoration`], [`TerminalText`]) and the entry points: [`launch`] in a
//! terminal, [`render_to_text`] for a headless test.
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

mod detect;
#[cfg(feature = "snapshot")]
mod dev;
mod driver;
mod ext;
mod field;
mod frame;
mod glyphs;
mod interaction;
mod layout;
mod paint;
mod resolve;
mod resource;
mod routing;
mod selection;
mod store;
mod task;
mod terminal;
mod token;
mod typography_set;

pub use skene_core as core;
pub use skene_core::{
  Bubbling,
  Color,
  ColorScheme,
  Errors,
  FocusOrder,
  KeyEvent,
  Modifiers,
  PointerButton,
  PointerEvent,
  Prevent,
  Prevented,
  Role,
  ScrollEvent,
  next::{
    AnyKind,
    Children,
    ContainerName,
    ControlKind,
    ControlState,
    Definitions,
    Expr,
    HasChildren,
    IntoChildOf,
    IntoChildren,
    Leaf,
    Length,
    NoChild,
    NodeId,
    Number,
    Part,
    Scene,
    State,
    StaticLeaf,
    TokenName,
    TokenRef,
    TokenSet,
    When,
  },
};
pub use skene_reactive as reactive;
pub use skene_router as router;
pub use skene_simple::next::SimpleVariant;
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

#[cfg(all(feature = "snapshot", feature = "test"))]
pub use crate::dev::{adopt_scroll, notice_view as dev_notice_view};
#[cfg(feature = "snapshot")]
pub(crate) use crate::driver::try_with_store;
#[cfg(feature = "test")]
pub use crate::driver::uninstall;
pub(crate) use crate::driver::with_store;
#[cfg(feature = "test")]
pub use crate::terminal::{
  current_cursor,
  current_title,
  focused_node,
  layout_generation,
  repainted_rows,
  set_style_calls,
  take_copied,
  take_opened,
  trigger_clipboard_support,
  trigger_color_scheme,
  trigger_key,
  trigger_mouse_click,
  trigger_mouse_drag,
  trigger_mouse_move,
  trigger_mouse_press,
  trigger_mouse_release,
  trigger_mouse_right_click,
  trigger_mouse_scroll,
  trigger_resize,
};
pub use crate::{
  driver::{Tui, handle, install, mount},
  ext::{
    BOLD_WEIGHT,
    BorderGlyphs,
    TerminalBoxFields,
    TerminalDecoration,
    TerminalExt,
    TerminalProperty,
    TerminalRunFields,
    TerminalText,
    TerminalValue,
    terminal_fields,
  },
  glyphs::Glyphs,
  media::{derive_scheme, set_scheme, stored_scheme},
  resource::Resource,
  routing::install_router,
  store::Handle,
  task::spawn,
  terminal::launch,
  token::TokenValue,
  typography_set::Typography,
};

/// The driver the build selected: the terminal driver, on every host target.
pub type ActiveDriver = Tui;

/// The named glyph tokens; see [`Glyphs`].
pub mod glyph {
  pub use crate::glyphs::{
    CHECKBOX_CHECKED,
    CHECKBOX_UNCHECKED,
    PICKER_INDICATOR,
    RADIO_CHECKED,
    RADIO_UNCHECKED,
    SLIDER_THUMB,
    SLIDER_TRACK,
  };
}

/// A view on the active driver; see [`skene_core::next::View`].
pub trait View: skene_core::next::View<ActiveDriver> {
  /// Realizes this view and returns its root node.
  #[inline(always)]
  fn build(self) -> NodeId
  where
    Self: Sized,
  {
    skene_core::next::View::build(self)
  }

  /// Erases this view's concrete type.
  #[inline(always)]
  fn into_any(self) -> AnyView
  where
    Self: Sized,
  {
    skene_core::next::View::into_any(self)
  }
}

impl<V: skene_core::next::View<ActiveDriver>> View for V {}

/// A value that can be turned into a view on the active driver, accepted as a child
/// of every kind; see [`skene_core::next::IntoView`] and [`AnyKind`].
pub trait IntoView: skene_core::next::IntoView<ActiveDriver> + AnyKind {
  /// Converts this value into a view.
  #[inline(always)]
  fn into_view(self) -> AnyView
  where
    Self: Sized,
  {
    skene_core::next::IntoView::into_view(self)
  }
}

impl<V: skene_core::next::IntoView<ActiveDriver> + AnyKind> IntoView for V {}

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
pub type AnyView = skene_core::next::AnyView<ActiveDriver>;
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
/// A scrolling keyed list building only the slice in view; see
/// [`skene_views::LazyColumn`].
pub type LazyColumn = skene_views::LazyColumn<ActiveDriver>;
/// The closure building a view's child under the context the view provides; see
/// [`skene_views::ChildBuilder`].
pub type ChildBuilder = skene_views::ChildBuilder<ActiveDriver>;

/// The Simple design language's palette; see [`skene_simple::next::SimplePalette`].
pub type SimplePalette = skene_simple::next::SimplePalette<u32>;
/// One emphasis variant's colors; see [`skene_simple::next::SimpleLook`].
pub type SimpleLook = skene_simple::next::SimpleLook<u32>;
/// See [`skene_simple::next::SimpleButton`].
pub type SimpleButton = skene_simple::next::SimpleButton<ActiveDriver>;
/// See [`skene_simple::next::SimpleCard`].
pub type SimpleCard = skene_simple::next::SimpleCard<ActiveDriver>;
/// See [`skene_simple::next::SimpleCheckbox`].
pub type SimpleCheckbox = skene_simple::next::SimpleCheckbox<ActiveDriver>;
/// See [`skene_simple::next::SimplePicker`].
pub type SimplePicker = skene_simple::next::SimplePicker<ActiveDriver>;
/// See [`skene_simple::next::SimpleRadio`].
pub type SimpleRadio = skene_simple::next::SimpleRadio<ActiveDriver>;
/// See [`skene_simple::next::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = NoChild> = skene_simple::next::SimpleRadioGroup<ActiveDriver, C>;
/// See [`skene_simple::next::SimpleSlider`].
pub type SimpleSlider = skene_simple::next::SimpleSlider<ActiveDriver>;
/// See [`skene_simple::next::SimpleTextEditor`].
pub type SimpleTextEditor = skene_simple::next::SimpleTextEditor<ActiveDriver>;
/// See [`skene_simple::next::SimpleTextField`].
pub type SimpleTextField = skene_simple::next::SimpleTextField<ActiveDriver>;

/// A condition on the environment, in cells; see [`skene_core::next::Media`].
pub type Media = skene_core::next::Media<u32>;
/// A condition a property varies under; see [`skene_core::next::Condition`].
pub type Condition = skene_core::next::Condition<u32>;
/// A test of a container's size, in cells; see [`skene_core::next::ContainerQuery`].
pub type ContainerQuery = skene_core::next::ContainerQuery<u32>;
/// A property's value: a leaf under conditions; see [`skene_core::next::Value`].
pub type Value<T> = skene_core::next::Value<T, Condition>;
/// A token's definition; see [`skene_core::next::Definition`].
pub type Definition<T> = skene_core::next::Definition<T, u32>;
/// A token as a palette holds it; see [`skene_core::next::Token`].
pub type Token<T> = skene_core::next::Token<T, u32>;
/// A border: one cell of glyphs in a color; see [`skene_core::next::Border`].
pub type Border = skene_core::next::Border<u32>;
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
/// changes; see [`skene_core::next::derived`].
#[inline(always)]
pub fn derived(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::next::derived::<ActiveDriver>(build)
}

/// Reactively reconciles the content of the segment `start` begins from a keyed
/// collection; see [`skene_core::next::reconcile`].
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
  skene_core::next::reconcile::<ActiveDriver, T, I, K>(start, items, key, build);
}

/// A view that renders nothing.
#[inline(always)]
#[must_use]
pub fn empty() -> impl View {
  skene_core::next::empty::<ActiveDriver>()
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
#[cfg(feature = "test")]
#[must_use]
pub fn render_to_caret(width: u16, height: u16) -> Option<(u16, u16)> {
  frame::render(width, height).caret
}

/// The painted frame's colors at `width` x `height`: each cell's foreground and
/// background, row-major, for a headless test asserting paint.
#[cfg(feature = "test")]
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
#[cfg(feature = "test")]
#[must_use]
pub fn pump_tasks() -> bool {
  task::pump()
}
