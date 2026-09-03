use std::{any::Any, cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{
  access::{Get, Track, With},
  computation::{self, Computation},
  graph::flags::MEMO,
  id::Id,
  runtime::{Runtime, with_graph},
};

/// A memoized derived value.
///
/// Computed once on creation, then lazily: a read after a dependency changed
/// recomputes first, and nothing recomputes while nobody reads. Dependents are
/// notified only when the recomputed value differs under the memo's comparator.
#[must_use]
pub struct Memo<T: 'static> {
  id: Id,
  ty: PhantomData<T>,
}

impl<T: 'static> Copy for Memo<T> {}

impl<T: 'static> Clone for Memo<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: Clone + 'static> Get<T> for Memo<T> {
  fn id(&self) -> Id {
    self.id
  }

  fn try_get(&self) -> Option<T> {
    self.with_value(true, Clone::clone)
  }

  fn try_get_untracked(&self) -> Option<T> {
    self.with_value(false, Clone::clone)
  }
}

impl<T: 'static> Track for Memo<T> {
  fn id(&self) -> Id {
    self.id
  }
}

impl<T: 'static> With<T> for Memo<T> {
  fn id(&self) -> Id {
    self.id
  }

  fn try_with<O>(&self, f: impl FnOnce(&T) -> O) -> Option<O> {
    self.with_value(true, f)
  }

  fn try_with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> Option<O> {
    self.with_value(false, f)
  }
}

impl<T: PartialEq + 'static> Memo<T> {
  /// Creates a memo computed by `f`, which receives the previous value or `None` on
  /// its first run; dependents are notified when the value is not equal to before.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(f: impl Fn(Option<&T>) -> T + 'static) -> Self {
    Self::new_with(PartialEq::eq, f)
  }
}

impl<T: 'static> Memo<T> {
  /// Creates a memo computed by `f`, notifying dependents when `compare` says the
  /// recomputed value differs from the previous one.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new_with(
    compare: impl Fn(&T, &T) -> bool + 'static,
    f: impl Fn(Option<&T>) -> T + 'static,
  ) -> Self {
    Runtime::assert_ui_thread();

    let body: Rc<dyn Computation> = Rc::new(MemoBody {
      compute: Box::new(f),
      compare: Box::new(compare),
      value: RefCell::new(None),
    });
    let id = computation::create(body, MEMO);

    computation::run(id);

    Self {
      id,
      ty: PhantomData,
    }
  }

  /// The memo's id.
  #[inline(always)]
  pub fn id(&self) -> Id {
    self.id
  }

  /// Brings the memo up to date and passes its value to `f`, subscribing the running
  /// computation when `track` is set; `None` when the memo is disposed.
  fn with_value<O>(&self, track: bool, f: impl FnOnce(&T) -> O) -> Option<O> {
    computation::ensure_fresh(self.id);

    let body = with_graph(|graph| graph.body(self.id))?;

    if track {
      computation::subscribe(self.id);
    }

    let body = body
      .as_any()
      .downcast_ref::<MemoBody<T>>()
      .expect("to downcast memo type");
    let value = body.value.borrow();

    Some(f(value.as_ref().expect(
      "memo has no value: it is computing, or its computation panicked",
    )))
  }
}

type ComputeFn<T> = Box<dyn Fn(Option<&T>) -> T>;
type CompareFn<T> = Box<dyn Fn(&T, &T) -> bool>;

/// The body behind [`Memo`]: the compute closure, its comparator, and the value.
struct MemoBody<T> {
  compute: ComputeFn<T>,
  compare: CompareFn<T>,
  value: RefCell<Option<T>>,
}

impl<T: 'static> Computation for MemoBody<T> {
  fn run(&self) -> bool {
    let previous = self.value.borrow_mut().take();
    let next = (self.compute)(previous.as_ref());
    let changed = previous
      .as_ref()
      .is_none_or(|previous| !(self.compare)(previous, &next));

    *self.value.borrow_mut() = Some(next);

    changed
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
