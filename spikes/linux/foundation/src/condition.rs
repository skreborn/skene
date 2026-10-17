//! The conditions a value varies under: the environment ([`Media`]) and a node's own state or
//! its enclosing control's ([`State`]).

use std::fmt;

/// A condition on the environment. Widths are whole pixels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Media {
  /// The window is at least this wide.
  MinWidth(u32),
  /// The window is at most this wide.
  MaxWidth(u32),
  /// The dark color scheme is in force.
  Dark,
  /// The reader asked for more contrast.
  HighContrast,
  /// The reader asked for reduced motion.
  ReducedMotion,
}

/// A state a node's style varies under: its own (hovered), or, inside a control, the
/// control's (checked, disabled, focus and press).
///
/// A realization keeping states ([`crate::seam::style::Keep::states`]) maps each to its
/// toolkit's own (CSS's pseudo-classes); one that does not reports them where a record
/// varies under them, at the node itself or, for a node built inside a control, at the
/// control ([`State::follows_control`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum State {
  /// The pointer is over the node.
  Hovered,
  /// The node is being pressed.
  Pressed,
  /// Focus is on the node.
  Focused,
  /// Focus is on the node and shows.
  FocusVisible,
  /// Focus is on the node or inside it.
  FocusWithin,
  /// The enclosing control is disabled.
  Disabled,
  /// The enclosing control is on or checked (a chosen item is checked).
  Checked,
  /// The enclosing control is mixed.
  Indeterminate,
  /// The enclosing control's value is invalid.
  Invalid,
  /// The enclosing popup or disclosure is open.
  Open,
}

impl State {
  /// Every state, in the order a realization compiles their combinations.
  pub const ALL: [Self; 10] = [
    Self::Hovered,
    Self::Pressed,
    Self::Focused,
    Self::FocusVisible,
    Self::FocusWithin,
    Self::Disabled,
    Self::Checked,
    Self::Indeterminate,
    Self::Invalid,
    Self::Open,
  ];

  /// Whether the state is always the enclosing control's: checked, mixed, disabled, invalid,
  /// open. A node outside every control is never in one.
  #[must_use]
  pub const fn is_control_state(self) -> bool {
    matches!(
      self,
      Self::Disabled | Self::Checked | Self::Indeterminate | Self::Invalid | Self::Open
    )
  }

  /// Whether a node built inside a control takes the state from the control: the control's
  /// own states, and its focus and press, which a part that takes neither cannot have on its
  /// own. Hovered stays the node's own, as focus and press stay a node's that takes them
  /// itself (an acting box, a control within a control) ([`State::is_interaction`]).
  #[must_use]
  pub const fn follows_control(self) -> bool {
    self.is_control_state() || self.is_interaction()
  }

  /// Whether the state is a focus or a press: a node that takes focus and presses itself has
  /// it on its own, inside a control or not.
  #[must_use]
  pub const fn is_interaction(self) -> bool {
    matches!(self, Self::Focused | Self::FocusVisible | Self::Pressed)
  }
}

/// A set of [`State`]s.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StateSet(pub u16);

impl StateSet {
  /// No state.
  pub const EMPTY: Self = Self(0);

  /// Whether `state` is in the set.
  #[inline(always)]
  #[must_use]
  pub const fn contains(self, state: State) -> bool {
    self.0 & (1 << state as u8) != 0
  }

  /// The set with `state` added.
  #[inline(always)]
  #[must_use]
  pub const fn with(self, state: State) -> Self {
    Self(self.0 | (1 << state as u8))
  }

  /// The set with `state` removed.
  #[inline(always)]
  #[must_use]
  pub const fn without(self, state: State) -> Self {
    Self(self.0 & !(1 << state as u8))
  }

  /// Both sets' states.
  #[inline(always)]
  #[must_use]
  pub const fn union(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }

  /// Whether every state of this set is in `other`.
  #[inline(always)]
  #[must_use]
  pub const fn is_subset(self, other: Self) -> bool {
    self.0 & !other.0 == 0
  }

  /// Whether the set is empty.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(self) -> bool {
    self.0 == 0
  }

  /// How many states the set holds.
  #[inline(always)]
  #[must_use]
  pub const fn len(self) -> u32 {
    self.0.count_ones()
  }

  /// The states in the set, in [`State::ALL`]'s order.
  pub fn iter(self) -> impl Iterator<Item = State> {
    State::ALL
      .into_iter()
      .filter(move |state| self.contains(*state))
  }

  /// Every subset of this set, the smaller first.
  #[must_use]
  pub fn subsets(self) -> Vec<Self> {
    let states: Vec<State> = self.iter().collect();
    let mut subsets: Vec<Self> = (0..1u32 << states.len())
      .map(|bits| {
        states
          .iter()
          .enumerate()
          .filter(|(index, _)| bits & (1 << index) != 0)
          .fold(Self::EMPTY, |set, (_, state)| set.with(*state))
      })
      .collect();

    subsets.sort_by_key(|set| set.len());

    subsets
  }
}

impl fmt::Debug for StateSet {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_set().entries(self.iter()).finish()
  }
}

impl FromIterator<State> for StateSet {
  fn from_iter<I: IntoIterator<Item = State>>(states: I) -> Self {
    states.into_iter().fold(Self::EMPTY, Self::with)
  }
}

/// A condition a property's value varies under.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Condition {
  /// A condition on the environment.
  Media(Media),
  /// A condition on the node's state.
  State(State),
}

impl From<Media> for Condition {
  #[inline(always)]
  fn from(media: Media) -> Self {
    Self::Media(media)
  }
}

impl From<State> for Condition {
  #[inline(always)]
  fn from(state: State) -> Self {
    Self::State(state)
  }
}
