//! A text run on its own line.

use skene_foundation::{HasRun, RunBase, TextBase, View, Wraps};

use crate::{ActivePlatform, LinkPolicy};

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Text(skene_foundation::Text<ActivePlatform>);

impl Text {
  /// Sets how the link opens and what it discloses; a link left without one follows the
  /// default [`LinkPolicy`].
  pub fn set_link_policy(self, policy: LinkPolicy) -> Self {
    self.map_run(|run| run.add_realization(move |node, props| policy.realize(node, props)))
  }
}

impl TextBase<ActivePlatform> for Text {}

impl RunBase<ActivePlatform> for Text {}
