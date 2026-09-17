//! The clipboard actions a platform performs on its own selection.

/// A clipboard action on whatever the platform holds selected: the document's
/// selection or the focused text entry's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum ClipboardAction {
  /// Writes the selection to the clipboard.
  Copy,
  /// Writes the focused entry's selection to the clipboard and removes it.
  Cut,
  /// Inserts the clipboard's text into the focused entry, in place of its selection.
  Paste,
}

/// Whether an action can be offered right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Availability {
  /// Not offered at all: a menu lists no entry for it.
  Absent,
  /// Offered but not applicable to what is selected or focused: listed, disabled.
  Disabled,
  /// Offered and applicable.
  Enabled,
}
