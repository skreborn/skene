//! Accessibility: a Skene role as GTK's accessible role, fixed at construction, and the
//! facts views state mapped onto GTK's accessible properties, states and relations (an invalid
//! value also takes the theme's `error` style class, which [`State::Invalid`] styles under).
//! A drawn control's actions and a painted node's parts are not stated on GTK 4.14, which has
//! no public API for an accessible action of a widget's own or an accessible object without a
//! widget.
//!
//! [`State::Invalid`]: skene_foundation::State::Invalid

use gtk::prelude::*;
use skene_foundation::{
  NodeId,
  seam::access::{AccessFacts, Checked, Role},
};

use crate::Node;

/// GTK's accessible role for `role`.
pub(crate) const fn accessible_role(role: Role) -> gtk::AccessibleRole {
  use gtk::AccessibleRole as A;

  match role {
    Role::Generic => A::Generic,
    Role::Presentation => A::Presentation,
    Role::Group => A::Group,
    Role::Region => A::Region,
    Role::Text => A::Label,
    Role::Heading(_) => A::Heading,
    Role::Image => A::Img,
    Role::Button => A::Button,
    Role::Link => A::Link,
    Role::Switch => A::Switch,
    Role::Checkbox => A::Checkbox,
    Role::Radio => A::Radio,
    Role::RadioGroup => A::RadioGroup,
    Role::Slider => A::Slider,
    Role::SpinButton => A::SpinButton,
    Role::TextField | Role::TextEditor => A::TextBox,
    Role::ComboBox => A::ComboBox,
    Role::ListBox => A::ListBox,
    Role::Option => A::Option,
    Role::Menu => A::Menu,
    Role::MenuItem => A::MenuItem,
    Role::ProgressBar => A::ProgressBar,
    Role::Dialog => A::Dialog,
    Role::Status => A::Status,
    Role::Alert => A::Alert,
    Role::Tooltip => A::Tooltip,
    Role::ScrollArea => A::Group,
  }
}

/// Skene's role for `widget`'s GTK accessible role: a role GTK has and Skene does not is
/// [`Role::Generic`].
pub(crate) fn role_of(widget: &gtk::Widget) -> Role {
  use gtk::AccessibleRole as A;

  match widget.accessible_role() {
    A::Button | A::ToggleButton => Role::Button,
    A::Switch => Role::Switch,
    A::Checkbox => Role::Checkbox,
    A::Radio => Role::Radio,
    A::Slider => Role::Slider,
    A::SpinButton => Role::SpinButton,
    A::TextBox | A::SearchBox => Role::TextField,
    A::ComboBox => Role::ComboBox,
    A::ListBox | A::List => Role::ListBox,
    A::ListItem | A::Option => Role::Option,
    A::Menu => Role::Menu,
    A::MenuItem | A::MenuItemCheckbox | A::MenuItemRadio => Role::MenuItem,
    A::Label => Role::Text,
    A::Heading => Role::Heading(0),
    A::Img => Role::Image,
    A::Link => Role::Link,
    A::ProgressBar => Role::ProgressBar,
    A::Region => Role::Region,
    A::Group => Role::Group,
    _ => Role::Generic,
  }
}

/// Writes what changed from `previous` to `next` on `node`'s accessible, resetting what `next`
/// no longer states, `related` finding the nodes a relation names. A name the view gives wins
/// over the caption the toolkit's own control is labelled by (a `GtkButton`'s label).
pub(crate) fn apply_access(
  node: &Node,
  previous: &AccessFacts,
  next: &AccessFacts,
  related: &dyn Fn(NodeId) -> Option<Node>,
) {
  use gtk::{
    AccessibleProperty as P, AccessibleRelation as L, AccessibleState as S,
    accessible::{Property, Relation, State},
  };

  let widget = &node.widget;
  let widgets = |ids: &[NodeId]| -> Vec<gtk::Accessible> {
    ids
      .iter()
      .filter_map(|id| related(*id))
      .map(|node| node.widget.upcast())
      .collect()
  };

  if previous.name != next.name {
    match &next.name {
      Some(name) => {
        widget.update_property(&[Property::Label(name)]);

        if next.labelled_by.is_empty() {
          widget.reset_relation(L::LabelledBy);
        }
      }
      None => widget.reset_property(P::Label),
    }
  }

  if previous.description != next.description {
    match &next.description {
      Some(description) => widget.update_property(&[Property::Description(description)]),
      None => widget.reset_property(P::Description),
    }
  }

  for (before, after, relation) in [
    (&previous.labelled_by, &next.labelled_by, L::LabelledBy),
    (&previous.described_by, &next.described_by, L::DescribedBy),
  ] {
    if before != after {
      let targets = widgets(after);
      let targets: Vec<&gtk::Accessible> = targets.iter().collect();

      match (targets.is_empty(), relation) {
        (true, _) => widget.reset_relation(relation),
        (false, L::LabelledBy) => widget.update_relation(&[Relation::LabelledBy(&targets)]),
        (false, _) => widget.update_relation(&[Relation::DescribedBy(&targets)]),
      }
    }
  }

  if previous.required != next.required {
    widget.update_property(&[Property::Required(next.required)]);
  }

  if previous.read_only != next.read_only {
    widget.update_property(&[Property::ReadOnly(next.read_only)]);
  }

  if previous.invalid != next.invalid {
    if next.invalid {
      widget.update_state(&[State::Invalid(gtk::AccessibleInvalidState::True)]);
      widget.add_css_class("error");
    } else {
      widget.reset_state(S::Invalid);
      widget.remove_css_class("error");
    }
  }

  if previous.hidden != next.hidden {
    widget.update_state(&[State::Hidden(next.hidden)]);
  }

  if previous.checked != next.checked {
    match next.checked {
      Some(checked) => widget.update_state(&[State::Checked(match checked {
        Checked::Off => gtk::AccessibleTristate::False,
        Checked::On => gtk::AccessibleTristate::True,
        Checked::Mixed => gtk::AccessibleTristate::Mixed,
      })]),
      None => widget.reset_state(S::Checked),
    }
  }

  if previous.expanded != next.expanded {
    match next.expanded {
      Some(expanded) => widget.update_state(&[State::Expanded(Some(expanded))]),
      None => widget.reset_state(S::Expanded),
    }
  }

  if previous.selected != next.selected {
    match next.selected {
      Some(selected) => widget.update_state(&[State::Selected(Some(selected))]),
      None => widget.reset_state(S::Selected),
    }
  }

  if previous.value != next.value {
    match &next.value {
      Some(value) => {
        widget.update_property(&[
          Property::ValueMin(value.minimum),
          Property::ValueMax(value.maximum),
          Property::ValueNow(value.now),
        ]);

        match &value.text {
          Some(text) => widget.update_property(&[Property::ValueText(text)]),
          None => widget.reset_property(P::ValueText),
        }
      }
      None => {
        for property in [P::ValueMin, P::ValueMax, P::ValueNow, P::ValueText] {
          widget.reset_property(property);
        }
      }
    }
  }

  if previous.focusable != next.focusable {
    widget.set_focusable(next.focusable);
  }
}
