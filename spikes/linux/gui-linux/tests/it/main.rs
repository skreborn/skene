//! skene-gui-linux's integration tests: the realization driven through the seam as the facade
//! drives it, and its window shell driven as a backend drives it, painting into its own frame
//! with no display: layout, paint, hit testing, states, focus order and activation.

mod chrome;
mod contents;
mod dnd;
mod environment;
mod gestures;
#[cfg(feature = "gpu")]
mod gpu;
mod keyboard;
mod reader;
mod scale;
mod secure;
mod shell;

use skene_foundation::{
  NodeId, Owner,
  seam::{
    node::{LayoutKind, NodeKind, NodeSpec},
    record::Record,
    style::ScopeChain,
    tree,
  },
};
use skene_gui_linux::{Linux, node::Node};

/// A node of `spec` with `record`, its realized node.
fn node(spec: NodeSpec, record: Record) -> (NodeId, Node) {
  let id = tree::create::<Linux>(spec, &ScopeChain::root(), None);

  tree::set_record::<Linux>(id, record);

  (id, tree::native::<Linux>(id).expect("realized"))
}

/// A box of `record`.
fn boxed(record: Record) -> (NodeId, Node) {
  node(NodeSpec::new(NodeKind::Layout(LayoutKind::Box)), record)
}

/// Runs `body` under an owner, disposed after.
fn owned(body: impl FnOnce()) {
  let owner = Owner::root();

  owner.run(body);
  owner.dispose();
}
