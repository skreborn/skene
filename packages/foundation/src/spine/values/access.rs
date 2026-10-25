use super::{Axis, CheckState};
use crate::spine::{NodeId, Shortcut};

/// One accessibility property a view reports, for a reactive update.
///
/// Semantics, not style: a platform maps these to whatever its accessibility layer
/// speaks—ARIA attributes on the web, a platform accessibility node elsewhere—so the
/// vocabulary names the concept and never the platform's spelling of it. Pairs with a
/// [`Role`](super::Role), which says what the view *is* while these say what it currently
/// reports.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub enum AccessProp {
  /// Whether a toggle is checked.
  Checked(CheckState),
  /// The value a range control currently holds.
  Value(f32),
  /// The lowest value that control accepts.
  ValueMin(f32),
  /// The highest value that control accepts.
  ValueMax(f32),
  /// The step that control's value moves by; non-positive for a continuous range.
  ValueStep(f32),
  /// A human-readable reading of the value, when the number alone does not say enough.
  ValueText(String),
  /// Whether the control's associated content is revealed.
  Expanded(bool),
  /// Whether a choice within a set is the chosen one.
  Selected(bool),
  /// Whether the control is present but not operable.
  Disabled(bool),
  /// The control's accessible name, when nothing visible supplies one.
  Name(String),
  /// Whether a value must be supplied before the form the control is in submits.
  Required(bool),
  /// Whether the control holds an invalid value.
  Invalid(bool),
  /// Whether the control's value is shown but not editable.
  ReadOnly(bool),
  /// The kind of popup the control opens.
  HasPopup(PopupKind),
  /// The nodes the control operates, in order: its popup's list, the region it expands.
  Controls(Vec<NodeId>),
  /// The nodes whose text names the control, in order, where the name is not the
  /// control's own content.
  LabelledBy(Vec<NodeId>),
  /// The nodes whose text describes the control, in order.
  DescribedBy(Vec<NodeId>),
  /// A plain-text description: the accessible description where nothing else supplies
  /// one, and the native tooltip where the platform shows one without script.
  Description(String),
  /// The axis a range control runs along.
  Orientation(Axis),
  /// The level in a hierarchy: a heading's, `1` the highest, a tree item's or a nested
  /// list item's.
  Level(usize),
  /// The keyboard shortcut that activates the control from anywhere in the window.
  Shortcut(Shortcut),
}

impl AccessProp {
  /// `access` with the relations of one kind folded into the first of them, each node
  /// once, in the order the reports name them; every other property as reported.
  pub fn fold(access: impl IntoIterator<Item = Self>) -> Vec<Self> {
    let mut folded: Vec<Self> = Vec::new();

    for prop in access {
      if let Self::Controls(nodes) | Self::LabelledBy(nodes) | Self::DescribedBy(nodes) = &prop
        && let Some(Self::Controls(held) | Self::LabelledBy(held) | Self::DescribedBy(held)) =
          folded
            .iter_mut()
            .find(|held| std::mem::discriminant(*held) == std::mem::discriminant(&prop))
      {
        for node in nodes {
          if !held.contains(node) {
            held.push(*node);
          }
        }

        continue;
      }

      folded.push(prop);
    }

    folded
  }

  /// The level `access` reports, if any.
  #[must_use]
  pub fn level_of(access: &[Self]) -> Option<usize> {
    access.iter().find_map(|prop| match prop {
      Self::Level(level) => Some(*level),
      _ => None,
    })
  }

  /// The nodes the property refers to: a relation's, and none for every other.
  #[must_use]
  pub fn referenced(&self) -> &[NodeId] {
    match self {
      Self::Controls(nodes) | Self::LabelledBy(nodes) | Self::DescribedBy(nodes) => nodes,
      _ => &[],
    }
  }
}

/// What a control's popup is; a list of options unless told otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub enum PopupKind {
  /// A list of options.
  #[default]
  Listbox,
  /// A menu of commands.
  Menu,
  /// A dialog.
  Dialog,
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Relations of one kind fold into the first of them, each node once and in order,
  /// while every other property stays as reported.
  #[test]
  fn relations_of_one_kind_fold_into_one_list() {
    let [a, b, c] = [1, 2, 3].map(|index| NodeId::new(index, 0));
    let folded = AccessProp::fold([
      AccessProp::DescribedBy(vec![a]),
      AccessProp::Required(true),
      AccessProp::LabelledBy(vec![b]),
      AccessProp::DescribedBy(vec![c, a]),
      AccessProp::Controls(vec![]),
      AccessProp::Description("hint".to_owned()),
    ]);

    assert_eq!(
      folded,
      [
        AccessProp::DescribedBy(vec![a, c]),
        AccessProp::Required(true),
        AccessProp::LabelledBy(vec![b]),
        AccessProp::Controls(vec![]),
        AccessProp::Description("hint".to_owned()),
      ]
    );
    assert_eq!(folded[0].referenced(), [a, c]);
    assert_eq!(folded[1].referenced(), []);
  }
}
