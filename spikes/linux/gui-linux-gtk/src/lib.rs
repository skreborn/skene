//! # skene-gui-linux
//!
//! skene-gui's Linux realization, over GTK 4.14: the seam's
//! [`Realization`](skene_foundation::seam::Realization) implemented once by [`Linux`].
//!
//! - **Layout** ([`host`]): every Skene container is a widget whose GTK layout manager
//!   ([`SkeneLayout`]) runs the foundation's engine over its children, each toolkit widget
//!   measured through GTK's own `measure` with its minimum as a floor, its own alignment and
//!   margins neutral, and fractional rectangles handed from one Skene container to the next.
//! - **Style** ([`sheet`]): content-addressed classes in one display-wide provider at priority
//!   599, reference-counted and collected; color tokens as named colors in a provider at 598,
//!   rewritten on a palette or scheme change; states as pseudo-classes; derived values in a
//!   per-widget provider.
//! - **Controls** (`controls`): the toolkit's own, reporting through the seam and brought
//!   back to the truth with each control's own timing.
//! - **Environment** ([`environment`]): the reader's scheme and contrast from the XDG settings
//!   portal (GtkSettings where no portal answers), motion from `gtk-enable-animations`, the
//!   window's size and scale observed live.
//! - **Start** ([`app`]): a `GtkApplication` whose `activate` builds the root into a window.

#[cfg(not(skene_platform = "linux"))]
compile_error!("skene-gui-linux builds for `skene_platform = \"linux\"` alone");

mod access;
pub mod app;
mod controls;
pub mod environment;
mod events;
pub mod host;
mod realization;
pub mod sheet;
pub mod testing;

use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  rc::Rc,
};

pub use gtk;
use gtk::prelude::*;
use skene_foundation::{
  NodeId,
  layout::{Rect, ResolvedLayout},
  seam::{
    content::{Content, PropValue},
    node::LayoutKind,
    report::{Answer, Report},
    style::CompiledStyle,
    tree,
  },
};

pub use crate::host::{SkeneBox, SkeneButton, SkeneLayout, SkenePainted};

/// GTK 4: the realization a Linux build is for.
#[derive(Clone, Copy, Debug)]
pub struct Linux;

/// A node's toolkit objects.
#[derive(Clone, Debug)]
pub struct Node {
  /// The widget placed in the parent.
  pub widget: gtk::Widget,
}

/// What each realized widget carries for the engine, the sheet and the controls.
#[derive(Default)]
pub struct Slot {
  /// The node it realizes.
  pub id: Cell<Option<NodeId>>,
  /// The container kind, where it is a Skene container.
  pub container: Cell<Option<LayoutKind>>,
  /// Its resolved layout: its own where a container, and its placement.
  pub layout: RefCell<ResolvedLayout>,
  /// The fractional rectangle its Skene parent laid it out at.
  pub hint: Cell<Option<Rect>>,
  /// Its style class on the widget now.
  pub class: RefCell<Option<String>>,
  /// The style class it holds a reference to: the latest applied, swapped in at the next
  /// flush.
  pub target: RefCell<Option<String>>,
  /// Its compiled style, for its derived properties' per-widget rules.
  pub style: RefCell<Option<CompiledStyle>>,
  /// The per-widget provider of its derived properties.
  pub provider: RefCell<Option<gtk::CssProvider>>,
  /// The interests installed ([`skene_foundation::seam::report::Interest`]): one controller
  /// per kind.
  pub watched: Cell<u8>,
  /// Whether an input method composes in it.
  pub composing: Cell<bool>,
  /// Values GTK has no property for (a link's destination, a progress's bounds).
  pub extra: RefCell<HashMap<Content, PropValue>>,
  /// The size last reported ([`skene_foundation::seam::report::Interest::size`]).
  pub size: Cell<Option<(i32, i32, f64)>>,
}

const SLOT: &str = "skene-slot";

/// The slot of `widget`, created on first use.
pub fn slot(widget: &gtk::Widget) -> Rc<Slot> {
  // SAFETY: the key only ever holds an `Rc<Slot>`, written here.
  unsafe {
    if let Some(slot) = widget.data::<Rc<Slot>>(SLOT) {
      return Rc::clone(slot.as_ref());
    }

    let slot = Rc::new(Slot::default());

    widget.set_data(SLOT, Rc::clone(&slot));

    slot
  }
}

/// The node `widget` realizes.
pub fn node_of(widget: &gtk::Widget) -> Option<NodeId> {
  slot(widget).id.get()
}

/// The node `widget` realizes, or the nearest ancestor's.
pub fn nearest_node(widget: &gtk::Widget) -> Option<NodeId> {
  let mut next = Some(widget.clone());

  while let Some(widget) = next {
    if let Some(id) = node_of(&widget) {
      return Some(id);
    }

    next = widget.parent();
  }

  None
}

/// Skene's role for `widget`'s GTK accessible role: a role GTK has and Skene does not is
/// `Role::Generic`.
#[must_use]
pub fn role_of(widget: &gtk::Widget) -> skene_foundation::seam::access::Role {
  access::role_of(widget)
}

/// The widget realizing `id`.
#[must_use]
pub fn widget(id: NodeId) -> Option<gtk::Widget> {
  tree::native::<Linux>(id).map(|node| node.widget)
}

/// Reports at `id` through the shared half.
fn report(id: NodeId, report: Report) -> Answer {
  tree::report::<Linux>(id, report)
}

/// The device pixels per pixel `widget` is drawn at: its surface's, fractional where the
/// display is, or its integer scale factor before it has one.
pub fn scale(widget: &gtk::Widget) -> f64 {
  widget
    .native()
    .and_then(|native| native.surface())
    .map_or_else(
      || f64::from(widget.scale_factor()),
      |surface| surface.scale(),
    )
}

/// Reports `widget`'s size where its node watches it and it changed.
pub(crate) fn report_size(widget: &gtk::Widget, width: i32, height: i32) {
  let slot = slot(widget);

  if slot.watched.get() & realization::WATCH_SIZE == 0 {
    return;
  }

  let now = (width, height, scale(widget));

  if slot.size.replace(Some(now)) == Some(now) {
    return;
  }

  if let Some(id) = slot.id.get() {
    report(
      id,
      Report::Size(skene_foundation::seam::report::Size {
        width: width as f32,
        height: height as f32,
        scale: now.2,
      }),
    );
  }
}

/// Whether an event's answer says a handler consumed it.
const fn handled(answer: &Answer) -> bool {
  matches!(answer, Answer::Event { handled: true })
}

/// A widget's children, in order.
fn children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
  let mut children = Vec::new();
  let mut next = widget.first_child();

  while let Some(child) = next {
    next = child.next_sibling();
    children.push(child);
  }

  children
}
