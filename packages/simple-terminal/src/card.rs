//! A ready-made card surface in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleCardBase;
use skene_terminal::ActivePlatform;

/// A ready-made card: a raised, hairline-bordered, rounded surface grouping related
/// content, laid out as a column.
///
/// Its own properties shape the column; wrap it in a [`Container`](skene_terminal::Container) to
/// change the surface itself. Accepts any view or a [`Flexible`](skene_terminal::Flexible) wrapper
/// as a child.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let card = SimpleCard::new()
///   .set_spacing(1)
///   .set_children((Text::new("Title"), Text::new("Body")));
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleCard(skene_simple::SimpleCard<ActivePlatform>);

impl SimpleCardBase<ActivePlatform> for SimpleCard {}
