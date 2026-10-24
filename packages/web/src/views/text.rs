//! A text run on its own line.

use skene_foundation::{HasRun, RunBase, TextBase, View, Wraps};

use crate::{Active, LinkPolicy};

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
///
/// ```
/// use skene_web::prelude::*;
///
/// let count = Signal::new(0);
/// let label =
///   Text::derive(move || format!("{} items", count.get())).set_color(Color::srgb(0.2, 0.2, 0.2));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Text(skene_foundation::Text<Active>);

impl Text {
  /// Sets how the link opens and what it discloses; a link left without one follows the
  /// default [`LinkPolicy`].
  pub fn set_link_policy(self, policy: LinkPolicy) -> Self {
    self.map_run(|run| run.add_realization(move |node, spec, _| policy.realize(node, spec)))
  }
}

impl TextBase<Active> for Text {}

impl RunBase<Active> for Text {}
