//! What the terminal resolves beyond a node's own style, through the drawn runtime's
//! resolver at the node: a text entry's value and a focused link take the typography in
//! force there.

use skene_foundation::{Color, StaticLeaf, terminal::BOLD_WEIGHT, typography};
use skene_layout::runtime::Env;

use crate::{Terminal, records::PlainRun};

/// The terminal's own resolutions at a node.
pub(crate) trait TerminalEnv {
  /// The run style a text entry's value paints with: the typography inherited at the
  /// entry, every property resolved.
  fn resolve_entry_run(&self) -> PlainRun;

  /// The colors a link paints in while its focus shows: the typography's focused-link
  /// ink and surface in force at the run, each `None` where unset or transparent.
  fn resolve_link_focus(&self) -> (Option<Color>, Option<Color>);
}

impl TerminalEnv for Env<'_, Terminal> {
  fn resolve_entry_run(&self) -> PlainRun {
    let mut style = self.with_resolver(|resolver| {
      let mut style = PlainRun {
        color: resolver.keyframe_run_value(&StaticLeaf::from(typography::INK)),
        selection_color: resolver.keyframe_run_value(&StaticLeaf::from(typography::SELECTION_INK)),
        selection_background: resolver
          .keyframe_run_value(&StaticLeaf::from(typography::SELECTION_SURFACE)),
        ..PlainRun::default()
      };

      style.ext.weight = resolver.keyframe_run_value(&StaticLeaf::from(typography::WEIGHT));

      style
    });

    style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);

    style
  }

  fn resolve_link_focus(&self) -> (Option<Color>, Option<Color>) {
    self.with_resolver(|resolver| {
      (
        resolver.color_leaf(&StaticLeaf::from(typography::LINK_FOCUS_INK)),
        resolver.color_leaf(&StaticLeaf::from(typography::LINK_FOCUS_SURFACE)),
      )
    })
  }
}
