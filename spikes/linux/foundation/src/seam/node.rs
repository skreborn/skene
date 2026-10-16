//! What a node is, and the facts a toolkit fixes when it creates one.

use crate::seam::{access::Role, content::Content};

/// A Skene layout container: a host whose children the foundation's engine lays out where the
/// realization's [`crate::seam::LayoutOwner`] is the engine.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LayoutKind {
  /// `Container`: decoration and sizing around exactly one child.
  Box,
  /// `Row`: children side by side.
  Row,
  /// `Column`: children one above the other.
  Column,
  /// `Grid`: children in tracks.
  Grid,
  /// `Stack`: children over one another in one cell.
  Stack,
  /// A box whose one child scrolls along the axes it names, the toolkit's own scroller (its
  /// physics, scroll bars and keyboard).
  Scroll {
    /// Whether it scrolls horizontally.
    horizontal: bool,
    /// Whether it scrolls vertically.
    vertical: bool,
  },
}

/// A control every pixel platform has with the same meaning, realized as the toolkit's own.
///
/// A control one platform alone has is that platform's, built through the native door
/// ([`crate::seam::door`]) by the facade's `platform::<p>` module.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ControlKind {
  /// An on/off switch.
  Switch,
  /// A checkbox, which may also be mixed.
  Checkbox,
  /// One choice of a group.
  Radio,
  /// A value on a range, dragged.
  Slider,
  /// A one-line text entry.
  TextField,
  /// A multi-line text entry.
  TextEditor,
  /// A command.
  Button,
  /// One choice of a list, shown closed.
  Picker,
  /// How far along a task is.
  Progress,
  /// A number stepped up and down, or typed.
  Stepper,
}

/// What a node is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
  /// A layout container.
  Layout(LayoutKind),
  /// A paragraph of text.
  Text,
  /// A picture.
  Image,
  /// A toolkit control.
  Control(ControlKind),
  /// A theme's token scope: realized as a node only where the realization says so
  /// ([`crate::seam::Realization::SCOPE_NODES`]); elsewhere its children join its parent.
  Scope,
  /// An anchor a reactive subtree mounts under (a derived view, a keyed list): never realized,
  /// so its children join its parent's layout in its place.
  Segment,
  /// A toolkit widget a library handed in through the native door.
  Native,
  /// A box drawn by the app: its canvas rendered to pixels ([`Content::Pixels`]).
  Painted,
}

impl NodeKind {
  /// Every kind with one representative of each layout kind: what
  /// [`crate::seam::testing::contents_gate`] walks.
  pub const ALL: [Self; 19] = [
    Self::Layout(LayoutKind::Box),
    Self::Layout(LayoutKind::Row),
    Self::Layout(LayoutKind::Column),
    Self::Layout(LayoutKind::Grid),
    Self::Layout(LayoutKind::Stack),
    Self::Layout(LayoutKind::Scroll {
      horizontal: false,
      vertical: true,
    }),
    Self::Text,
    Self::Image,
    Self::Control(ControlKind::Switch),
    Self::Control(ControlKind::Checkbox),
    Self::Control(ControlKind::Radio),
    Self::Control(ControlKind::Slider),
    Self::Control(ControlKind::TextField),
    Self::Control(ControlKind::TextEditor),
    Self::Control(ControlKind::Button),
    Self::Control(ControlKind::Picker),
    Self::Control(ControlKind::Progress),
    Self::Control(ControlKind::Stepper),
    Self::Painted,
  ];

  /// The contents a node of this kind takes: binding or controlling any other is refused.
  ///
  /// A native node takes the door's own names ([`Content::Door`]) and nothing else.
  #[must_use]
  pub const fn contents(self) -> &'static [Content] {
    use Content as C;

    match self {
      Self::Layout(LayoutKind::Box) => &[C::Destination],
      Self::Layout(_) | Self::Scope | Self::Segment | Self::Native => &[],
      Self::Text => &[C::Text, C::Runs],
      Self::Image => &[C::Source],
      Self::Painted => &[C::Pixels],
      Self::Control(control) => match control {
        ControlKind::Switch => &[C::On, C::Enabled],
        ControlKind::Checkbox => &[C::On, C::Mixed, C::Caption, C::Enabled],
        ControlKind::Radio => &[C::On, C::Caption, C::Enabled],
        ControlKind::Slider | ControlKind::Stepper => &[C::Range, C::Enabled],
        ControlKind::Progress => &[C::Range],
        ControlKind::TextField => &[
          C::Edit,
          C::Placeholder,
          C::Enabled,
          C::ReadOnly,
          C::Secure,
          C::Keyboard,
        ],
        ControlKind::TextEditor => &[C::Edit, C::Enabled, C::ReadOnly],
        ControlKind::Button => &[C::Caption, C::Prominent, C::Enabled],
        ControlKind::Picker => &[C::Choice, C::Enabled],
      },
    }
  }

  /// Whether a node of this kind takes `content`.
  #[must_use]
  pub fn takes(self, content: Content) -> bool {
    match (self, content) {
      (Self::Native, Content::Door(_)) => true,
      _ => self.contents().contains(&content),
    }
  }
}

/// What a realization is told when it creates a node: what it is, and the facts a toolkit
/// fixes at construction. Changing one means building another node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeSpec {
  /// What the node is.
  pub kind: NodeKind,
  /// What a reader takes it for, fixed at construction as a toolkit fixes a view's role.
  pub role: Role,
  /// Whether the node is a box that acts on activation (an activatable row, a link): it takes
  /// focus in the scene's child order, `Enter` and `Space` activate it, and a click on it
  /// outside the controls it holds activates it.
  ///
  /// Such a box may hold controls, which own their own presses.
  pub acts: bool,
}

impl NodeSpec {
  /// A node of `kind`, its role the kind's default.
  #[must_use]
  pub const fn new(kind: NodeKind) -> Self {
    Self {
      kind,
      role: Role::of(kind),
      acts: false,
    }
  }

  /// This node with `role`.
  #[must_use]
  pub const fn with_role(mut self, role: Role) -> Self {
    self.role = role;

    self
  }

  /// This node, acting on activation, its role an activatable row unless it was given a
  /// link's.
  #[must_use]
  pub const fn acting(mut self) -> Self {
    self.acts = true;

    if !matches!(self.role, Role::Link) {
      self.role = Role::Row;
    }

    self
  }

  /// Whether the node is interactive: a control, an acting box, or a toolkit widget adopted
  /// through the door whose role a reader operates.
  #[must_use]
  pub const fn is_interactive(&self) -> bool {
    self.acts
      || matches!(self.kind, NodeKind::Control(_))
      || (matches!(self.kind, NodeKind::Native) && self.role.is_interactive())
  }
}
