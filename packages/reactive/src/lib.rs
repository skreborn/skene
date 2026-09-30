//! # Reactive engine
//!
//! Fine-grained reactivity: [`Signal`]s hold state, [`Memo`]s derive from it,
//! [`Effect`]s react to it, and [`Owner`]s tie every entity's lifetime to the subtree
//! that created it. Reads inside an effect subscribe it; writes re-run exactly the
//! subscribed effects, batched by [`batch`] and silenced by [`untrack`].
//!
//! Entities live in generational arenas, one set per thread, joined by pooled
//! dependency edges; every handle stays on the thread that made it, and work on other
//! threads reaches it through a task that wakes there.
//!
//! Every serializable signal takes part in state snapshots by position
//! ([`snapshot`]/[`restore`]); a [`Key`] anchors a scope's position against edits and
//! reordering around it, and [`Signal::transient`] opts out. A [`struct@Store`] adds
//! field-level subscriptions over one value, and a [`Resource`] loads a value through
//! the executor a platform installs ([`set_spawner`]).

mod access;
mod arena;
mod computation;
mod context;
mod dispose;
mod effect;
mod encode;
mod graph;
mod guards;
pub mod hash;
mod id;
mod link;
mod memo;
mod obituary;
mod owner;
mod path;
mod resource;
mod runtime;
mod selector;
mod signal;
mod snapshot;
mod spawn;
mod store;
mod tracked_cell;
mod trigger;

pub use access::{Get, Read, Set, Track, Update, With, Write};
pub use context::{provide_context, use_context};
pub use effect::{Effect, EffectHandle, batch, untrack};
pub use encode::{Fields, Reader, Record, Recorder, Seq, Snapshot, Value, decode, encode};
pub use guards::{ReadGuard, WriteGuard};
pub use hash::field_key;
pub use id::Id as ReactiveId;
pub use memo::Memo;
pub use owner::{Owner, on_cleanup};
pub use path::{Key, SnapshotPlace};
pub use resource::{LoadState, Resource};
pub use runtime::Runtime;
pub use selector::Selector;
pub use signal::{ReadSignal, Signal, WriteSignal, signal};
pub use skene_macros::{Snapshot, Store};
pub use snapshot::{provide_extra, record_snapshots, restore, snapshot, take_extra};
pub use spawn::{Task, TaskGuard, TaskHandle, set_spawner, spawn};
pub use store::{
  AtIndex,
  AtKey,
  IndexedField,
  KeyedBy,
  KeyedCollection,
  KeyedField,
  Store,
  StoreCore,
  StoreField,
  StoreLocation,
  Subfield,
};
pub use trigger::Trigger;
