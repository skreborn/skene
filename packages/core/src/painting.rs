//! The portable low-level path for pixel media: a paint callback on a box.

use super::Driver;
use crate::Size;

/// The surface a paint callback draws on.
///
/// A placeholder until a pixel driver realizes it: no driver constructs one yet, so a
/// callback taking it compiles against every driver and runs on none.
#[derive(Debug)]
#[must_use]
pub struct Canvas {
  _private: (),
}

/// A paint callback: draws into the canvas at the node's laid-out size.
pub type Painter = dyn Fn(&mut Canvas, Size<f32>);

/// A driver that paints a callback into a box.
///
/// The bound a portable low-level view states for pixel media: the view builds an
/// ordinary box for its sizing and hands the driver a painter for it, which the driver
/// runs whenever the box repaints. A driver that cannot paint (the terminal, a server)
/// does not implement it, so the view does not exist there.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not paint",
  label = "a painted view is written against a pixel driver implementing `Painting`; the terminal and the server leaf do not"
)]
pub trait Painting: Driver {
  /// Paints `painter` into the box `sink` at its laid-out size, now and whenever the
  /// box repaints; a later call replaces the painter.
  fn paint(&self, sink: &Self::Sink, painter: Box<Painter>);
}
