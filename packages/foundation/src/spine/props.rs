//! What a platform receives to realize a node, and what it reports of the environment.

use rustc_hash::FxHashMap;
use skene_reactive::{Get, Memo, ReadSignal, Signal, Update, With, WriteSignal};

use super::{
  BoxStyle,
  Condition,
  ContainerName,
  ControlFacts,
  Declared,
  DragAxes,
  Environment,
  Inset,
  Keyframe,
  Media,
  MediaEnv,
  NodeKind,
  PointerPrecision,
  Realization,
  RunStyle,
  Scalar,
  Static,
};
use crate::spine::{Color, ColorScheme, Content, Layout, Role, RunPlacement, Size};

/// A box's style as a builder declares it: values under the platform's conditions,
/// possibly derived.
pub type DeclaredBoxStyle<P> = BoxStyle<
  <P as Realization>::Scalar,
  Declared<Condition<<P as Realization>::Scalar>>,
  <P as Realization>::StyleExt,
>;

/// A box's style split from its derived leaves, each replaced by its slot: what its
/// record is built from.
pub type StaticBoxStyle<P> = BoxStyle<
  <P as Realization>::Scalar,
  Static<Condition<<P as Realization>::Scalar>>,
  <P as Realization>::StyleExt,
>;

/// A run's style as a builder declares it.
pub type DeclaredRunStyle<P> =
  RunStyle<Declared<Condition<<P as Realization>::Scalar>>, <P as Realization>::StyleExt>;

/// A run's style split from its derived leaves, each replaced by its slot: what its
/// record is built from.
pub type StaticRunStyle<P> =
  RunStyle<Static<Condition<<P as Realization>::Scalar>>, <P as Realization>::StyleExt>;

/// A box's style as one keyframe of an animation: a static leaf per set property.
pub type BoxKeyframe<P> =
  BoxStyle<<P as Realization>::Scalar, Keyframe, <P as Realization>::StyleExt>;

/// A run's style as one keyframe of an animation: a static leaf per set property.
pub type RunKeyframe<P> = RunStyle<Keyframe, <P as Realization>::StyleExt>;

/// What a platform creates a node from beside its style.
///
/// It holds what fixes the node's element: what a reader takes the node for, the heading
/// level its element is fixed at, whether a press drags it, and the facts of its kind.
/// The style arrives through [`Realization::apply_style`], and what can change once the
/// node exists, a link's destination, an entry's placeholder, an overlay's placement
/// among it, through [`Realization::write`].
#[derive(Clone, Debug)]
#[must_use]
pub struct NodeSpec {
  /// The semantic role, guiding the platform's concrete choice of element; an
  /// overlay's is the one [`new`](Self::new) computes from its facts.
  pub role: Option<Role>,
  /// The heading level the node reports as it is created, which a medium with an
  /// element per level fixes the element by.
  pub level: Option<usize>,
  /// The axes a press's pointer drags the node along; `None` for a node no press drags.
  pub drag: Option<DragAxes>,
  /// The facts of the node's kind.
  pub facts: NodeFacts,
}

impl NodeSpec {
  /// The spec of a node of `facts` stating `role`, reporting the heading `level` and
  /// dragged along `drag`: an overlay carries a hint's tooltip role, else the role it
  /// states, else a modal's dialog role.
  pub const fn new(
    role: Option<Role>,
    level: Option<usize>,
    drag: Option<DragAxes>,
    facts: NodeFacts,
  ) -> Self {
    let role = match &facts {
      NodeFacts::Overlay(overlay) => overlay.role(role),
      _ => role,
    };

    Self {
      role,
      level,
      drag,
      facts,
    }
  }

  /// The spec of an anchor: a layout-invisible marker a reactive subtree mounts
  /// between.
  #[inline(always)]
  pub const fn anchor() -> Self {
    Self::bare(NodeFacts::Anchor)
  }

  /// The spec of a scope: a layout-invisible node token definitions hang off.
  #[inline(always)]
  pub const fn scope() -> Self {
    Self::bare(NodeFacts::Scope)
  }

  /// The spec of a node of `facts` stating nothing else.
  #[inline(always)]
  const fn bare(facts: NodeFacts) -> Self {
    Self {
      role: None,
      level: None,
      drag: None,
      facts,
    }
  }

  /// Whether the spec creates a box that acts on a click of its own; see
  /// [`BoxFacts::acting`].
  #[inline(always)]
  #[must_use]
  pub const fn acting(&self) -> bool {
    matches!(&self.facts, NodeFacts::Box(facts) if facts.acting)
  }

  /// The kind of node the spec creates, as the scene records it.
  pub const fn kind(&self) -> NodeKind {
    match self.facts {
      NodeFacts::Box(_) => NodeKind::Box,
      NodeFacts::Control(_) => NodeKind::Control,
      NodeFacts::Overlay(_) => NodeKind::Overlay,
      NodeFacts::Run(_) => NodeKind::Run,
      NodeFacts::Anchor => NodeKind::Anchor,
      NodeFacts::Scope => NodeKind::Scope,
    }
  }
}

/// The facts of a node's kind: what a platform creates it from that no other kind
/// has.
#[derive(Clone, Debug)]
#[must_use]
pub enum NodeFacts {
  /// A box.
  Box(BoxFacts),
  /// A control: a box a platform may stand a native element in for.
  Control(ControlFacts),
  /// An overlay: a box shown above the app while open, opened and closed through
  /// [`Realization::set_overlay_open`].
  Overlay(OverlayFacts),
  /// A text run.
  Run(RunFacts),
  /// An anchor: a layout-invisible marker a reactive subtree mounts between.
  Anchor,
  /// A scope: a layout-invisible node token definitions hang off.
  Scope,
}

/// What a platform creates a box from beside its style: the box of a box, a control
/// or an overlay.
#[derive(Clone, Debug)]
#[must_use]
pub struct BoxFacts {
  /// The layout mode.
  pub layout: Layout,
  /// The name the box publishes for container queries against its size.
  pub container: Option<ContainerName>,
  /// The name pairing the box with its counterpart across a view transition.
  pub transition_name: Option<String>,
  /// Whether the box is a link, navigating to the path its
  /// [`Destination`](super::Content::Destination) content holds when activated; a real
  /// link where the medium has one.
  pub link: bool,
  /// Whether the box acts on a click of its own: it handles one and is no link, so it
  /// takes focus and `Enter` and `Space` activate it, an activatable row.
  pub acting: bool,
  /// Whether a view taking decoration created the box (a `Container`, an overlay's box,
  /// a control's), so a platform that draws lays it out with an engine of its own,
  /// where a layout-only box joins its nearest decorated ancestor's.
  pub decorated: bool,
}

/// What a platform creates an overlay from: its box, whether it is modal or a hint, and
/// its backdrop; where it sits is its [`Placement`](super::Content::Placement) content.
#[derive(Clone, Debug)]
#[must_use]
pub struct OverlayFacts {
  /// The box the overlay's content is realized as.
  pub boxed: BoxFacts,
  /// Whether the overlay is modal: it traps focus and blocks interaction beneath it.
  pub modal: bool,
  /// Whether the overlay is a hint: a popup describing its anchor, which never takes
  /// focus and sits beneath every other popup.
  pub hint: bool,
  /// The backdrop color painted over the app while open, if any.
  pub backdrop: Option<Color>,
}

impl OverlayFacts {
  /// The role the overlay carries where its box states `stated`: a hint's tooltip, a
  /// modal's dialog unless its box states another, else the one its box states.
  #[must_use]
  pub const fn role(&self, stated: Option<Role>) -> Option<Role> {
    if self.hint {
      return Some(Role::Tooltip);
    }

    match stated {
      Some(role) => Some(role),
      None if self.modal => Some(Role::Dialog),
      None => None,
    }
  }
}

/// What a platform creates a text run from beside its style.
#[derive(Clone, Debug)]
#[must_use]
pub struct RunFacts {
  /// Where the run sits in the surrounding layout.
  pub placement: RunPlacement,
  /// Whether the user can select and copy the run's text.
  pub selectable: bool,
  /// Whether the run is a link, navigating to the path its
  /// [`Destination`](super::Content::Destination) content holds when followed.
  pub link: bool,
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
    Self(Signal::new(FxHashMap::default()))
  }

  /// The last size reported for `name`, tracked.
  #[inline(always)]
  #[must_use]
  pub fn get(&self, name: ContainerName) -> Option<Size<N>> {
    self.0.with(|sizes| sizes.get(&name).copied())
  }

  /// The last size reported for `name`, read without tracking.
  #[inline(always)]
  #[must_use]
  pub fn get_untracked(&self, name: ContainerName) -> Option<Size<N>> {
    skene_reactive::untrack(|| self.get(name))
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

/// The write halves of [`MediaSignals`], one per signal of the environment.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct MediaWriters<N: 'static> {
  /// Writes the viewport's size.
  pub viewport: WriteSignal<Size<N>>,
  /// Writes the color scheme in force.
  pub scheme: WriteSignal<ColorScheme>,
  /// Writes whether the reader prefers reduced motion.
  pub reduced_motion: WriteSignal<bool>,
  /// Writes whether the reader prefers reduced transparency.
  pub reduced_transparency: WriteSignal<bool>,
  /// Writes whether the reader prefers more contrast.
  pub high_contrast: WriteSignal<bool>,
  /// Writes how precisely the reader's primary pointer points.
  pub pointer: WriteSignal<PointerPrecision>,
  /// Writes whether the reader's primary pointer hovers.
  pub hover: WriteSignal<bool>,
  /// Writes the insets the display keeps clear of the viewport's edges.
  pub safe_area: WriteSignal<Inset<N>>,
}

impl<N: Scalar> MediaSignals<N> {
  /// Signals holding `environment`, owned by the current owner, with no container
  /// measured yet, and their write halves.
  pub fn new(environment: Environment<N>) -> (Self, MediaWriters<N>) {
    let (viewport, set_viewport) = Signal::new(environment.viewport).split();
    let (scheme, set_scheme) = Signal::new(environment.scheme).split();
    let (reduced_motion, set_reduced_motion) = Signal::new(environment.reduced_motion).split();
    let (reduced_transparency, set_reduced_transparency) =
      Signal::new(environment.reduced_transparency).split();
    let (high_contrast, set_high_contrast) = Signal::new(environment.high_contrast).split();
    let (pointer, set_pointer) = Signal::new(environment.pointer).split();
    let (hover, set_hover) = Signal::new(environment.hover).split();
    let (safe_area, set_safe_area) = Signal::new(environment.safe_area).split();

    (
      Self {
        viewport,
        scheme,
        reduced_motion,
        reduced_transparency,
        high_contrast,
        pointer,
        hover,
        safe_area,
        container_sizes: ContainerSizes::new(),
      },
      MediaWriters {
        viewport: set_viewport,
        scheme: set_scheme,
        reduced_motion: set_reduced_motion,
        reduced_transparency: set_reduced_transparency,
        high_contrast: set_high_contrast,
        pointer: set_pointer,
        hover: set_hover,
        safe_area: set_safe_area,
      },
    )
  }
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
  /// never holds here; the box it names answers it per node.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub enum EventKind {
  /// A press and release on one target.
  Click,
  /// The second click of a double click, after the click itself.
  DoubleClick,
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
  /// The platform took a pointer pressed over the node for itself, ending its press
  /// without a release.
  PointerCancel,
  /// The pointer entered the node.
  PointerEnter,
  /// The pointer left the node.
  PointerLeave,
  /// The node gained focus.
  Focus,
  /// The node lost focus.
  Blur,
  /// A wheel turned over the node, before the platform scrolls by it.
  Wheel,
  /// The node's scroll offsets changed.
  Scroll,
  /// A reader changed a content the node controls, which awaits the verdict on it.
  Report(Content),
  /// An overlay was dismissed by the platform: a light dismiss, or `Escape`.
  Dismiss,
  /// A drag started from the node, which offers what it carries.
  DragStart,
  /// A drag the node takes dropped on it.
  Drop,
  /// A context menu was asked for at the node: a secondary press, or the menu key
  /// while it holds focus; the platform's own menu opens unless prevented.
  ContextMenu,
}

#[cfg(test)]
mod tests {
  use super::{BoxFacts, NodeFacts, NodeSpec, OverlayFacts, RunFacts};
  use crate::spine::{ControlFacts, Layout, NodeKind, Role, RunPlacement};

  /// The facts of a plain box.
  fn boxed() -> BoxFacts {
    BoxFacts {
      layout: Layout::Document,
      container: None,
      transition_name: None,
      link: false,
      acting: false,
      decorated: false,
    }
  }

  /// The facts of an overlay, modal and a hint where `modal` and `hint` say.
  fn overlay(modal: bool, hint: bool) -> NodeFacts {
    NodeFacts::Overlay(OverlayFacts {
      boxed: boxed(),
      modal,
      hint,
      backdrop: None,
    })
  }

  #[test]
  fn an_overlay_takes_a_hints_role_else_its_stated_role_else_a_modals() {
    let role = |stated, modal, hint| NodeSpec::new(stated, None, None, overlay(modal, hint)).role;

    assert_eq!(role(None, false, true), Some(Role::Tooltip));
    assert_eq!(role(Some(Role::Menu), true, true), Some(Role::Tooltip));
    assert_eq!(role(Some(Role::Menu), true, false), Some(Role::Menu));
    assert_eq!(role(None, true, false), Some(Role::Dialog));
    assert_eq!(role(Some(Role::ListBox), false, false), Some(Role::ListBox));
    assert_eq!(role(None, false, false), None);
  }

  #[test]
  fn a_node_of_another_kind_keeps_its_stated_role() {
    assert_eq!(
      NodeSpec::new(Some(Role::Heading), Some(2), None, NodeFacts::Box(boxed())).role,
      Some(Role::Heading),
    );
    assert_eq!(
      NodeSpec::new(None, None, None, NodeFacts::Box(boxed())).role,
      None
    );
  }

  #[test]
  fn a_spec_creates_the_node_of_its_facts_kind() {
    let run = NodeFacts::Run(RunFacts {
      placement: RunPlacement::Line,
      selectable: false,
      link: false,
    });
    let control = NodeFacts::Control(ControlFacts {
      boxed: boxed(),
      native: None,
      label: None,
      lines: 1,
      label_beside: false,
    });

    for (facts, kind) in [
      (NodeFacts::Box(boxed()), NodeKind::Box),
      (control, NodeKind::Control),
      (overlay(false, false), NodeKind::Overlay),
      (run, NodeKind::Run),
    ] {
      assert_eq!(NodeSpec::new(None, None, None, facts).kind(), kind);
    }

    assert_eq!(NodeSpec::anchor().kind(), NodeKind::Anchor);
    assert_eq!(NodeSpec::scope().kind(), NodeKind::Scope);
  }

  /// Only a box acts on a click of its own: a control or an overlay whose box says it
  /// does is the control's or the overlay's to act on.
  #[test]
  fn only_a_box_acts_on_its_click() {
    let acting = BoxFacts {
      acting: true,
      ..boxed()
    };
    let spec = |facts| NodeSpec::new(None, None, None, facts);

    assert!(spec(NodeFacts::Box(acting.clone())).acting());
    assert!(!spec(NodeFacts::Box(boxed())).acting());
    assert!(
      !spec(NodeFacts::Overlay(OverlayFacts {
        boxed: acting,
        modal: false,
        hint: false,
        backdrop: None,
      }))
      .acting()
    );
  }
}
