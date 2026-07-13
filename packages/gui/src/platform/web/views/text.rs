//! Web realization of [`Text`] (inheritance is resolved in the shared `text` module).

use skene_core::{NodeId, View};

use super::{
  super::{Declarations, set_text},
  open_run,
};
use crate::Text;

impl View for Text {
  fn build(self) -> NodeId {
    // A block run: its own line, unlike an inline `Span`.
    let (node, _) = open_run(
      self.link.as_deref(),
      self.style,
      Declarations::new().set("display", "block"),
    );

    self.source.bind(move |text| set_text(node, &text));

    node
  }
}
