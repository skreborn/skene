//! Semantic roles.

/// The semantic role of a box, beyond a plain grouping container.
///
/// A box with no role carries no semantic meaning. Each platform maps a role to
/// whatever its accessibility layer understands—a semantic element, an ARIA role, a
/// native accessibility node.
///
/// The landmark roles describe a region of a document; the widget roles describe a
/// control a view draws itself, and pair with the [`AccessProp`](crate::AccessProp) state
/// that control reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Role {
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
  /// A control's caption: activating it activates the control it wraps.
  Label,
  /// A window of its own within the page, named by its label: a popup holding a form.
  Dialog,
  /// A picture, named by its label.
  Image,
  /// A region whose changes are announced when convenient.
  Status,
  /// A region whose changes are announced at once.
  Alert,
  /// A control that activates on press.
  Button,
  /// A control toggling between checked and unchecked.
  Checkbox,
  /// One mutually-exclusive choice within a [`RadioGroup`](Role::RadioGroup).
  Radio,
  /// The group a set of [`Radio`](Role::Radio) choices belongs to.
  RadioGroup,
  /// A control picking a value from a continuous range.
  Slider,
  /// A list of choices to select from.
  Listbox,
  /// One choice within a [`Listbox`](Role::Listbox).
  ListboxOption,
  /// A control pairing an input with a list of choices.
  Combobox,
  /// A list of commands.
  Menu,
  /// One command within a [`Menu`](Role::Menu).
  MenuItem,
}

impl Role {
  /// Whether this role is spelled on a generic element rather than realized as an
  /// element of its own: every widget, and the dialog, image, status and alert roles.
  #[inline(always)]
  #[must_use]
  pub const fn is_spelled(self) -> bool {
    self.is_widget()
      || matches!(
        self,
        Self::Dialog | Self::Image | Self::Status | Self::Alert
      )
  }

  /// Whether this role describes a control a view draws itself, rather than a region of
  /// a document.
  ///
  /// A widget reports state through [`AccessProp`](crate::AccessProp); a landmark does not.
  #[inline(always)]
  #[must_use]
  pub const fn is_widget(self) -> bool {
    matches!(
      self,
      Self::Button
        | Self::Checkbox
        | Self::Radio
        | Self::RadioGroup
        | Self::Slider
        | Self::Listbox
        | Self::ListboxOption
        | Self::Combobox
        | Self::Menu
        | Self::MenuItem
    )
  }
}
