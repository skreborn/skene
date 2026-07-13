//! Web realization of [`Span`]: an inline text run, a nested group, or an embedded view.

use skene_core::{NodeId, View};

use super::{
  super::{Declarations, set_text, tree},
  open_run,
};
use crate::{
  Span,
  text::{SpanContent, provide_group_base},
};

impl View for Span {
  fn build(self) -> NodeId {
    // A run is inline; a view content box is inline-level so it flows among the text.
    let initial = if matches!(self.content, SpanContent::View(_)) {
      Declarations::new().set("display", "inline-block")
    }
    else {
      Declarations::new()
    };

    let (node, resolved) = open_run(self.link.as_deref(), self.style, initial);

    match self.content {
      SpanContent::Text(source) => {
        source.bind(move |text| set_text(node, &text));
      }
      SpanContent::Group(children) => {
        // The group's resolved style becomes the inherited base for its nested spans.
        provide_group_base(resolved, || {
          for child in children {
            let child = child.build();

            tree::attach(node, child, None);
          }
        });
      }
      SpanContent::View(view) => {
        let child = view.build();

        tree::attach(node, child, None);
      }
    }

    node
  }
}
