//! # skene-tui
//!
//! The terminal facade: the single dependency a terminal app pulls in, equal in standing
//! to `skene-gui` and sharing the same spine (the reactive engine, the view trait, the
//! reconciliation machinery)—but with its own vocabulary, typed to what a terminal
//! renders: lengths are `u32` cells, text is a color and a `bool` bold, borders are one
//! cell of box-drawing characters. What the medium lacks, the types lack.
//!
//! Layout is taffy at `1.0 = 1 cell`; frames paint into a cell buffer—to the terminal
//! through [`launch`], or to plain text through [`render_to_text`] for headless tests.
//! Interaction is keyboard-first: `Tab`/`Shift-Tab` move focus across inputs and
//! clickable boxes, `Enter` clicks the focused node, printable keys edit the focused
//! input, and `Ctrl-C` quits. Every dispatch runs inside a reactive batch.
//!
//! Host-only: on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

pub mod prelude;

mod button;
mod checkbox;
mod container;
mod containers;
mod control;
mod detect;
#[cfg(feature = "snapshot")]
mod dev;
mod events;
mod focus;
mod lazy_column;
mod overlay;
mod paint;
mod picker;
mod props;
mod radio;
mod realize;
mod resource;
mod routing;
mod selection;
mod slider;
mod state;
mod style;
mod task;
mod terminal;
mod text;
mod text_field;
mod theme;

use std::cell::RefCell;

#[cfg(all(feature = "snapshot", feature = "test"))]
pub use dev::{adopt_scroll, notice_view as dev_notice_view};
pub use skene_core::{
  self as core,
  Errors,
  HasChildren,
  IntoView,
  KeyEvent,
  Layout,
  Modifiers,
  NodeId,
  PointerButton,
  View,
};
pub use skene_reactive::{self as reactive};
pub use skene_router as router;
use state::{NodeKind, State};
pub use terminal::launch;
#[cfg(feature = "test")]
pub use terminal::{
  current_cursor,
  current_title,
  focused_node,
  take_copied,
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
};

pub use crate::{
  button::Button,
  checkbox::Checkbox,
  container::Container,
  containers::{Column, Flexible, Grid, GridCell, Pinned, Row, Stack},
  events::{PointerEvent, ScrollEvent},
  focus::FocusHandle,
  lazy_column::LazyColumn,
  overlay::{Anchor, Overlay},
  picker::Picker,
  props::{BorderGlyphs, BoxStyle, Condition, ControlStyle, RunStyle},
  radio::Radio,
  resource::Resource,
  routing::install_router,
  slider::Slider,
  task::spawn,
  text::{RichText, Span, Text},
  text_field::TextField,
  theme::{ControlTheme, TextTheme},
};

/// A suspense view over the terminal tree; see [`skene_core::Suspense`].
pub type Suspense = skene_core::Suspense<TuiOps>;
/// An error boundary over the terminal tree; see [`skene_core::ErrorBoundary`].
pub type ErrorBoundary = skene_core::ErrorBoundary<TuiOps>;

/// Spacing on the four edges of a box, in cells.
pub type Inset = skene_core::Inset<u32>;

/// The terminal driver's tree mutations, instantiating the spine's shared structural
/// machinery. App code sees it only through the [`Children`] and [`DerivedView`]
/// aliases.
#[derive(Clone, Copy, Default)]
pub struct TuiOps;

impl skene_core::TreeOps for TuiOps {
  #[inline(always)]
  fn create_group(self) -> NodeId {
    create_group()
  }

  #[inline(always)]
  fn attach(self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    with_state(|state| state.attach_child(parent, child, before));
  }

  #[inline(always)]
  fn detach(self, parent: NodeId, child: NodeId) {
    with_state(|state| state.detach_child(parent, child));
  }

  #[inline(always)]
  fn destroy(self, node: NodeId) {
    with_state(|state| state.destroy(node));
  }
}

/// A container's children.
pub type Children = skene_core::Children<TuiOps>;
/// A reactive subtree.
pub type DerivedView = skene_core::DerivedView<TuiOps>;

/// Reactively reconciles a parent's children from a keyed collection; see
/// [`skene_core::reconcile`].
#[inline(always)]
pub fn reconcile<T, I, K>(
  parent: NodeId,
  items: impl Fn() -> I + 'static,
  key: impl Fn(&T) -> K + 'static,
  build: impl Fn(T) -> NodeId + 'static,
) where
  I: IntoIterator<Item = T>,
  K: Eq + std::hash::Hash + 'static,
{
  skene_core::reconcile::<TuiOps, T, I, K>(parent, items, key, build);
}

/// Anchors a reactive subtree under a fresh group node, returning that node; see
/// [`skene_core::derived`].
#[inline(always)]
pub fn derived(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::derived::<TuiOps>(build)
}

/// Anchors a transitional reactive subtree, each rebuild swapping in once it settles;
/// see [`skene_core::transition`].
#[inline(always)]
pub fn transition(build: impl Fn() -> NodeId + 'static) -> NodeId {
  skene_core::transition::<TuiOps>(build)
}

/// A view that renders nothing.
#[inline(always)]
#[must_use]
pub fn empty() -> impl View {
  skene_core::empty::<TuiOps>()
}

thread_local! {
  static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs `apply` against the installed driver state.
///
/// # Panics
///
/// Panics if the terminal driver is not installed for this thread.
pub(crate) fn with_state<R>(apply: impl FnOnce(&State) -> R) -> R {
  STATE.with(|state| {
    apply(
      state
        .borrow()
        .as_ref()
        .expect("the terminal driver to be installed for this thread"),
    )
  })
}

/// Runs `apply` against the driver state, or `None` when none is installed.
#[cfg(feature = "snapshot")]
pub(crate) fn try_with_state<R>(apply: impl FnOnce(&State) -> R) -> Option<R> {
  STATE.with(|state| state.borrow().as_ref().map(apply))
}

/// Installs the terminal driver for this thread.
///
/// # Panics
///
/// Panics if it is already installed.
pub fn install() {
  skene_reactive::Runtime::init_on_ui_thread();

  STATE.with(|state| {
    let mut state = state.borrow_mut();

    assert!(
      state.is_none(),
      "the terminal driver is already installed for this thread",
    );

    *state = Some(State::new());
  });

  #[cfg(feature = "snapshot")]
  dev::provide_scroll();
}

/// Uninstalls the terminal driver and resets the reactive runtime and root scope.
///
/// Lets one thread run many independent mounts, as a test suite does.
#[cfg(feature = "test")]
pub fn uninstall() {
  skene_reactive::Runtime::reset();
  skene_core::reset_root_owner();
  task::clear();

  STATE.with(|state| *state.borrow_mut() = None);
}

/// Polls every ready task, as the interactive loop does between events; `true` when any
/// ran. For a headless test driving async work.
#[cfg(feature = "test")]
#[must_use]
pub fn pump_tasks() -> bool {
  task::pump()
}

/// Builds `view` under the installed root, returning its node.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_owner().enter(|| {
    let node = view();

    with_state(|state| state.attach_child(state::ROOT_ID, node, None));

    node
  })
}

/// Sets the terminal title, emitted via OSC 2; terminals without support ignore it.
#[inline(always)]
pub fn set_title(title: impl Into<String>) {
  let title = title.into();

  with_state(move |state| {
    *state.title.borrow_mut() = Some(title);

    state.title_dirty.set(true);
  });
}

/// Derives the terminal title, re-emitted whenever it changes.
#[inline(always)]
pub fn derive_title(title: impl Fn() -> String + 'static) {
  skene_reactive::Effect::new(move |_| set_title(title()));
}

/// Lays the mounted tree out at `width` x `height` cells and paints it to plain text—the
/// headless render a test asserts against.
#[must_use]
pub fn render_to_text(width: u16, height: u16) -> String {
  with_state(|state| paint::render(state, width, height).to_text())
}

/// The painted frame's caret cell at `width` x `height`—where the terminal's bar
/// cursor shows—for a headless test asserting caret placement.
#[cfg(feature = "test")]
#[must_use]
pub fn render_to_caret(width: u16, height: u16) -> Option<(u16, u16)> {
  with_state(|state| paint::render(state, width, height).caret)
}

/// The painted frame's colors at `width` x `height`: each cell's foreground and
/// background, row-major—for a headless test asserting paint, such as a shadow's mix.
#[cfg(feature = "test")]
#[must_use]
#[expect(clippy::type_complexity)]
pub fn render_to_colors(
  width: u16,
  height: u16,
) -> Vec<(Option<(u8, u8, u8)>, Option<(u8, u8, u8)>)> {
  with_state(|state| {
    paint::render(state, width, height)
      .cells
      .iter()
      .map(|cell| (cell.foreground, cell.background))
      .collect()
  })
}

/// Allocates a detached grouping node—a block box, as every facade's group is.
pub(crate) fn create_group() -> NodeId {
  with_state(|state| {
    state.insert_node(
      NodeKind::Group,
      taffy::style::Style {
        display: taffy::style::Display::Block,
        ..taffy::style::Style::default()
      },
    )
  })
}
