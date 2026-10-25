//! The accessible-name computation and the queries over it, for a platform naming its
//! nodes itself: the scene's structure and parts, and what the platform records of
//! each node.

use std::{fmt::Write as _, marker::PhantomData};

use accesskit::{
  Action,
  HasPopup,
  Invalid,
  Orientation,
  TextDirection,
  TextPosition,
  Toggled,
  TreeInfo,
  TreeUpdate,
};

use super::{
  AccessProp,
  AccessRun,
  AccessText,
  AccessTextPosition,
  Axis,
  BoxFacts,
  CheckState,
  NativeElement,
  NodeId,
  Part,
  PopupKind,
  Realization,
  Role,
  RunFacts,
  RunPlacement,
  Scene,
};

/// What the accessible-name computation reads of one node from its platform's records.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct NameFacts {
  /// The role the node carries: the one it was created with, else its native element's,
  /// its overlay's or a link's.
  pub role: Option<Role>,
  /// What the node reports to assistive technology.
  pub access: Vec<AccessProp>,
  /// The name a control was created with.
  pub label: Option<String>,
  /// A run's text.
  pub text: Option<String>,
  /// Whether the node is a link: a run or a box created as one.
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
  /// A drawn text entry's laid lines and selection; `None` where its platform's own entry
  /// reports them.
  pub text_runs: Option<AccessText>,
}

impl NameFacts {
  /// The facts of a run holding `text`, created from `facts`.
  pub fn of_run(facts: &RunFacts, text: String) -> Self {
    let link = facts.link;

    Self {
      role: link.then_some(Role::Link),
      text: Some(text),
      link,
      block: facts.placement == RunPlacement::Line,
      ..Self::default()
    }
  }

  /// The facts of a box, a control or an overlay created stating `role` from the box
  /// `facts`, reporting `access`.
  pub fn of_box(role: Option<Role>, facts: &BoxFacts, access: Vec<AccessProp>) -> Self {
    let link = facts.link;

    Self {
      role: role.or(link.then_some(Role::Link)),
      access,
      link,
      block: true,
      ..Self::default()
    }
  }

  /// These facts as an overlay's carrying `role`, out of the tree while it is not
  /// `open`.
  pub fn with_overlay(self, role: Option<Role>, open: bool) -> Self {
    Self {
      role,
      hidden: self.hidden || !open,
      ..self
    }
  }

  /// These facts as a control's standing for the native element `native`, named
  /// `label`, hinting `placeholder` while empty and holding `value`: an entry's text or
  /// a picker's chosen option's name.
  pub fn with_control(
    self,
    native: Option<NativeElement>,
    label: Option<String>,
    placeholder: Option<String>,
    value: Option<String>,
  ) -> Self {
    Self {
      role: self.role.or(native.map(NativeElement::role)),
      label,
      placeholder,
      value,
      ..self
    }
  }

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
      AccessProp::Name(label) if !label.trim().is_empty() => Some(label.as_str()),
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

impl<P: Realization> Scene<P> {
  /// The name `node` reports to assistive technology, by the accessible-name
  /// computation, each node read through `facts`.
  ///
  /// In its order: the text alternatives of the nodes it is labelled by, else the label
  /// its accessibility properties carry, else the name a control was created with,
  /// else the text alternative of its [`Part::Label`], else a text entry's
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

  /// Every node in the accessibility tree with its facts, in tree order: a tree's root,
  /// then its children, and after a segment standing in no parent, its content.
  fn accessible_nodes(facts: &impl Fn(NodeId) -> NameFacts) -> Vec<(NodeId, NameFacts)> {
    let mut pending = Self::with(|scene| scene.roots().collect::<Vec<_>>());
    let mut nodes = Vec::new();

    pending.reverse();

    while let Some(node) = pending.pop() {
      let node_facts = facts(node);

      if node_facts.hidden {
        continue;
      }

      let beneath = Self::with(|scene| {
        let mut beneath: Vec<_> = scene.children(node).collect();

        if scene.parent(node).is_none() {
          beneath.extend(scene.segment_nodes(node));
        }

        beneath
      });

      pending.extend(beneath.into_iter().rev());
      nodes.push((node, node_facts));
    }

    nodes
  }
}

impl<P: Realization> Scene<P> {
  /// The accessibility tree the scene and `facts` make, as AccessKit models it, for a
  /// platform exposing it or a test asserting it.
  ///
  /// A window holds each tree's root; every node outside what is hidden carries its role
  /// (a run's a label holding its text, a box of no role a generic container), its
  /// accessible name, the states and relations it reports, a text entry's value and
  /// placeholder, a drawn entry's runs, selection and the editing actions its state allows
  /// ([`NameFacts::text_runs`]), and its children in tree order, the runs first; focus is on
  /// the node holding it, else on the window.
  #[must_use]
  pub fn access_tree(facts: impl Fn(NodeId) -> NameFacts) -> TreeUpdate {
    let window = accesskit::NodeId(0);
    let roots = Self::with(|scene| scene.roots().collect::<Vec<_>>());
    let mut nodes = Vec::new();
    let children: Vec<_> = roots
      .into_iter()
      .filter_map(|root| Self::access_node(root, &facts, &mut nodes))
      .collect();
    let mut root = accesskit::Node::new(Role::Window);

    root.set_children(children);
    nodes.push((window, root));

    let focus = Self::with(|scene| scene.focused())
      .map(NodeId::access_id)
      .filter(|focused| nodes.iter().any(|(id, _)| id == focused))
      .unwrap_or(window);

    TreeUpdate {
      nodes,
      tree: Some(TreeInfo::new(window)),
      tree_id: accesskit::TreeId::ROOT,
      focus,
    }
  }

  /// [`access_tree`](Self::access_tree) as indented text, one node a line: its role, its
  /// name quoted, its value after `=`, its states in brackets; a generic container naming
  /// nothing is left out, its children standing in its place, and a text entry's runs are
  /// left out, its value standing for them.
  #[must_use]
  pub fn access_outline(facts: impl Fn(NodeId) -> NameFacts) -> String {
    let tree = Self::access_tree(facts);
    let mut out = String::new();

    if let Some(info) = &tree.tree {
      outline_into(&tree, info.root, 0, &mut out);
    }

    out
  }

  /// Adds the AccessKit node of `node` and its accessible descendants to `nodes`,
  /// returning its id; `None` where it is hidden.
  fn access_node(
    node: NodeId,
    facts: &impl Fn(NodeId) -> NameFacts,
    nodes: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
  ) -> Option<accesskit::NodeId> {
    let own = facts(node);

    if own.hidden {
      return None;
    }

    let (mut access, mut children) =
      own.access_node(node, Self::accessible_name(node, facts), nodes);
    let beneath = Self::with(|scene| {
      let mut beneath: Vec<_> = scene.children(node).collect();

      if scene.parent(node).is_none() {
        beneath.extend(scene.segment_nodes(node));
      }

      beneath
    });

    children.extend(
      beneath
        .into_iter()
        .filter_map(|child| Self::access_node(child, facts, nodes)),
    );

    access.set_children(children);

    let id = node.access_id();

    nodes.push((id, access));

    Some(id)
  }
}

impl NameFacts {
  /// The AccessKit node these facts make of `node`, named `name`, with the ids of the
  /// children they give it: its role (a run's a label holding its text, a box of no role
  /// a generic container), its value and placeholder, what it reports, and a drawn
  /// entry's runs, added to `nodes`, with its selection and the editing actions its state
  /// allows.
  pub(in crate::spine) fn access_node(
    &self,
    node: NodeId,
    name: Option<String>,
    nodes: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
  ) -> (accesskit::Node, Vec<accesskit::NodeId>) {
    let role = self.role.unwrap_or(if self.text.is_some() {
      Role::Label
    }
    else {
      Role::GenericContainer
    });
    let mut access = accesskit::Node::new(role);

    if let Some(name) = name {
      access.set_label(name);
    }

    if let Some(text) = self.value.clone().or(self.text.clone()) {
      access.set_value(text);
    }

    if let Some(placeholder) = &self.placeholder {
      access.set_placeholder(placeholder.clone());
    }

    for prop in &self.access {
      prop.apply(&mut access);
    }

    let children = self
      .text_runs
      .as_ref()
      .map_or_else(Vec::new, |text| text.report(node, &mut access, nodes));

    (access, children)
  }
}

impl AccessText {
  /// Reports this text as the text of `entry`, the AccessKit node of the node `node`:
  /// adds its runs to `nodes`, one a line, and gives `entry` the selection and the
  /// actions it takes, returning the runs' ids; a disabled entry takes none, and a
  /// read-only one only a selection.
  fn report(
    &self,
    node: NodeId,
    entry: &mut accesskit::Node,
    nodes: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
  ) -> Vec<accesskit::NodeId> {
    if !entry.is_disabled() {
      entry.add_action(Action::SetTextSelection);

      if !entry.is_read_only() {
        entry.add_action(Action::SetValue);
        entry.add_action(Action::ReplaceSelectedText);
      }
    }

    let runs: Vec<_> = (0..=u16::MAX)
      .zip(&self.runs)
      .map(|(line, run)| {
        let id = node.access_run_id(line);

        nodes.push((id, run.access_node()));

        id
      })
      .collect();
    let position = |at: AccessTextPosition| TextPosition {
      node: node.access_run_id(at.line),
      character_index: at.character,
    };

    if !runs.is_empty() {
      entry.set_text_selection(accesskit::TextSelection {
        anchor: position(self.anchor),
        focus: position(self.focus),
      });
    }

    runs
  }
}

impl AccessRun {
  /// The AccessKit node of this run.
  fn access_node(&self) -> accesskit::Node {
    let mut run = accesskit::Node::new(Role::TextRun);
    let bounds = self.bounds;

    run.set_value(self.text.clone());
    run.set_character_lengths(self.character_lengths.clone());
    run.set_character_positions(self.character_positions.clone());
    run.set_character_widths(self.character_widths.clone());
    run.set_word_starts(self.word_starts.clone());
    run.set_text_direction(if self.right_to_left {
      TextDirection::RightToLeft
    }
    else {
      TextDirection::LeftToRight
    });
    run.set_bounds(accesskit::Rect {
      x0: f64::from(bounds.origin.x),
      y0: f64::from(bounds.origin.y),
      x1: f64::from(bounds.origin.x + bounds.size.width),
      y1: f64::from(bounds.origin.y + bounds.size.height),
    });

    run
  }
}

impl AccessProp {
  /// Writes what this property reports into the AccessKit node `node`; a label is left
  /// to the accessible-name computation.
  fn apply(&self, node: &mut accesskit::Node) {
    match self {
      Self::Checked(toggle) => node.set_toggled(match toggle {
        CheckState::On => Toggled::True,
        CheckState::Off => Toggled::False,
        CheckState::Mixed => Toggled::Mixed,
      }),
      Self::Value(value) => node.set_numeric_value(f64::from(*value)),
      Self::ValueMin(value) => node.set_min_numeric_value(f64::from(*value)),
      Self::ValueMax(value) => node.set_max_numeric_value(f64::from(*value)),
      Self::ValueStep(value) => node.set_numeric_value_step(f64::from(*value)),
      Self::ValueText(text) => node.set_value(text.clone()),
      Self::Expanded(expanded) => node.set_expanded(*expanded),
      Self::Selected(selected) => node.set_selected(*selected),
      Self::Disabled(true) => node.set_disabled(),
      Self::Required(true) => node.set_required(),
      Self::Invalid(true) => node.set_invalid(Invalid::True),
      Self::ReadOnly(true) => node.set_read_only(),
      Self::HasPopup(kind) => node.set_has_popup(match kind {
        PopupKind::Listbox => HasPopup::Listbox,
        PopupKind::Menu => HasPopup::Menu,
        PopupKind::Dialog => HasPopup::Dialog,
      }),
      Self::Controls(nodes) => {
        node.set_controls(
          nodes
            .iter()
            .copied()
            .map(NodeId::access_id)
            .collect::<Vec<_>>(),
        );
      }
      Self::LabelledBy(nodes) => {
        node.set_labelled_by(
          nodes
            .iter()
            .copied()
            .map(NodeId::access_id)
            .collect::<Vec<_>>(),
        );
      }
      Self::DescribedBy(nodes) => {
        node.set_described_by(
          nodes
            .iter()
            .copied()
            .map(NodeId::access_id)
            .collect::<Vec<_>>(),
        );
      }
      Self::Description(text) => node.set_description(text.clone()),
      Self::Orientation(axis) => node.set_orientation(match axis {
        Axis::Horizontal => Orientation::Horizontal,
        Axis::Vertical => Orientation::Vertical,
      }),
      Self::Level(level) => node.set_level(*level),
      Self::Shortcut(shortcut) => node.set_keyboard_shortcut(shortcut.reported()),
      Self::Disabled(false)
      | Self::Required(false)
      | Self::Invalid(false)
      | Self::ReadOnly(false)
      | Self::Name(_) => {}
    }
  }
}

/// Writes the node `id` of `tree` and its descendants into `out` at `depth`.
fn outline_into(tree: &TreeUpdate, id: accesskit::NodeId, depth: usize, out: &mut String) {
  let Some((_, node)) = tree.nodes.iter().find(|(held, _)| *held == id)
  else {
    return;
  };

  if node.role() == Role::TextRun {
    return;
  }

  let skipped = node.role() == Role::GenericContainer && node.label().is_none();

  if !skipped {
    let _ = write!(out, "{}{:?}", "  ".repeat(depth), node.role());

    if let Some(label) = node.label() {
      let _ = write!(out, " \"{label}\"");
    }

    if let Some(value) = node.value() {
      let _ = write!(out, " = \"{value}\"");
    }

    let states = outline_states(tree, id, node);

    if !states.is_empty() {
      let _ = write!(out, " [{}]", states.join(", "));
    }

    out.push('\n');
  }

  for child in node.children() {
    outline_into(tree, *child, if skipped { depth } else { depth + 1 }, out);
  }
}

/// The states of `node`, the node `id` of `tree`, as an outline names them.
fn outline_states(tree: &TreeUpdate, id: accesskit::NodeId, node: &accesskit::Node) -> Vec<String> {
  let mut states = Vec::new();

  if tree.focus == id && id != accesskit::NodeId(0) {
    states.push("focused".to_owned());
  }

  match node.toggled() {
    Some(Toggled::True) => states.push("checked".to_owned()),
    Some(Toggled::False) => states.push("unchecked".to_owned()),
    Some(Toggled::Mixed) => states.push("mixed".to_owned()),
    None => {}
  }

  if let Some(expanded) = node.is_expanded() {
    states.push(if expanded { "expanded" } else { "collapsed" }.to_owned());
  }

  if node.is_selected() == Some(true) {
    states.push("selected".to_owned());
  }

  if node.is_disabled() {
    states.push("disabled".to_owned());
  }

  if node.is_required() {
    states.push("required".to_owned());
  }

  if node.invalid().is_some() {
    states.push("invalid".to_owned());
  }

  if node.is_read_only() {
    states.push("read-only".to_owned());
  }

  if let Some(value) = node.numeric_value() {
    states.push(format!("value {value}"));
  }

  if let Some(level) = node.level() {
    states.push(format!("level {level}"));
  }

  if let Some(shortcut) = node.keyboard_shortcut() {
    states.push(format!("shortcut {shortcut}"));
  }

  states
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

impl<P: Realization, F: Fn(NodeId) -> NameFacts> Naming<'_, P, F> {
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
/// Declares a platform's accessibility queries: what a reader finds in the scene of
/// `$platform` once the turn ends, the accessible-name computation reading each node's
/// facts through `$facts`.
///
/// For a platform's test seams, beside its `trigger_` events and its renders:
/// `accessible_name`, `access_tree`, `access_outline`, `find_by_role`,
/// `find_all_by_role`, `find_all_unnamed` and `find_by_text`.
#[macro_export]
macro_rules! access_queries {
  ($platform:ty, $facts:expr) => {
    /// The name `node` reports to assistive technology, by the accessible-name
    /// computation ([`Scene::accessible_name`](skene_foundation::Scene::accessible_name)).
    #[must_use]
    pub fn accessible_name(node: $crate::NodeId) -> ::std::option::Option<::std::string::String> {
      $crate::end_turn();

      $crate::Scene::<$platform>::accessible_name(node, $facts)
    }

    /// The accessibility tree, as AccessKit models it
    /// ([`Scene::access_tree`](skene_foundation::Scene::access_tree)).
    #[must_use]
    pub fn access_tree() -> $crate::TreeUpdate {
      $crate::end_turn();

      $crate::Scene::<$platform>::access_tree($facts)
    }

    /// The accessibility tree as indented text, one node a line
    /// ([`Scene::access_outline`](skene_foundation::Scene::access_outline)).
    #[must_use]
    pub fn access_outline() -> ::std::string::String {
      $crate::end_turn();

      $crate::Scene::<$platform>::access_outline($facts)
    }

    /// The first node in tree order whose role is `role` and whose accessible name is
    /// `name`, outside hidden boxes and closed overlays.
    #[must_use]
    pub fn find_by_role(role: $crate::Role, name: &str) -> ::std::option::Option<$crate::NodeId> {
      $crate::end_turn();

      $crate::Scene::<$platform>::find_by_role(role, name, $facts)
    }

    /// Every node whose role is `role`, in tree order, outside hidden boxes and closed
    /// overlays.
    #[must_use]
    pub fn find_all_by_role(role: $crate::Role) -> ::std::vec::Vec<$crate::NodeId> {
      $crate::end_turn();

      $crate::Scene::<$platform>::find_all_by_role(role, $facts)
    }

    /// Every node that must be named and is not, in tree order, outside hidden boxes
    /// and closed overlays
    /// ([`Scene::find_all_unnamed`](skene_foundation::Scene::find_all_unnamed)).
    #[must_use]
    pub fn find_all_unnamed() -> ::std::vec::Vec<$crate::NodeId> {
      $crate::end_turn();

      $crate::Scene::<$platform>::find_all_unnamed($facts)
    }

    /// The first run in tree order whose own text is `text`, whitespace collapsed on
    /// both, outside hidden boxes and closed overlays.
    #[must_use]
    pub fn find_by_text(text: &str) -> ::std::option::Option<$crate::NodeId> {
      $crate::end_turn();

      $crate::Scene::<$platform>::find_by_text(text, $facts)
    }
  };
}

#[cfg(test)]
mod tests {
  use accesskit::Action;

  use crate::spine::{AccessProp, AccessText, NameFacts, NodeId, Role};

  /// An entry with runs reports the editing actions its state allows: an editable one takes
  /// a value, a selection and a replacement of what is selected, a read-only one only a
  /// selection, and a disabled one none.
  #[test]
  fn an_entry_reports_the_editing_actions_its_state_allows() {
    let actions = |access: Vec<AccessProp>| {
      let facts = NameFacts {
        role: Some(Role::TextInput),
        access,
        text_runs: Some(AccessText::default()),
        ..NameFacts::default()
      };
      let (node, _) = facts.access_node(NodeId::new(0, 0), None, &mut Vec::new());

      [
        Action::SetValue,
        Action::SetTextSelection,
        Action::ReplaceSelectedText,
      ]
      .map(|action| node.supports_action(action))
    };

    assert_eq!(actions(Vec::new()), [true; 3]);
    assert_eq!(
      actions(vec![AccessProp::ReadOnly(true)]),
      [false, true, false]
    );
    assert_eq!(actions(vec![AccessProp::Disabled(true)]), [false; 3]);
  }
}
