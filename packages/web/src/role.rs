/// The semantic role of a container, beyond a plain grouping container.
///
/// A container with no role (`Option<Role>::None`) carries no semantic role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Role {
  /// A clickable button.
  Button,
}

impl Role {
  /// The HTML tag this role maps to.
  #[inline(always)]
  pub(crate) fn tag(self) -> &'static str {
    match self {
      Role::Button => "button",
    }
  }
}
