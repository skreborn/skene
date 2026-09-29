//! What a platform receives to realize a node, and what it reports of the environment.

use rustc_hash::FxHashMap;
use skene_reactive::{Get, Memo, ReadSignal, Signal, Update, With};

use super::{
  BoxDynamics,
  BoxStyle,
  Condition,
  ContainerName,
  Declared,
  Environment,
  Inset,
  Keyframe,
  Media,
  MediaEnv,
  Platform,
  PointerPrecision,
  RunDynamics,
  RunStyle,
  Scalar,
  Static,
};
use crate::{
  AccessProp,
  Color,
  ColorScheme,
  FocusOrder,
  Layout,
  LinkPolicy,
  Placement,
  Role,
  RunDisplay,
  Size,
};

/// A box's style as a builder declares it: values under the platform's conditions,
/// possibly derived.
pub type DeclaredBoxStyle<P> = BoxStyle<
  <P as Platform>::Scalar,
  Declared<Condition<<P as Platform>::Scalar>>,
  <P as Platform>::StyleExt,
>;

/// A box's style as a platform receives it: every derived leaf replaced by its slot.
pub type StaticBoxStyle<P> = BoxStyle<
  <P as Platform>::Scalar,
  Static<Condition<<P as Platform>::Scalar>>,
  <P as Platform>::StyleExt,
>;

/// A run's style as a builder declares it.
pub type DeclaredRunStyle<P> =
  RunStyle<Declared<Condition<<P as Platform>::Scalar>>, <P as Platform>::StyleExt>;

/// A run's style as a platform receives it: every derived leaf replaced by its slot.
pub type StaticRunStyle<P> =
  RunStyle<Static<Condition<<P as Platform>::Scalar>>, <P as Platform>::StyleExt>;

/// A box's style as one keyframe of an animation: a static leaf per set property.
pub type BoxKeyframe<P> = BoxStyle<<P as Platform>::Scalar, Keyframe, <P as Platform>::StyleExt>;

/// A run's style as one keyframe of an animation: a static leaf per set property.
pub type RunKeyframe<P> = RunStyle<Keyframe, <P as Platform>::StyleExt>;

/// Everything a platform needs to realize a box.
#[derive(Clone, Debug)]
#[must_use]
pub struct BoxProps<P: Platform> {
  /// The layout mode.
  pub layout: Layout,
  /// The semantic role, guiding the platform's concrete choice of element.
  pub role: Option<Role>,
  /// The static style: every derived leaf already replaced by its slot.
  pub style: StaticBoxStyle<P>,
  /// The transitions and animations.
  pub dynamics: BoxDynamics<P::Scalar, P::StyleExt>,
  /// The name pairing the box with its counterpart across a view transition.
  pub transition_name: Option<String>,
  /// The name the box publishes for container queries against its size.
  pub container: Option<ContainerName>,
  /// The box's place in focus traversal; `None` for a box that takes no focus.
  pub focus_order: Option<FocusOrder>,
  /// The accessibility properties the box reports.
  pub access: Vec<AccessProp>,
  /// The path the box navigates to when activated; a real link where the medium has
  /// one.
  pub destination: Option<String>,
}

/// Everything a platform needs to realize a text run.
#[derive(Clone, Debug)]
#[must_use]
pub struct RunProps<P: Platform> {
  /// How the run participates in the surrounding layout.
  pub display: RunDisplay,
  /// Whether the user can select and copy the run's text.
  pub selectable: bool,
  /// The link destination, when the run is a link.
  pub link: Option<String>,
  /// How the link opens and what it discloses; meaningful only when `link` is set.
  pub link_policy: LinkPolicy,
  /// The static style: every derived leaf already replaced by its slot.
  pub style: StaticRunStyle<P>,
  /// The static per-child layout a wrapper gave the run (its flexibility, alignment,
  /// pinning and grid placement), as a box style holding nothing else.
  pub layout: Option<Box<StaticBoxStyle<P>>>,
  /// The transitions and animations.
  pub dynamics: RunDynamics<P::StyleExt>,
}

/// Everything a platform needs to realize an overlay: a box shown above the app while
/// open, opened and closed through [`Platform::set_overlay_open`].
#[derive(Clone, Debug)]
#[must_use]
pub struct OverlayProps<P: Platform> {
  /// Whether the overlay is modal: it traps focus and blocks interaction beneath it.
  pub modal: bool,
  /// Whether the overlay is a hint: a popup describing its anchor, which never takes
  /// focus and sits beneath every other popup.
  pub hint: bool,
  /// Where the overlay sits.
  pub placement: Placement,
  /// The backdrop color painted over the app while open, if any.
  pub backdrop: Option<Color>,
  /// The box the overlay's content is realized as.
  pub box_props: BoxProps<P>,
}

/// The measured sizes of the named query containers, filled by the platform.
///
/// A read tracks, so a condition against a container re-evaluates when the platform
/// reports a new size or names a container it had not measured before.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct ContainerSizes<N: 'static>(Signal<FxHashMap<ContainerName, Size<N>>>);

impl<N: Scalar> ContainerSizes<N> {
  /// An empty lookup, owned by the current owner.
  #[inline(always)]
  pub fn new() -> Self {
    Self(Signal::opaque(FxHashMap::default()))
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
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct MediaSignals<N: 'static> {
  /// The viewport's size.
  pub viewport: ReadSignal<Size<N>>,
  /// The color scheme in force.
  pub scheme: ReadSignal<ColorScheme>,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: ReadSignal<bool>,
  /// Whether the reader prefers reduced transparency.
  pub reduced_transparency: ReadSignal<bool>,
  /// Whether the reader prefers more contrast.
  pub high_contrast: ReadSignal<bool>,
  /// How precisely the reader's primary pointer points.
  pub pointer: ReadSignal<PointerPrecision>,
  /// Whether the reader's primary pointer hovers.
  pub hover: ReadSignal<bool>,
  /// The insets the display keeps clear of the viewport's edges.
  pub safe_area: ReadSignal<Inset<N>>,
  /// The measured sizes of the named query containers.
  pub container_sizes: ContainerSizes<N>,
}

impl<N: Clone + 'static> MediaSignals<N> {
  /// The environment the signals hold, each read tracked.
  pub fn environment(&self) -> Environment<N> {
    Environment {
      viewport: self.viewport.get(),
      scheme: self.scheme.get(),
      reduced_motion: self.reduced_motion.get(),
      reduced_transparency: self.reduced_transparency.get(),
      high_contrast: self.high_contrast.get(),
      pointer: self.pointer.get(),
      hover: self.hover.get(),
      safe_area: self.safe_area.get(),
    }
  }
}

impl<N: Scalar> MediaSignals<N> {
  /// Whether `media` holds, as a memo following these signals; a container condition
  /// never holds here, since it is answered per node by the box it names.
  pub fn watch(&self, media: &Media<N>) -> Memo<bool> {
    let media = media.clone();
    let signals = *self;

    Memo::new(move |_| {
      let container = |_: Option<ContainerName>| None;

      media.holds_in(&MediaEnv {
        environment: signals.environment(),
        container: &container,
      })
    })
  }
}

/// An event a node can carry handlers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum EventKind {
  /// A press and release on one target.
  Click,
  /// A key pressed while the node or a descendant holds focus.
  KeyDown,
  /// A key released while the node or a descendant holds focus.
  KeyUp,
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
  /// A text entry's selection changed.
  SelectionChange,
  /// A control's checked state changed.
  Toggle,
  /// An overlay was dismissed by the platform: a light dismiss, or `Escape`.
  Dismiss,
  /// A context menu was asked for at the node: a secondary press, or the menu key
  /// while it holds focus; the platform's own menu opens unless prevented.
  ContextMenu,
}
