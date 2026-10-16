//! Tasks, timers and resources on the platform's main loop (on Linux, the window's event loop),
//! bound to the current owner.

use std::{
  cell::{Cell, RefCell},
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
  time::Duration,
};

use skene_foundation::{
  Effect, Signal,
  seam::{Realization as _, runtime::Cancel},
  untrack,
};

use crate::Active;

/// Runs `task` on the platform's main loop, under the current owner: it is dropped, never
/// polled again, when that owner is disposed.
pub fn spawn(task: impl Future<Output = ()> + 'static) {
  skene_foundation::seam::runtime::spawn::<Active>(task);
}

/// Runs `action` once after `delay` on the platform's clock, cancelled when the current owner
/// is disposed first.
pub fn schedule(delay: Duration, action: impl FnOnce() + 'static) {
  skene_foundation::seam::runtime::schedule::<Active>(delay, action);
}

/// Waits `delay` on the platform's clock: the timer is cancelled when the future is dropped.
pub fn sleep(delay: Duration) -> Sleep {
  Sleep {
    delay,
    state: Rc::new(SleepState::default()),
    timer: None,
  }
}

#[derive(Default)]
struct SleepState {
  done: Cell<bool>,
  waker: RefCell<Option<Waker>>,
}

/// The future [`sleep`] returns.
#[must_use = "a sleep waits only when awaited"]
pub struct Sleep {
  delay: Duration,
  state: Rc<SleepState>,
  timer: Option<Cancel>,
}

impl Future for Sleep {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    if self.state.done.get() {
      return Poll::Ready(());
    }

    *self.state.waker.borrow_mut() = Some(context.waker().clone());

    if self.timer.is_none() {
      let state = Rc::clone(&self.state);

      self.timer = Some(Active::schedule(
        self.delay,
        Box::new(move || {
          state.done.set(true);

          if let Some(waker) = state.waker.borrow_mut().take() {
            waker.wake();
          }
        }),
      ));
    }

    Poll::Pending
  }
}

impl Drop for Sleep {
  fn drop(&mut self) {
    if let Some(timer) = self.timer.take()
      && !self.state.done.get()
    {
      timer.cancel();
    }
  }
}

/// A value loaded asynchronously from a source: whenever a signal `source` reads changes, the
/// load starts again and the previous one is dropped, never finishing; the value stays what the
/// last finished load gave until the next one finishes.
///
/// It belongs to the owner it was created under: disposing that owner drops a load in flight.
///
/// ```
/// use std::time::Duration;
///
/// use skene_gui::{prelude::*, sleep};
///
/// let id = Signal::new(1);
/// let user = Resource::new(
///   move || id.get(),
///   |id| async move {
///     sleep(Duration::from_millis(10)).await;
///     format!("user {id}")
///   },
/// );
/// let shown = Text::derive(move || user.get().unwrap_or_else(|| "Loading…".to_owned()));
/// ```
#[must_use]
pub struct Resource<T: 'static> {
  value: Signal<Option<T>>,
  loading: Signal<bool>,
}

impl<T: 'static> Clone for Resource<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: Clone + 'static> Resource<T> {
  /// The value `load` produces for what `source` returns, loaded again whenever it changes.
  pub fn new<S: 'static, F: Future<Output = T> + 'static>(
    source: impl Fn() -> S + 'static,
    load: impl Fn(S) -> F + 'static,
  ) -> Self {
    let value = Signal::new(None);
    let loading = Signal::new(true);

    Effect::new(move || {
      let source = source();
      let future = untrack(|| load(source));

      untrack(|| loading.set(true));
      spawn(async move {
        let loaded = future.await;

        value.set(Some(loaded));
        loading.set(false);
      });
    });

    Self { value, loading }
  }

  /// The last value loaded, if one finished, subscribing the running effect.
  #[must_use]
  pub fn get(&self) -> Option<T> {
    self.value.get()
  }

  /// Whether a load is in flight, subscribing the running effect.
  #[must_use]
  pub fn loading(&self) -> bool {
    self.loading.get()
  }
}
