//! What every client shares on a host: the pool its blocking work runs on, the
//! one-shot a worker settles a future through, and the one TLS configuration.

use std::{
  future::Future,
  panic::{AssertUnwindSafe, catch_unwind},
  pin::Pin,
  sync::{
    Arc,
    Mutex,
    OnceLock,
    PoisonError,
    atomic::{AtomicUsize, Ordering},
    mpsc,
  },
  task::{Context, Poll, Waker},
  thread,
};

use rustls::{ClientConfig, crypto::CryptoProvider};
use rustls_platform_verifier::BuilderVerifierExt as _;

/// The most threads the pool runs work on at once.
const WORKERS: usize = 8;

/// A unit of blocking work.
type Job = Box<dyn FnOnce() + Send>;

/// The pool: the queue its workers take jobs from, how many there are, and how many
/// wait for a job.
struct Pool {
  sender: mpsc::Sender<Job>,
  receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
  workers: AtomicUsize,
  idle: AtomicUsize,
}

static POOL: OnceLock<Pool> = OnceLock::new();

/// Runs `job` on the pool, a worker started for it while fewer than [`WORKERS`] run
/// and none is idle; a job that panics costs the pool nothing.
pub(crate) fn run(job: impl FnOnce() + Send + 'static) {
  let pool = POOL.get_or_init(|| {
    let (sender, receiver) = mpsc::channel();

    Pool {
      sender,
      receiver: Arc::new(Mutex::new(receiver)),
      workers: AtomicUsize::new(0),
      idle: AtomicUsize::new(0),
    }
  });

  let _ = pool.sender.send(Box::new(job));

  #[expect(
    deprecated,
    reason = "`try_update`, the name `fetch_update` is deprecated for, is unstable at the workspace's `rust-version`"
  )]
  if pool.idle.load(Ordering::Acquire) == 0
    && pool
      .workers
      .fetch_update(Ordering::AcqRel, Ordering::Acquire, |workers| {
        (workers < WORKERS).then_some(workers + 1)
      })
      .is_ok()
  {
    let receiver = Arc::clone(&pool.receiver);

    thread::spawn(move || {
      loop {
        pool.idle.fetch_add(1, Ordering::AcqRel);

        let job = receiver
          .lock()
          .unwrap_or_else(PoisonError::into_inner)
          .recv();

        pool.idle.fetch_sub(1, Ordering::AcqRel);

        let Ok(job) = job
        else {
          break;
        };

        let _ = catch_unwind(AssertUnwindSafe(job));
      }
    });
  }
}

/// What a settler and its promise share: the outcome once there is one, and the waker
/// to fire then.
struct Shared<T> {
  outcome: Option<T>,
  waker: Option<Waker>,
}

/// The settling end of a one-shot, held by the worker: settled once, or with its
/// fallback if dropped unsettled, as a panicking worker drops it.
pub(crate) struct Settler<T> {
  shared: Arc<Mutex<Shared<T>>>,
  fallback: Option<T>,
}

impl<T> Settler<T> {
  /// Settles the promise with `outcome`.
  pub(crate) fn settle(mut self, outcome: T) {
    self.fallback = None;
    self.set(outcome);
  }

  fn set(&self, outcome: T) {
    let waker = {
      let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);

      shared.outcome = Some(outcome);
      shared.waker.take()
    };

    if let Some(waker) = waker {
      waker.wake();
    }
  }
}

impl<T> Drop for Settler<T> {
  fn drop(&mut self) {
    if let Some(fallback) = self.fallback.take() {
      self.set(fallback);
    }
  }
}

/// The awaiting end of a one-shot: resolves with what its settler settled.
pub(crate) struct Promise<T>(Arc<Mutex<Shared<T>>>);

impl<T> Future for Promise<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<T> {
    let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);

    if let Some(outcome) = shared.outcome.take() {
      return Poll::Ready(outcome);
    }

    shared.waker = Some(context.waker().clone());

    Poll::Pending
  }
}

/// A one-shot whose promise resolves with `fallback` if its settler is dropped
/// unsettled.
pub(crate) fn one_shot<T>(fallback: T) -> (Settler<T>, Promise<T>) {
  let shared = Arc::new(Mutex::new(Shared {
    outcome: None,
    waker: None,
  }));

  (
    Settler {
      shared: Arc::clone(&shared),
      fallback: Some(fallback),
    },
    Promise(shared),
  )
}

/// The crypto provider every connection uses, even where a dependency compiles in
/// another.
pub(crate) fn provider() -> Arc<CryptoProvider> {
  static PROVIDER: OnceLock<Arc<CryptoProvider>> = OnceLock::new();

  Arc::clone(PROVIDER.get_or_init(|| Arc::new(rustls::crypto::ring::default_provider())))
}

/// The TLS client configuration: the [`provider`] over the platform's own trust store.
///
/// # Errors
///
/// The platform's trust store could not be read.
pub(crate) fn tls_config() -> Result<Arc<ClientConfig>, rustls::Error> {
  static CONFIG: OnceLock<Result<Arc<ClientConfig>, rustls::Error>> = OnceLock::new();

  CONFIG
    .get_or_init(|| {
      ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_platform_verifier()
        .map(|builder| Arc::new(builder.with_no_client_auth()))
    })
    .clone()
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A promise resolves with what its settler settled, or with the fallback once the
  /// settler is dropped unsettled, as by a worker's panic.
  #[test]
  fn a_promise_resolves_even_when_its_worker_panics() {
    let block_on = |promise: Promise<u32>| {
      let mut promise = Box::pin(promise);
      let mut context = Context::from_waker(Waker::noop());

      loop {
        if let Poll::Ready(value) = promise.as_mut().poll(&mut context) {
          return value;
        }

        thread::yield_now();
      }
    };
    let (settler, promise) = one_shot(0);

    run(move || settler.settle(1));

    assert_eq!(block_on(promise), 1);

    let (settler, promise) = one_shot(0);

    run(move || {
      let _settler = settler;

      panic!("a worker panics");
    });

    assert_eq!(block_on(promise), 0);

    let (settler, promise) = one_shot(0);

    run(move || settler.settle(2));

    assert_eq!(block_on(promise), 2, "the pool survives the panic");
  }
}
