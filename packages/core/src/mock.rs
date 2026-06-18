use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  ops::Deref,
};

use crate::{Backend, NodeId};

/// A backend used for testing.
///
/// Logs all structural changes.
#[must_use]
pub struct MockBackend {
  next: Cell<u64>,
  children: RefCell<HashMap<NodeId, Vec<NodeId>>>,
  parents: RefCell<HashMap<NodeId, NodeId>>,
  ops: RefCell<Vec<MockOp>>,
}

/// A recorded operation performed on [`MockBackend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MockOp {
  /// A group was created.
  CreateGroup {
    /// The created node.
    node: NodeId,
  },
  /// A child was attached to a parent.
  AttachChild {
    /// The parent node.
    parent: NodeId,
    /// The child node.
    child: NodeId,
    /// The node `child` was attached before, if any.
    before: Option<NodeId>,
  },
  /// A child was detached from a parent.
  DetachChild {
    /// The parent node.
    parent: NodeId,
    /// The child node.
    child: NodeId,
  },
  /// A node was destroyed.
  Destroy {
    /// The destroyed node.
    node: NodeId,
  },
}

impl Default for MockBackend {
  fn default() -> Self {
    MockBackend::new()
  }
}

impl MockBackend {
  /// Creates a new, empty instance.
  pub fn new() -> MockBackend {
    MockBackend {
      next: Cell::new(1),
      children: RefCell::new(HashMap::new()),
      parents: RefCell::new(HashMap::new()),
      ops: RefCell::new(Vec::new()),
    }
  }

  fn alloc(&self) -> NodeId {
    let id = NodeId::new(self.next.get());

    self.next.update(|value| value + 1);

    id
  }

  /// Returns the ordered list of children for `node`.
  ///
  /// Returns an empty list if `node` does not exist or has no children.
  pub fn children_of(&self, node: NodeId) -> Vec<NodeId> {
    self
      .children
      .borrow()
      .get(&node)
      .cloned()
      .unwrap_or_default()
  }

  /// Returns the ordered list of operations performed on this backend.
  pub fn ops(&self) -> Vec<MockOp> {
    self.ops.borrow().deref().clone()
  }
}

impl Backend for MockBackend {
  fn root(&self) -> NodeId {
    NodeId::new(0)
  }

  fn create_group(&self) -> NodeId {
    let node = self.alloc();

    self.ops.borrow_mut().push(MockOp::CreateGroup { node });

    node
  }

  fn attach_child(&self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
    self.ops.borrow_mut().push(MockOp::AttachChild {
      parent,
      child,
      before,
    });

    let mut children = self.children.borrow_mut();

    if let Some(old_parent) = self.parents.borrow_mut().insert(child, parent)
      && let Some(old_children) = children.get_mut(&old_parent)
    {
      old_children.retain(|node| *node != child);
    }

    let children = children.entry(parent).or_default();
    let index = before.and_then(|before| children.iter().position(|node| *node == before));

    if let Some(index) = index {
      children.insert(index, child);
    }
    else {
      children.push(child);
    }
  }

  fn detach_child(&self, parent: NodeId, child: NodeId) {
    if let Some(children) = self.children.borrow_mut().get_mut(&parent) {
      children.retain(|node| *node != child);
    }

    self.parents.borrow_mut().remove(&child);

    self
      .ops
      .borrow_mut()
      .push(MockOp::DetachChild { parent, child });
  }

  fn destroy(&self, node: NodeId) {
    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
      self.parents.borrow_mut().remove(&node);

      if let Some(children) = self.children.borrow_mut().remove(&node) {
        stack.extend(children);
      }

      self.ops.borrow_mut().push(MockOp::Destroy { node });
    }
  }
}
