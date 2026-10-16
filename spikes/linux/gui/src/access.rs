//! What a view tells a reader beyond what its toolkit reports: the facts any node may carry,
//! each fixed or derived, merged by the shared half into the node's facts and kept current.

use std::rc::Rc;

use skene_foundation::{
  NodeId,
  seam::{
    access::{AccessFacts, Shortcut},
    tree,
  },
};

use crate::{Active, view::NodeHandle};

type Derive<T> = Rc<dyn Fn() -> T>;

/// A view's accessibility facts, as its setters gave them.
#[derive(Clone, Default)]
pub(crate) struct Access {
  /// The accessible name, where no caption names the node.
  pub(crate) name: Option<Derive<Rc<str>>>,
  /// The captions whose text names the node.
  pub(crate) labelled_by: Vec<NodeHandle>,
  /// A description.
  pub(crate) description: Option<Derive<Rc<str>>>,
  /// The nodes whose text describes the node.
  pub(crate) described_by: Vec<NodeHandle>,
  /// Whether a value is required.
  pub(crate) required: Option<Derive<bool>>,
  /// Whether the value is invalid.
  pub(crate) invalid: Option<Derive<bool>>,
  /// The keyboard shortcut that activates the node.
  pub(crate) shortcut: Option<Shortcut>,
}

impl Access {
  /// States the facts on `id`'s accessible, kept current; nothing where none was given.
  pub(crate) fn apply(self, id: NodeId) {
    let Self {
      name,
      labelled_by,
      description,
      described_by,
      required,
      invalid,
      shortcut,
    } = self;

    if shortcut.is_none()
      && name.is_none()
      && labelled_by.is_empty()
      && description.is_none()
      && described_by.is_empty()
      && required.is_none()
      && invalid.is_none()
    {
      return;
    }

    let nodes = |handles: &[NodeHandle]| handles.iter().filter_map(NodeHandle::node).collect();

    tree::add_access::<Active>(id, move || AccessFacts {
      shortcut,
      name: name.as_ref().map(|name| name()),
      labelled_by: nodes(&labelled_by),
      description: description.as_ref().map(|description| description()),
      described_by: nodes(&described_by),
      required: required.as_ref().is_some_and(|required| required()),
      invalid: invalid.as_ref().is_some_and(|invalid| invalid()),
      ..AccessFacts::default()
    });
  }
}
