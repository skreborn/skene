/// A stable identifier for a node in the render tree.
///
/// Nodes are allocated by the [`Backend`](crate::Backend); the same id refers to
/// the same realized node (a DOM element, a native layout box, a TUI cell-region,
/// …) for the lifetime of that node. Ids are never reused after a node is
/// destroyed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
  #[must_use]
  #[inline(always)]
  pub const fn from_raw(raw: u64) -> Self {
    NodeId(raw)
  }

  #[must_use]
  #[inline(always)]
  pub const fn raw(self) -> u64 {
    self.0
  }
}

impl std::fmt::Debug for NodeId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "NodeId({})", self.0)
  }
}

/// The semantic kind of an element.
///
/// This is intentionally abstract: it describes the *role* of a node, not how a
/// particular backend realizes it. The DOM backend maps these to HTML tags
/// (`Box` → `div`, `Text` → `span`, `Button` → `button`); a native backend would
/// map them to its own widget primitives, a TUI backend to its own.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ElementKind {
  /// A generic layout container (a styled box).
  Box,
  /// A run of text. Its content is set with [`Backend::set_text`](crate::Backend::set_text).
  Text,
  /// An interactive button.
  Button,
}
