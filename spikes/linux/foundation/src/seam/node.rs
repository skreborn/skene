//! What a node is, and the facts a toolkit fixes when it creates one.

use crate::{
  layout::Axis,
  seam::{access::Role, content::Content, control::DragAxes},
};

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

/// What a node is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
  /// A layout container.
  Layout(LayoutKind),
  /// A paragraph of text.
  Text,
  /// A picture.
  Image,
  /// The realization's own editable text view: a text entry's whole box, decorated by its
  /// record, its text inside its padding.
  Editable {
    /// Whether it holds several lines (an editor) or one (a field).
    multiline: bool,
  },
  /// Content above the app, out of its parent's flow, placed against an anchor and shown as
  /// a surface of its own while it is shown: a popup. It lays its one child out as a box does.
  Overlay,
  /// A theme's token scope: realized as a node only where the realization says so
  /// ([`crate::seam::Realization::SCOPE_NODES`]); elsewhere its children join its parent.
  Scope,
  /// An anchor a reactive subtree mounts under (a derived view, a keyed list): never realized,
  /// so its children join its parent's layout in its place.
  Segment,
  /// A toolkit widget a library handed in through the native door.
  Native,
  /// A box drawn by the app: its picture replayed in the scene's order ([`Content::Picture`]).
  Painted,
}

impl NodeKind {
  /// The contents a node of this kind takes: binding or controlling any other is refused.
  ///
  /// A native node takes the door's own names ([`Content::Door`]) and nothing else.
  #[must_use]
  pub const fn contents(self) -> &'static [Content] {
    use Content as C;

    match self {
      Self::Layout(LayoutKind::Box) => &[C::Destination],
      Self::Layout(LayoutKind::Scroll { .. }) => &[C::ScrollOffset],
      Self::Layout(_) | Self::Scope | Self::Segment | Self::Native => &[],
      Self::Text => &[C::Text, C::Runs, C::Reserve],
      Self::Image => &[C::Source],
      Self::Painted => &[C::Picture],
      Self::Overlay => &[C::Placement],
      Self::Editable { multiline: false } => {
        &[C::Edit, C::Placeholder, C::ReadOnly, C::Secure, C::Keyboard]
      }
      Self::Editable { multiline: true } => &[C::Edit, C::Placeholder, C::ReadOnly],
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

/// How a node takes events.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Interaction {
  /// Layout and decoration: an event at it climbs to the nearest node that takes it.
  #[default]
  None,
  /// A box that acts on activation: an activatable row or a link. It takes focus in the
  /// scene's child order, `Enter` and `Space` activate it, and a click on it outside the
  /// controls it holds activates it; it may hold controls, which own their own presses.
  Acting,
  /// A control: the owner of every press inside it, focused in the scene's order where its
  /// facts say it takes focus, the pointer kept with it from a press where it drags.
  Control {
    /// The axes a press drags it along.
    drag: Option<DragAxes>,
  },
  /// A control's caption placed beside it: a press on it focuses that control.
  Answering,
  /// A scroll bar's track along `axis`, its one child the thumb: laid along the scroll box's
  /// edge, out of the scrolling content's flow and unscrolled, shown while the content
  /// overflows along `axis`. A press on it scrolls the box and moves no focus.
  ScrollBar {
    /// The axis the bar scrolls along.
    axis: Axis,
  },
}

/// What a realization is told when it creates a node: what it is, and the facts a toolkit
/// fixes at construction. Changing one means building another node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeSpec {
  /// What the node is.
  pub kind: NodeKind,
  /// What a reader takes it for, fixed at construction as a toolkit fixes a view's role.
  pub role: Role,
  /// How it takes events.
  pub interaction: Interaction,
}

impl NodeSpec {
  /// A node of `kind`, its role the kind's default, taking no event of its own.
  #[must_use]
  pub const fn new(kind: NodeKind) -> Self {
    Self {
      kind,
      role: Role::of(kind),
      interaction: Interaction::None,
    }
  }

  /// This node with `role`.
  #[must_use]
  pub const fn with_role(mut self, role: Role) -> Self {
    self.role = role;

    self
  }

  /// This node, taking events as `interaction` says.
  #[must_use]
  pub const fn with_interaction(mut self, interaction: Interaction) -> Self {
    self.interaction = interaction;

    self
  }

  /// This node, acting on activation, its role an activatable row unless it was given a
  /// link's.
  #[must_use]
  pub const fn acting(mut self) -> Self {
    self.interaction = Interaction::Acting;

    if !matches!(self.role, Role::Link) {
      self.role = Role::Row;
    }

    self
  }

  /// Whether the node is a control.
  #[must_use]
  pub const fn is_control(&self) -> bool {
    matches!(self.interaction, Interaction::Control { .. })
  }

  /// Whether a press on the node is the node's own: a control, an acting box, or a toolkit
  /// widget adopted through the door whose role a reader operates.
  #[must_use]
  pub const fn is_interactive(&self) -> bool {
    matches!(
      self.interaction,
      Interaction::Acting | Interaction::Control { .. }
    ) || (matches!(self.kind, NodeKind::Native) && self.role.is_interactive())
  }
}
