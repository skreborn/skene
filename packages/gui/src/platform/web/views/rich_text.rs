//! Web realization of [`RichText`]: a block that flows its inline [`Span`]s.

use skene_core::{NodeId, View};

use super::super::{create_element, tree};
use crate::RichText;

impl View for RichText {
  fn build(self) -> NodeId {
    // A block box; the inline spans inside flow and wrap within it.
    let node = create_element("div");

    for span in self.spans {
      let child = span.build();

      tree::attach(node, child, None);
    }

    node
  }
}
