//! The accessible-name computation and the queries over it, for a platform naming its
//! nodes itself: the scene's structure and parts, and what the platform records of
//! each node.

use std::marker::PhantomData;

use super::{AccessProp, NodeId, Part, Platform, Role, Scene};

/// What the accessible-name computation reads of one node from its platform's records.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct NameFacts {
  /// The role the node was created with.
  pub role: Option<Role>,
  /// What the node reports to assistive technology.
  pub access: Vec<AccessProp>,
  /// The name a control's props were created with.
  pub label: Option<String>,
  /// A run's text.
  pub text: Option<String>,
  /// Whether the node is a link: a run or a box with a destination.
  pub link: bool,
  /// Whether the node lays out apart from the text around it: a box, or a run on its
  /// own line.
  pub block: bool,
  /// Whether the node is out of the accessibility tree: hidden, or a closed overlay.
  pub hidden: bool,
  /// The text a text entry holds, or the name of a picker's chosen option.
  pub value: Option<String>,
  /// The hint a text entry shows while empty.
  pub placeholder: Option<String>,
}

impl NameFacts {
  /// Whether the node takes its name from its content, as a link and ARIA's roles
  /// named from contents do.
  fn names_from_content(&self) -> bool {
    self.link
      || self.role.is_some_and(|role| {
        matches!(
          role,
          Role::Button
            | Role::DefaultButton
            | Role::CheckBox
            | Role::RadioButton
            | Role::Switch
            | Role::Link
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::ListBoxOption
            | Role::MenuListOption
            | Role::Tab
            | Role::TreeItem
            | Role::Heading
            | Role::Cell
            | Role::GridCell
            | Role::ColumnHeader
            | Role::RowHeader
            | Role::Row
            | Role::Tooltip
            | Role::DisclosureTriangle
            | Role::Comment
            | Role::DocBackLink
            | Role::DocBiblioRef
            | Role::DocGlossRef
            | Role::DocNoteRef
        )
      })
  }

  /// Whether the node must be named, as a link and ARIA's roles requiring an accessible
  /// name must: a control, an option, a picture, a heading, a dialog.
  fn needs_name(&self) -> bool {
    self.link
      || self.role.is_some_and(|role| {
        matches!(
          role,
          Role::Button
            | Role::DefaultButton
            | Role::CheckBox
            | Role::RadioButton
            | Role::RadioGroup
            | Role::Switch
            | Role::Slider
            | Role::SpinButton
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
            | Role::DateInput
            | Role::DateTimeInput
            | Role::MonthInput
            | Role::TimeInput
            | Role::WeekInput
            | Role::ComboBox
            | Role::EditableComboBox
            | Role::ListBox
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::MenuListOption
            | Role::Link
            | Role::Image
            | Role::Heading
            | Role::Dialog
            | Role::AlertDialog
            | Role::Tree
            | Role::TreeItem
            | Role::TreeGrid
            | Role::Grid
            | Role::Meter
            | Role::ProgressIndicator
            | Role::Tooltip
        )
      })
  }

  /// What the node contributes as a control embedded in content a name is taken from:
  /// an entry's or a picker's value, a range's value text or value.
  fn embedded_value(&self) -> Option<String> {
    if self.value.is_some() {
      return self.value.clone();
    }

    let range = self.role.is_some_and(|role| {
      matches!(
        role,
        Role::Slider | Role::SpinButton | Role::ScrollBar | Role::ProgressIndicator | Role::Meter
      )
    });

    if !range {
      return None;
    }

    self
      .access
      .iter()
      .find_map(|prop| match prop {
        AccessProp::ValueText(text) => Some(text.clone()),
        _ => None,
      })
      .or_else(|| {
        self.access.iter().find_map(|prop| match prop {
          AccessProp::Value(value) => Some(value.to_string()),
          _ => None,
        })
      })
  }

  /// The label the node's accessibility properties carry, where it says anything.
  fn access_label(&self) -> Option<&str> {
    self.access.iter().find_map(|prop| match prop {
      AccessProp::Label(label) if !label.trim().is_empty() => Some(label.as_str()),
      _ => None,
    })
  }

  /// The nodes the node is labelled by, where it names any.
  fn labelled_by(&self) -> Option<&[NodeId]> {
    self.access.iter().find_map(|prop| match prop {
      AccessProp::LabelledBy(nodes) if !nodes.is_empty() => Some(nodes.as_slice()),
      _ => None,
    })
  }
}

impl<P: Platform> Scene<P> {
  /// The name `node` reports to assistive technology, by the accessible-name
  /// computation, each node read through `facts`.
  ///
  /// In its order: the text alternatives of the nodes it is labelled by, else the label
  /// its accessibility properties carry, else the name a control's props were created
  /// with, else the text alternative of its [`Part::Label`], else a text entry's
  /// placeholder, else, where its role or its link takes its name from its content, the
  /// text alternatives of its children.
  ///
  /// Within content, a run's text is its own, a block's is set apart by spaces, a hidden
  /// node contributes nothing unless a node is labelled by it, and an embedded entry,
  /// picker or range contributes its value. Whitespace collapses, and an empty name is
  /// none.
  #[must_use]
  pub fn accessible_name(node: NodeId, facts: impl Fn(NodeId) -> NameFacts) -> Option<String> {
    let mut naming = Naming::<P, _> {
      facts: &facts,
      referencing: false,
      platform: PhantomData,
    };
    let root = facts(node);

    collapsed(&naming.alternative(node, &root, Reach::Root))
  }

  /// Every node in the accessibility tree whose role is `role`, in tree order, each node
  /// read through `facts`; a hidden node's subtree is out of the tree.
  #[must_use]
  pub fn find_all_by_role(role: Role, facts: impl Fn(NodeId) -> NameFacts) -> Vec<NodeId> {
    Self::accessible_nodes(&facts)
      .into_iter()
      .filter(|(_, facts)| facts.role == Some(role))
      .map(|(node, _)| node)
      .collect()
  }

  /// The first node in tree order whose role is `role` and whose accessible name is
  /// `name`, whitespace collapsed, each node read through `facts`.
  #[must_use]
  pub fn find_by_role(
    role: Role,
    name: &str,
    facts: impl Fn(NodeId) -> NameFacts,
  ) -> Option<NodeId> {
    let name = collapsed(name);

    Self::find_all_by_role(role, &facts)
      .into_iter()
      .find(|node| Self::accessible_name(*node, &facts) == name)
  }

  /// Every node in the accessibility tree that must be named and is not, in tree order,
  /// each node read through `facts`: a link, or a node whose role requires a name, as a
  /// control's, an option's, a picture's, a heading's or a dialog's does.
  #[must_use]
  pub fn find_all_unnamed(facts: impl Fn(NodeId) -> NameFacts) -> Vec<NodeId> {
    Self::accessible_nodes(&facts)
      .into_iter()
      .filter(|(node, node_facts)| {
        node_facts.needs_name() && Self::accessible_name(*node, &facts).is_none()
      })
      .map(|(node, _)| node)
      .collect()
  }

  /// The first run in tree order whose own text is `text`, whitespace collapsed on both,
  /// each node read through `facts`.
  #[must_use]
  pub fn find_by_text(text: &str, facts: impl Fn(NodeId) -> NameFacts) -> Option<NodeId> {
    let text = collapsed(text);

    Self::accessible_nodes(&facts)
      .into_iter()
      .find(|(_, facts)| {
        facts
          .text
          .as_deref()
          .is_some_and(|own| collapsed(own) == text)
      })
      .map(|(node, _)| node)
  }

  /// Every node in the accessibility tree with its facts, in tree order.
  fn accessible_nodes(facts: &impl Fn(NodeId) -> NameFacts) -> Vec<(NodeId, NameFacts)> {
    let mut pending = Self::with(|scene| scene.roots().collect::<Vec<_>>());
    let mut nodes = Vec::new();

    pending.reverse();

    while let Some(node) = pending.pop() {
      let node_facts = facts(node);

      if node_facts.hidden {
        continue;
      }

      let children = Self::with(|scene| scene.children(node).collect::<Vec<_>>());

      pending.extend(children.into_iter().rev());
      nodes.push((node, node_facts));
    }

    nodes
  }
}

/// `text` with its whitespace collapsed to single spaces and trimmed; `None` where
/// nothing remains.
fn collapsed(text: &str) -> Option<String> {
  let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

  (!text.is_empty()).then_some(text)
}

/// How an accessible-name computation reached a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
  /// The node being named.
  Root,
  /// A node the named node is labelled by, or its caption.
  Reference,
  /// A node within content a name is taken from.
  Content,
}

/// One accessible-name computation.
struct Naming<'a, P, F> {
  /// Reads a node's facts.
  facts: &'a F,
  /// Whether the computation is within a labelled-by traversal, which follows no other.
  referencing: bool,
  /// The platform whose scene is read.
  platform: PhantomData<P>,
}

impl<P: Platform, F: Fn(NodeId) -> NameFacts> Naming<'_, P, F> {
  /// The text alternative of `node`, whose facts are `facts`, reached by `reach`.
  fn alternative(&mut self, node: NodeId, facts: &NameFacts, reach: Reach) -> String {
    if reach != Reach::Reference && facts.hidden {
      return String::new();
    }

    if !self.referencing
      && let Some(nodes) = facts.labelled_by()
    {
      let name = self.references(nodes);

      if !name.trim().is_empty() {
        return name;
      }
    }

    if reach != Reach::Root
      && let Some(value) = facts.embedded_value()
    {
      return value;
    }

    if let Some(label) = facts.access_label() {
      return label.to_owned();
    }

    if let Some(label) = facts
      .label
      .as_ref()
      .filter(|label| !label.trim().is_empty())
    {
      return label.clone();
    }

    let caption = Scene::<P>::with(|scene| {
      scene
        .children(node)
        .find(|child| scene.part(*child) == Some(Part::Label))
    });

    if let Some(caption) = caption
      && !self.referencing
    {
      let name = self.references(&[caption]);

      if !name.trim().is_empty() {
        return name;
      }
    }

    if let Some(placeholder) = facts
      .placeholder
      .as_ref()
      .filter(|placeholder| !placeholder.trim().is_empty())
    {
      return placeholder.clone();
    }

    if reach != Reach::Root || facts.names_from_content() {
      return self.content(node, facts);
    }

    String::new()
  }

  /// The text alternatives of `nodes`, each reached as a reference, joined by spaces.
  fn references(&mut self, nodes: &[NodeId]) -> String {
    self.referencing = true;

    let name = nodes
      .iter()
      .map(|node| self.alternative(*node, &(self.facts)(*node), Reach::Reference))
      .filter(|name| !name.trim().is_empty())
      .collect::<Vec<_>>()
      .join(" ");

    self.referencing = false;

    name
  }

  /// The text of `node`, whose facts are `facts`, and the text alternatives of its
  /// children, in order, a block's set apart by spaces.
  fn content(&mut self, node: NodeId, facts: &NameFacts) -> String {
    let mut out = facts.text.clone().unwrap_or_default();
    let children = Scene::<P>::with(|scene| scene.children(node).collect::<Vec<_>>());

    for child in children {
      let child_facts = (self.facts)(child);
      let block = child_facts.block;
      let alternative = self.alternative(child, &child_facts, Reach::Content);

      if block {
        out.push(' ');
        out.push_str(&alternative);
        out.push(' ');
      }
      else {
        out.push_str(&alternative);
      }
    }

    out
  }
}
