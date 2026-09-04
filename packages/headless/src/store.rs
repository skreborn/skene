//! The record store: what the driver keeps per node, and the environment it reports.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use skene_core::{
  ColorScheme,
  Size,
  next::{
    BoxProps,
    BoxValue,
    ContainerName,
    ContainerSizes,
    Definitions,
    EventKind,
    MediaSignals,
    RunProps,
    RunValue,
    Slot,
  },
};
use skene_reactive::{Owner, WriteSignal, signal};

use crate::{Handle, Headless, HeadlessTokenValue};

/// The viewport the driver reports until [`set_viewport`](crate::set_viewport) is called.
pub const DEFAULT_VIEWPORT: Size<f32> = Size::new(1024., 768.);

/// What a record is, with everything its creation call carried.
pub enum Kind {
  /// A box, with the props it was created from.
  Box(Box<BoxProps<Headless>>),
  /// A text run, with the props it was created from.
  Run(Box<RunProps<Headless>>),
  /// An anchor.
  Anchor,
  /// A scope.
  Scope,
}

/// The latest value a render effect wrote into one slot.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum SlotValue {
  /// A box property's value.
  Box(BoxValue<f32>),
  /// A run property's value.
  Run(RunValue),
}

/// One realized node.
pub struct Record {
  pub kind: Kind,
  pub text: String,
  pub value: String,
  pub checked: bool,
  pub hovered: bool,
  pub slots: FxHashMap<Slot, SlotValue>,
  pub definitions: Option<Definitions<HeadlessTokenValue>>,
  pub listened: Vec<EventKind>,
  pub children: Vec<Handle>,
  pub size_observers: Vec<Rc<dyn Fn(Size<f32>)>>,
}

impl Record {
  pub fn new(kind: Kind) -> Self {
    Self {
      kind,
      text: String::new(),
      value: String::new(),
      checked: false,
      hovered: false,
      slots: FxHashMap::default(),
      definitions: None,
      listened: Vec::new(),
      children: Vec::new(),
      size_observers: Vec::new(),
    }
  }
}

struct Entry {
  generation: u32,
  record: Option<Record>,
}

/// The environment as signals, with the write halves the seams drive.
pub struct MediaState {
  pub owner: Owner,
  pub set_viewport: WriteSignal<Size<f32>>,
  pub set_scheme: WriteSignal<ColorScheme>,
  pub set_reduced_motion: WriteSignal<bool>,
  pub signals: MediaSignals<f32>,
}

/// Every record the driver holds, the focus, the root definitions and the environment.
pub struct Store {
  entries: Vec<Entry>,
  free: Vec<u32>,
  pub focused: Option<Handle>,
  pub root_definitions: Definitions<HeadlessTokenValue>,
  pub root_definition_writes: usize,
  pub container_sizes: FxHashMap<ContainerName, Size<f32>>,
  pub viewport: Option<Size<f32>>,
  pub scheme: Option<ColorScheme>,
  pub reduced_motion: Option<bool>,
  media: Option<MediaState>,
}

impl Default for Store {
  fn default() -> Self {
    Self {
      entries: Vec::new(),
      free: Vec::new(),
      focused: None,
      root_definitions: Definitions::from(Vec::new()),
      root_definition_writes: 0,
      container_sizes: FxHashMap::default(),
      viewport: None,
      scheme: None,
      reduced_motion: None,
      media: None,
    }
  }
}

impl Store {
  /// Mints a record and returns its handle.
  pub fn create(&mut self, record: Record) -> Handle {
    if let Some(index) = self.free.pop() {
      let entry = &mut self.entries[index as usize];

      entry.record = Some(record);

      return Handle::new(index, entry.generation);
    }

    let index = u32::try_from(self.entries.len()).expect("a store holds at most 2^32 records");

    self.entries.push(Entry {
      generation: 0,
      record: Some(record),
    });

    Handle::new(index, 0)
  }

  /// The record of `handle`, if it is live.
  #[inline(always)]
  pub fn record(&self, handle: Handle) -> Option<&Record> {
    self
      .entries
      .get(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_ref())
  }

  /// The record of `handle`, mutably, if it is live.
  #[inline(always)]
  pub fn record_mut(&mut self, handle: Handle) -> Option<&mut Record> {
    self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_mut())
  }

  /// Releases the record of `handle`, retiring the handle.
  pub fn destroy(&mut self, handle: Handle) {
    let Some(entry) = self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
    else {
      return;
    };

    if entry.record.take().is_none() {
      return;
    }

    entry.generation = entry.generation.wrapping_add(1);
    self.free.push(handle.index);

    if self.focused == Some(handle) {
      self.focused = None;
    }
  }

  /// The environment's signals, created on first use under a root owner of their own.
  pub fn media(&mut self) -> &MediaState {
    let viewport = self.viewport.unwrap_or(DEFAULT_VIEWPORT);
    let scheme = self.scheme.unwrap_or(ColorScheme::Light);
    let reduced_motion = self.reduced_motion.unwrap_or(false);

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (viewport, set_viewport) = signal(viewport);
        let (scheme, set_scheme) = signal(scheme);
        let (reduced_motion, set_reduced_motion) = signal(reduced_motion);

        MediaState {
          owner,
          set_viewport,
          set_scheme,
          set_reduced_motion,
          signals: MediaSignals {
            viewport,
            scheme,
            reduced_motion,
            container_sizes: ContainerSizes::new(),
          },
        }
      })
    })
  }

  /// Forgets everything and hands back the owner of the environment's signals, for
  /// the caller to dispose once the store is released.
  pub fn reset(&mut self) -> Option<Owner> {
    let media = self.media.take();

    *self = Self::default();

    media.map(|media| media.owner)
  }
}
