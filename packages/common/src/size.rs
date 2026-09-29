//! A handle reporting a built view's measured size.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{NodeId, Platform, Size};
use skene_reactive::{Get, Set, Signal};

use crate::realize::with_sink;

/// A handle reporting the measured size of the view it is bound to.
///
/// Bound to a box through its `bind_size`; [`size`](Self::size) is `None` until the
/// platform has measured the box and tracks every later measurement, so a view that lays
/// out by its own size derives from it.
#[derive(Debug)]
#[must_use]
pub struct SizeHandle<P: Platform> {
  size: Signal<Option<Size<P::Scalar>>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SizeHandle<P> {
  /// An unbound handle.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the handle to `node`, a built box, and starts observing its size.
  pub fn assign(&self, node: NodeId) {
    let size = self.size;

    with_sink::<P, _>(node, |sink| {
      P::observe_size(sink, Rc::new(move |measured| size.set(Some(measured))));
    });
  }

  /// The last measured size, tracked; `None` before the first measurement.
  #[inline(always)]
  #[must_use]
  pub fn size(&self) -> Option<Size<P::Scalar>> {
    self.size.get()
  }
}

impl<P: Platform> Clone for SizeHandle<P> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      size: self.size,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Default for SizeHandle<P> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      size: Signal::opaque(None),
      platform: PhantomData,
    }
  }
}
