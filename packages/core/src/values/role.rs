//! Semantic roles.

/// The semantic role of a box, beyond a plain grouping container.
///
/// A box with no role carries no semantic meaning. Each platform maps a role to its
/// platform's semantic element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Role {
  /// A clickable button.
  Button,
  /// A navigation region.
  Navigation,
  /// The document's main content region; one per page.
  Main,
  /// An introductory region.
  Header,
  /// A closing region.
  Footer,
  /// A complementary region, tangential to the main content.
  Aside,
  /// A standalone section of content.
  Section,
  /// A self-contained composition.
  Article,
  /// A heading of the given level, `1` (highest) through `6`; out-of-range levels clamp.
  Heading(u8),
  /// A list of items.
  List,
  /// One item of a [`List`](Role::List).
  ListItem,
}
