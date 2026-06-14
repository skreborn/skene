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
/// (`Box` → `div`, `Text` → `span`); a native backend would map them to its own
/// widget primitives, a TUI backend to its own.
///
/// There is deliberately no `Button` kind: a button is just a composition of
/// these primitives (see `skene_core::Button`), not a special element.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ElementKind {
  /// A generic layout container (a styled box).
  Box,
  /// A run of text. Its content is set with [`Backend::set_text`](crate::Backend::set_text).
  Text,
}

/// The accessibility role of a node: its semantic purpose, independent of how it
/// is built. Set with [`Backend::set_role`](crate::Backend::set_role).
///
/// This is backend-agnostic. The DOM backend maps it to an ARIA `role`
/// attribute; a native backend would map it into its accessibility tree, a TUI
/// backend into screen-reader output.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Role {
  Button,
  Link,
  Checkbox,
}
