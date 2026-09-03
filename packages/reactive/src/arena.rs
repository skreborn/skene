/// One generational slot: the value it currently holds, if any, and the generation
/// ids must carry to resolve.
struct Slot<T> {
  generation: u16,
  value: Option<T>,
}

/// A generational arena.
///
/// Slots are reused through a free list; freeing bumps the generation, so ids minted
/// for the previous occupant resolve to nothing. A slot whose generations are spent
/// is retired instead of reused.
pub(crate) struct Arena<T> {
  slots: Vec<Slot<T>>,
  free: Vec<u32>,
}

impl<T> Arena<T> {
  pub(crate) const fn new() -> Self {
    Self {
      slots: Vec::new(),
      free: Vec::new(),
    }
  }

  /// Stores `value` in a slot and returns the slot's generation and index.
  pub(crate) fn allocate(&mut self, value: T) -> (u16, u32) {
    if let Some(index) = self.free.pop() {
      let slot = &mut self.slots[index as usize];

      slot.value = Some(value);

      return (slot.generation, index);
    }

    let index = self.slots.len() as u32;

    self.slots.push(Slot {
      generation: 0,
      value: Some(value),
    });

    (0, index)
  }

  /// The value at `index`, when the slot still holds generation `generation`.
  #[inline(always)]
  pub(crate) fn get(&self, generation: u16, index: u32) -> Option<&T> {
    let slot = self.slots.get(index as usize)?;

    if slot.generation != generation {
      return None;
    }

    slot.value.as_ref()
  }

  /// The value at `index`, mutably; see [`get`](Self::get).
  #[inline(always)]
  pub(crate) fn get_mut(&mut self, generation: u16, index: u32) -> Option<&mut T> {
    let slot = self.slots.get_mut(index as usize)?;

    if slot.generation != generation {
      return None;
    }

    slot.value.as_mut()
  }

  /// Removes and returns the value at `index`, freeing its slot for reuse.
  pub(crate) fn remove(&mut self, generation: u16, index: u32) -> Option<T> {
    let slot = self.slots.get_mut(index as usize)?;

    if slot.generation != generation {
      return None;
    }

    let value = slot.value.take()?;

    slot.generation = slot.generation.wrapping_add(1);

    if slot.generation != 0 {
      self.free.push(index);
    }

    Some(value)
  }
}
