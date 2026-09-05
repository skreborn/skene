//! A handle reporting a built view's measured size.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{Driver, NodeId, Size};
use skene_reactive::{Get, Set, Signal};

use crate::realize::with_sink;

/// A handle reporting the measured size of the view it is bound to.
///
/// Bound to a box through its `bind_size`; [`size`](Self::size) is `None` until the
/// driver has measured the box and tracks every later measurement, so a view that lays
/// out by its own size derives from it.
#[must_use]
pub struct SizeHandle<D: Driver> {
  size: Signal<Option<Size<D::Scalar>>>,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SizeHandle<D> {
  /// An unbound handle.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the handle to `node`, a built box, and starts observing its size.
  pub fn assign(&self, node: NodeId) {
    let size = self.size;

    with_sink::<D, _>(node, |sink| {
      D::default().observe_size(sink, Rc::new(move |measured| size.set(Some(measured))));
    });
  }

  /// The last measured size, tracked; `None` before the first measurement.
  #[inline(always)]
  #[must_use]
  pub fn size(&self) -> Option<Size<D::Scalar>> {
    self.size.get()
  }
}

impl<D: Driver> Clone for SizeHandle<D> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      size: self.size,
      driver: PhantomData,
    }
  }
}

impl<D: Driver> Default for SizeHandle<D> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      size: Signal::opaque(None),
      driver: PhantomData,
    }
  }
}
