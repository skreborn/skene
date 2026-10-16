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
  /// These facts with `caption` naming the node, unless they name it another way.
  pub(crate) fn captioned(mut self, caption: &NodeHandle) -> Self {
    if self.name.is_none() && self.labelled_by.is_empty() {
      self.labelled_by = vec![*caption];
    }

    self
  }

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

/// The accessibility setters of a view with an `access: Access` field.
macro_rules! access {
  () => {
    /// Names it for a reader where no caption names it.
    pub fn set_accessible_name(self, name: impl Into<::std::rc::Rc<str>>) -> Self {
      let name = name.into();

      self.derive_accessible_name(move || ::std::rc::Rc::clone(&name))
    }

    /// Names it for a reader where no caption names it, kept current.
    pub fn derive_accessible_name<S: Into<::std::rc::Rc<str>>>(
      mut self,
      name: impl Fn() -> S + 'static,
    ) -> Self {
      self.access.name = Some(::std::rc::Rc::new(move || name().into()));

      self
    }

    /// Names it by the text of the node `caption` captured, placed anywhere.
    pub fn set_labelled_by(mut self, caption: &$crate::NodeHandle) -> Self {
      self.access.labelled_by = vec![*caption];

      self
    }

    /// Describes it for a reader.
    pub fn set_accessible_description(self, description: impl Into<::std::rc::Rc<str>>) -> Self {
      let description = description.into();

      self.derive_accessible_description(move || ::std::rc::Rc::clone(&description))
    }

    /// Describes it for a reader, kept current.
    pub fn derive_accessible_description<S: Into<::std::rc::Rc<str>>>(
      mut self,
      description: impl Fn() -> S + 'static,
    ) -> Self {
      self.access.description = Some(::std::rc::Rc::new(move || description().into()));

      self
    }

    /// Describes it by the text of the node `description` captured, placed anywhere.
    pub fn set_described_by(mut self, description: &$crate::NodeHandle) -> Self {
      self.access.described_by = vec![*description];

      self
    }

    /// Whether a value is required.
    pub fn set_required(self, required: bool) -> Self {
      self.derive_required(move || required)
    }

    /// Whether a value is required, kept current.
    pub fn derive_required(mut self, required: impl Fn() -> bool + 'static) -> Self {
      self.access.required = Some(::std::rc::Rc::new(required));

      self
    }

    /// Whether the value is invalid.
    pub fn set_invalid(self, invalid: bool) -> Self {
      self.derive_invalid(move || invalid)
    }

    /// Whether the value is invalid, kept current.
    pub fn derive_invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
      self.access.invalid = Some(::std::rc::Rc::new(invalid));

      self
    }
  };
}

pub(crate) use access;

/// `$view` given the facts the [`Access`] `$access` holds, each through the view's public
/// setter.
macro_rules! give_access {
  ($access:expr, $view:expr) => {{
    let access: $crate::access::Access = $access;
    let mut view = $view;

    if let Some(name) = access.name {
      view = view.derive_accessible_name(move || name());
    }

    if let Some(caption) = access.labelled_by.first() {
      view = view.set_labelled_by(caption);
    }

    if let Some(description) = access.description {
      view = view.derive_accessible_description(move || description());
    }

    if let Some(node) = access.described_by.first() {
      view = view.set_described_by(node);
    }

    if let Some(required) = access.required {
      view = view.derive_required(move || required());
    }

    if let Some(invalid) = access.invalid {
      view = view.derive_invalid(move || invalid());
    }

    view
  }};
}

pub(crate) use give_access;
