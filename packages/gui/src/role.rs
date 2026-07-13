/// The semantic role of a box, beyond a plain grouping container.
///
/// A box with no role carries no semantic meaning. Each platform maps a role to its
/// platform's semantic element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Role {
  /// A clickable button.
  Button,
}
