//! What a driver receives to realize a node, and what it reports of the environment.

use rustc_hash::FxHashMap;
use skene_reactive::{ReadSignal, Signal, Update, With};

use super::{
  BoxDynamics,
  BoxStyle,
  Condition,
  ContainerName,
  Declared,
  Driver,
  RunDynamics,
  RunStyle,
  Scalar,
  Static,
};
use crate::{AccessProp, ColorScheme, FocusOrder, Layout, LinkPolicy, Role, RunDisplay, Size};

/// A box's style as a builder declares it: values under the driver's conditions,
/// possibly derived.
pub type DeclaredBoxStyle<D> = BoxStyle<
  <D as Driver>::Scalar,
  Declared<Condition<<D as Driver>::Scalar>>,
  <D as Driver>::StyleExt,
>;

/// A box's style as a driver receives it: every derived leaf replaced by its slot.
pub type StaticBoxStyle<D> = BoxStyle<
  <D as Driver>::Scalar,
  Static<Condition<<D as Driver>::Scalar>>,
  <D as Driver>::StyleExt,
>;

/// A run's style as a builder declares it.
pub type DeclaredRunStyle<D> =
  RunStyle<Declared<Condition<<D as Driver>::Scalar>>, <D as Driver>::StyleExt>;

/// A run's style as a driver receives it: every derived leaf replaced by its slot.
pub type StaticRunStyle<D> =
  RunStyle<Static<Condition<<D as Driver>::Scalar>>, <D as Driver>::StyleExt>;

/// Everything a driver needs to realize a box.
#[derive(Clone)]
#[must_use]
pub struct BoxProps<D: Driver> {
  /// The layout mode.
  pub layout: Layout,
  /// The semantic role, guiding the driver's concrete choice of element.
  pub role: Option<Role>,
  /// The static style: every derived leaf already replaced by its slot.
  pub style: StaticBoxStyle<D>,
  /// The transitions and animations.
  pub dynamics: BoxDynamics<D::Scalar, D::StyleExt>,
  /// The name the box publishes for container queries against its size.
  pub container: Option<ContainerName>,
  /// The box's place in focus traversal; `None` for a box that takes no focus.
  pub focus_order: Option<FocusOrder>,
  /// The accessibility properties the box reports.
  pub access: Vec<AccessProp>,
}

/// Everything a driver needs to realize a text run.
#[derive(Clone)]
#[must_use]
pub struct RunProps<D: Driver> {
  /// How the run participates in the surrounding layout.
  pub display: RunDisplay,
  /// Whether the user can select and copy the run's text.
  pub selectable: bool,
  /// The link destination, when the run is a link.
  pub link: Option<String>,
  /// How the link opens and what it discloses; meaningful only when `link` is set.
  pub link_policy: LinkPolicy,
  /// The static style: every derived leaf already replaced by its slot.
  pub style: StaticRunStyle<D>,
  /// The transitions and animations.
  pub dynamics: RunDynamics<D::StyleExt>,
}

/// The measured sizes of the named query containers, filled by the driver.
///
/// A read tracks, so a condition against a container re-evaluates when the driver
/// reports a new size or names a container it had not measured before.
#[derive(Clone, Copy)]
#[must_use]
pub struct ContainerSizes<N: 'static>(Signal<FxHashMap<ContainerName, Size<N>>>);

impl<N: Scalar> ContainerSizes<N> {
  /// An empty lookup, owned by the current owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self(Signal::new(FxHashMap::default()))
  }

  /// The last size reported for `name`, tracked.
  #[inline(always)]
  #[must_use]
  pub fn get(&self, name: ContainerName) -> Option<Size<N>> {
    self.0.with(|sizes| sizes.get(&name).copied())
  }

  /// Records the size measured for `name`.
  #[inline(always)]
  pub fn set(&self, name: ContainerName, size: Size<N>) {
    self.0.update(|sizes| {
      sizes.insert(name, size);
    });
  }

  /// Forgets `name`, once no box publishes it.
  #[inline(always)]
  pub fn remove(&self, name: ContainerName) {
    self.0.update(|sizes| {
      sizes.remove(&name);
    });
  }
}

impl<N: Scalar> Default for ContainerSizes<N> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// The environment as signals: what every [`Media`](super::Media) condition answers to.
#[derive(Clone, Copy)]
#[must_use]
pub struct MediaSignals<N: 'static> {
  /// The viewport's size.
  pub viewport: ReadSignal<Size<N>>,
  /// The color scheme in force.
  pub scheme: ReadSignal<ColorScheme>,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: ReadSignal<bool>,
  /// The measured sizes of the named query containers.
  pub container_sizes: ContainerSizes<N>,
}

/// An event a node can carry handlers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum EventKind {
  /// A press and release on one target.
  Click,
  /// A key pressed while the node or a descendant holds focus.
  KeyDown,
  /// A pointer button pressed over the node.
  PointerDown,
  /// A pointer button released over the node.
  PointerUp,
  /// The pointer moved over the node.
  PointerMove,
  /// The pointer entered the node.
  PointerEnter,
  /// The pointer left the node.
  PointerLeave,
  /// The node gained focus.
  Focus,
  /// The node lost focus.
  Blur,
  /// The node's scroll offsets changed.
  Scroll,
  /// A control's textual value changed.
  Change,
  /// A control's checked state changed.
  Toggle,
}
