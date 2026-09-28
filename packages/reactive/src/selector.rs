//! Membership tracking over one key-valued source.

use std::hash::Hash;

use rustc_hash::FxHashMap;

use crate::{
  access::{Get, With},
  effect::{Effect, batch},
  memo::Memo,
  owner::Owner,
  runtime::Runtime,
  signal::{Signal, update_silently},
  trigger::Trigger,
};

/// Tracks which key a source currently yields, so a reader asking about one key
/// re-runs only when that key's membership flips.
///
/// One tracked run of the source per change fans out to at most two readers' worth
/// of triggers: the key left and the key entered. An answer is the source's own,
/// brought up to date as it is asked, so a batch or a render effect never hears an
/// old key; a key nothing asks about any more is forgotten.
#[must_use]
pub struct Selector<K: Eq + Hash + Clone + 'static> {
  current: Memo<K>,
  keys: Signal<FxHashMap<K, Trigger>>,
  owner: Owner,
}

impl<K: Eq + Hash + Clone + 'static> Copy for Selector<K> {}

#[expect(
  clippy::expl_impl_clone_on_copy,
  reason = "a derive would bound the key on Copy"
)]
impl<K: Eq + Hash + Clone + 'static> Clone for Selector<K> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<K: Eq + Hash + Clone + 'static> Selector<K> {
  /// Creates a selector over `source`, owned by the current [`Owner`].
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(source: impl Fn() -> K + 'static) -> Self {
    let owner = Owner::current();
    let current = Memo::new(move |_| source());
    let keys = Signal::opaque(FxHashMap::<K, Trigger>::default());

    Effect::render(move |previous: Option<K>| {
      let next = current.get();

      if previous.as_ref() != Some(&next) {
        let flipped: Option<(Option<Trigger>, Option<Trigger>)> = keys.try_with_untracked(|keys| {
          (
            previous
              .as_ref()
              .and_then(|previous| keys.get(previous).copied()),
            keys.get(&next).copied(),
          )
        });

        if let Some((left, entered)) = flipped {
          batch(|| {
            left.iter().chain(&entered).for_each(Trigger::notify);
          });
        }
      }

      next
    });

    Self {
      current,
      keys,
      owner,
    }
  }

  /// Whether the source currently yields `key`, subscribing the running computation
  /// to changes of exactly that answer.
  #[must_use]
  pub fn is(&self, key: &K) -> bool {
    if Runtime::is_tracking() {
      let (owner, keys) = (self.owner, self.keys);
      let trigger = update_silently(keys.id(), |triggers: &mut FxHashMap<K, Trigger>| {
        if let Some(trigger) = triggers.get(key) {
          return (*trigger, false);
        }

        let trigger = owner.enter(Trigger::new);

        triggers.insert(key.clone(), trigger);

        (trigger, true)
      });

      if let Some((trigger, created)) = trigger {
        if created {
          let forgotten = key.clone();

          trigger.dispose_when_unobserved(move || {
            let _ = update_silently(keys.id(), |triggers: &mut FxHashMap<K, Trigger>| {
              triggers.remove(&forgotten);
            });
          });
        }

        trigger.track();
      }
    }

    self
      .current
      .try_with_untracked(|current| current == key)
      .unwrap_or(false)
  }
}
