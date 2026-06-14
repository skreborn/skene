//! # skene Reactive
//!
//! [`RwSignal::new_split`](RwSignal::new_split) returns a separated [`ReadSignal`] and
//! [`WriteSignal`] for a variable. An existing `RwSignal` may be converted using
//! [`RwSignal::read_only`](RwSignal::read_only) and [`RwSignal::write_only`](RwSignal::write_only)
//! where necessary, but the reverse is not possible.

mod base;
mod context;
mod derived;
mod effect;
mod id;
mod impls;
mod memo;
mod read;
mod runtime;
mod scope;
mod signal;
mod storage;
mod sync_runtime;
mod trigger;
mod write;

pub use base::{BaseSignal, SyncBaseSignal};
pub use context::Context;
pub use derived::{
  DerivedRwSignal,
  SyncDerivedRwSignal,
  create_derived_rw_signal,
  create_sync_derived_rw_signal,
};
pub use effect::{Effect, EffectTrait, SignalTracker, UpdaterEffect};
pub use id::Id as ReactiveId;
pub use memo::Memo;
pub use read::{ReadRef, SignalGet, SignalRead, SignalTrack, SignalWith};
pub use runtime::Runtime;
pub use scope::Scope;
pub use signal::{
  ReadSignal,
  RwSignal,
  SyncReadSignal,
  SyncRwSignal,
  SyncWriteSignal,
  WriteSignal,
};
pub use trigger::Trigger;
pub use write::{SignalUpdate, SignalWrite, WriteRef};
