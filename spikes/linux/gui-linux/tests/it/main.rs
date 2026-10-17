//! skene-gui-linux's integration tests, every one through the driver on the display runner's
//! session: a window shown on the leg's display server, read as a reader reads it (AT-SPI)
//! and as the screen shows it, acted on by real input.

mod chrome;
mod contents;
mod desktop;
mod dnd;
mod gestures;
mod glyphs;
#[cfg(feature = "gpu")]
mod gpu;
mod menu;
mod presented;
mod reader;
mod roles;
mod rows;
mod scale;
mod scroll;
mod secure;
mod selection;

use skene_foundation::{
  NodeId,
  seam::{node::NodeSpec, record::Record, style::ScopeChain, tree},
};
use skene_gui_linux::Linux;

/// A node of `spec` with `record`, built through the seam as a facade builds it.
fn node(spec: NodeSpec, record: Record) -> NodeId {
  let id = tree::create::<Linux>(spec, &ScopeChain::root(), None);

  tree::set_record::<Linux>(id, record);

  id
}
