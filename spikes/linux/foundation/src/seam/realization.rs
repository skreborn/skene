//! The seam itself: what every pixel platform's realization implements.

use std::{process::ExitCode, time::Duration};

use crate::{
  NodeId, Scheme, SystemColors,
  layout::{Rect, ResolvedLayout},
  seam::{
    access::AccessFacts,
    content::{Content, PropValue, Unit},
    control::{Availability, ClipboardOp},
    node::NodeSpec,
    report::Interest,
    runtime::{Cancel, Task},
    style::{CompiledStyle, Keep, KeptDefinition, KeptLeaf, ScopeId},
  },
};

/// Who lays a container's children out.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LayoutOwner {
  /// The foundation's engine (taffy) through the realization's host container: the layout
  /// names are resolved by the shared half and reach [`Realization::apply_layout`], never
  /// the toolkit. Linux (drawn by Skene), UIKit, AppKit, Android Views, WinUI.
  Engine,
  /// The toolkit, whose layout is CSS's flexbox and grid: the layout names reach
  /// [`Realization::apply_style`] with the box and run names. The web.
  Toolkit,
}

/// A pixel platform's realization: a zero-sized type each platform crate implements once,
/// selected per build by `skene_platform`, monomorphized and never `dyn`.
///
/// It is a sink and a source: the shared half creates, places and writes nodes through it,
/// and it reports what the reader did through [`crate::seam::tree::report`]. It never reads a
/// view, and the shared half never names a toolkit.
///
/// Every method is a toolkit call. A realization must not call back into the shared half
/// while the shared half is calling it, except to report (reports made while Skene writes are
/// ignored, [`crate::seam::content::writing`]).
///
/// What the realization owes the testing driver beyond these methods: it calls
/// [`crate::seam::activity::frame`] after each frame it paints, and reports every user change
/// through [`crate::seam::tree::report`], which counts it ([`crate::seam::activity`]).
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a Skene pixel platform's realization",
  note = "a platform realizes skene-gui's views by implementing `skene_foundation::seam::Realization` once, over its toolkit"
)]
pub trait Realization: Sized + 'static {
  /// What the realization keeps per node: the toolkit objects that realize it.
  type Node: Clone + 'static;

  /// A toolkit widget: what the native door takes in, and what a realized Skene subtree is
  /// handed out as ([`Realization::widget`] of its root), so a native container lays it out
  /// as any widget while its host container answers the toolkit's measure and allocate.
  type Widget: Clone + 'static;

  /// Who lays containers out.
  const LAYOUT: LayoutOwner;

  /// What of a style the toolkit resolves itself.
  const KEEP: Keep;

  /// The unit the toolkit counts text offsets in: what [`crate::seam::report::Report::Edit`]
  /// and [`crate::seam::report::Report::Selection`] carry.
  const UNIT: Unit;

  /// Whether a theme scope is a node of its own (the web's custom properties inherit through
  /// elements); otherwise its children join its parent.
  const SCOPE_NODES: bool;

  /// Realizes a detached node of `spec` for `id`, wiring its toolkit signals to reports.
  fn create(id: NodeId, spec: &NodeSpec) -> Self::Node;

  /// Adopts a toolkit widget a library handed in through the native door.
  fn adopt(id: NodeId, widget: Self::Widget, spec: &NodeSpec) -> Self::Node;

  /// The toolkit widget realizing `node`: for the platform's own vocabulary and to embed a
  /// Skene subtree in a native container.
  fn widget(node: &Self::Node) -> Self::Widget;

  /// Places `child` at `index` among the realized children of `parent`.
  fn insert(parent: &Self::Node, child: &Self::Node, index: usize);

  /// Moves `child`, already under `parent`, to `index`, keeping what it holds (focus, scroll
  /// offsets, an input method's composition).
  fn relocate(parent: &Self::Node, child: &Self::Node, index: usize);

  /// Hides `node` to show it again later, or shows it: a hidden node takes no room and keeps
  /// what leaving would lose.
  fn set_hidden(node: &Self::Node, hidden: bool);

  /// Removes `node` and everything beneath it for good.
  fn remove(node: &Self::Node);

  /// Applies a compiled style: called again only when its address changed. A style that
  /// declares nothing is never applied to a node that had none.
  fn apply_style(node: &Self::Node, style: &CompiledStyle);

  /// Writes a style's slots alone, its address unchanged: a toolkit's per-view rules of its
  /// derived properties, the web's per-node custom properties.
  fn apply_slots(node: &Self::Node, slots: &[KeptLeaf]);

  /// Hands a node's resolved layout to its host container ([`LayoutOwner::Engine`]): its own
  /// where it is a container, and its placement, which its parent's container reads.
  fn apply_layout(node: &Self::Node, layout: &ResolvedLayout);

  /// Defines a scope's kept tokens, on its node where scopes are nodes; display-wide where
  /// [`Keep::tags_tokens`] is, each under its name and its tag. An empty `tokens` withdraws
  /// what the scope defined: its theme was disposed.
  fn define_tokens(scope: ScopeId, node: Option<&Self::Node>, tokens: &[KeptDefinition]);

  /// Shows a content value, reporting nothing. A value the shared half bound or controls is
  /// always one the node's kind takes ([`crate::seam::node::NodeKind::contents`]), of its
  /// type.
  fn write(node: &Self::Node, content: Content, value: &PropValue);

  /// The content value the platform shows now, read from the toolkit.
  fn read(node: &Self::Node, content: Content) -> Option<PropValue>;

  /// Whether an input method is composing in `node`.
  fn composing(node: &Self::Node) -> bool;

  /// Maps accessibility facts onto the platform's layer: writes what changed from `previous`
  /// to `next` and resets what `next` no longer states, `related` finding the nodes a
  /// relation names. A fact the toolkit's own control reports (a switch's check) is never
  /// touched unless a view stated it.
  fn apply_access(
    node: &Self::Node,
    previous: &AccessFacts,
    next: &AccessFacts,
    related: &dyn Fn(NodeId) -> Option<Self::Node>,
  );

  /// Reports what `interest` names at `node`: the node's whole interest, installing what is
  /// not installed yet and nothing twice.
  fn watch(node: &Self::Node, interest: Interest);

  /// Moves focus to `node`.
  fn focus(node: &Self::Node);

  /// `node`'s rectangle in window pixels (where it is drawn: an overlay's nodes where the
  /// overlay sits over the window), as last laid out; `None` for a node never laid out.
  fn frame(node: &Self::Node) -> Option<Rect>;

  /// The platform's colors for the environment in force, read under an effect: what the
  /// built-in controls' defaults are drawn in, installed by the facade as the root theme.
  fn system_colors() -> SystemColors;

  /// Chooses the scheme for the whole app, or follows the reader's again (`None`); the
  /// realization reports the scheme in force ([`crate::environment::report`]).
  fn set_scheme(choice: Option<Scheme>);

  /// Polls `task` on the main loop.
  fn spawn_local(task: Task) -> Cancel;

  /// Runs `action` once after `delay` on the platform's clock.
  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Cancel;

  /// Whether `op` is offered on `node` now: absent where the node holds no text it acts on
  /// (the realization offers no clipboard there).
  fn clipboard_availability(_node: &Self::Node, _op: ClipboardOp) -> Availability {
    Availability::Absent
  }

  /// Carries `op` out on `node`, through the platform's clipboard where it cuts, copies or
  /// pastes; nothing where it is not offered.
  fn clipboard(_node: &Self::Node, _op: ClipboardOp) {}

  /// Runs the app: opens its window, reports the environment, then builds `root` inside it
  /// under an owner that lives as long as the window, and returns when the app quits.
  fn start(app_id: &str, root: Box<dyn FnOnce() -> NodeId>) -> ExitCode;
}
