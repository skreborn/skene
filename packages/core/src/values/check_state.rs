//! A checkable control's reported state.

/// What a checkable control reports: on, off, or the mixed state a control with
/// partly-checked descendants stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum CheckState {
  /// Not checked.
  Off,
  /// Checked.
  On,
  /// Neither: the control stands for a set that is only partly checked.
  Mixed,
}

impl From<bool> for CheckState {
  #[inline(always)]
  fn from(checked: bool) -> Self {
    if checked { Self::On } else { Self::Off }
  }
}
