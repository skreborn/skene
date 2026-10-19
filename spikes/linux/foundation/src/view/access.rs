//! What a view tells a reader beyond what its toolkit reports: the facts any node may carry,
//! each fixed or derived, merged by the shared half into the node's facts and kept current.

use std::rc::Rc;

use crate::{
  NodeId,
  seam::{
    Realization,
    access::{AccessFacts, Shortcut},
    tree,
  },
  view::NodeHandle,
};

/// A fact read now, tracking what it reads.
type Derive<T> = Rc<dyn Fn() -> T>;

/// A view's accessibility facts, as its setters gave them.
#[derive(Clone, Default)]
pub struct Access {
  /// The accessible name, where no caption names the node.
  pub name: Option<Derive<Rc<str>>>,
  /// The captions whose text names the node.
  pub labelled_by: Vec<NodeHandle>,
  /// A description.
  pub description: Option<Derive<Rc<str>>>,
  /// The nodes whose text describes the node.
  pub described_by: Vec<NodeHandle>,
  /// Whether a value is required.
  pub required: Option<Derive<bool>>,
  /// Whether the value is invalid.
  pub invalid: Option<Derive<bool>>,
  /// The keyboard shortcut that activates the node.
  pub shortcut: Option<Shortcut>,
}

impl Access {
  /// States the facts on `id`'s accessible, kept current; nothing where none was given.
  pub fn apply<R: Realization>(self, id: NodeId) {
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

    tree::add_access::<R>(id, move || AccessFacts {
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
