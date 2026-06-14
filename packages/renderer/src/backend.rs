use std::rc::Rc;

use crate::{ElementKind, Event, EventKind, NodeId, Role, Style};

/// A sink that a backend calls when an event fires on a node.
///
/// The core installs one of these via [`Backend::set_event_sink`]; the backend
/// invokes it with the originating node and the abstract event. The core is then
/// responsible for routing it to the relevant handler.
pub type EventSink = Rc<dyn Fn(NodeId, Event)>;

/// The renderer abstraction.
///
/// A `Backend` owns a retained tree of nodes and realizes the core's mutations
/// against some target: the DOM, a native window, a terminal. The core never
/// talks to the target directly — it only issues these tree mutations and reads
/// back events through the [`EventSink`].
///
/// The contract is intentionally retained-mode and high level so that very
/// different targets can implement it:
///
/// * **DOM** maps nodes to elements and offloads layout, painting, text shaping, hit-testing and
///   accessibility to the browser.
/// * **Native** (future) would run its own layout (e.g. taffy), paint (e.g. vello), text (e.g.
///   parley) and accessibility (e.g. accesskit) behind this same trait.
/// * **TUI** (future) would run its own cell layout and drawing.
pub trait Backend {
  /// The root node of the tree. Created by the backend at construction; views
  /// are inserted under it.
  fn root(&self) -> NodeId;

  /// Allocate a new node of the given semantic kind. It is detached until
  /// inserted with [`Backend::insert_child`].
  fn create_element(&mut self, kind: ElementKind) -> NodeId;

  /// Apply a style to a node, replacing any previously applied style.
  fn set_style(&mut self, node: NodeId, style: &Style);

  /// Set the text content of a node.
  fn set_text(&mut self, node: NodeId, text: &str);

  /// Set (`Some`) or remove (`None`) an attribute on a node.
  fn set_attribute(&mut self, node: NodeId, name: &str, value: Option<&str>);

  /// Set a node's accessibility [`Role`].
  fn set_role(&mut self, node: NodeId, role: Role);

  /// Insert `child` under `parent`. If `before` is `Some`, insert immediately
  /// before that existing child; otherwise append.
  fn insert_child(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>);

  /// Detach `child` from `parent` without destroying it.
  fn remove_child(&mut self, parent: NodeId, child: NodeId);

  /// Destroy a node and release its resources. The node must already be
  /// detached.
  fn destroy(&mut self, node: NodeId);

  /// Subscribe a node to a category of events. Fired events are delivered
  /// through the installed [`EventSink`].
  fn listen(&mut self, node: NodeId, kind: EventKind);

  /// Install the sink that receives events from this backend.
  fn set_event_sink(&mut self, sink: EventSink);
}
