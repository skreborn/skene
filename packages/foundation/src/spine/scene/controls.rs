//! Controls: their state, their parts, and what each node reports to assistive
//! technology.

use std::rc::Rc;

use super::{Mirror, NodeKind, NodeRecord, Scene};
use crate::spine::{AccessProp, ControlSink, ControlState, NodeId, Part, Realization};

/// What a node reports to assistive technology: the view's own report, and the report
/// of a view describing it.
#[derive(Default)]
pub(super) struct AccessReports {
  pub(super) own: Vec<AccessProp>,
  pub(super) description: Vec<AccessProp>,
}

impl AccessReports {
  /// Both reports, the node's own first, with their relations folded.
  pub(super) fn merged(&self) -> Vec<AccessProp> {
    AccessProp::fold(self.own.iter().chain(&self.description).cloned())
  }
}

impl<P: Realization> Scene<P> {
  /// Registers `sink` as what reads and writes the door's contents of `node`, a view
  /// adopted through the native door, replacing the sink registered before.
  pub fn set_sink(&mut self, node: NodeId, sink: Rc<dyn ControlSink>) {
    if let Some(record) = self.record_mut(node) {
      record.sink = Some(sink);
    }
  }

  /// The sink reading and writing the door's contents of `node`, where one is
  /// registered.
  #[must_use]
  pub fn sink(&self, node: NodeId) -> Option<Rc<dyn ControlSink>> {
    self.record(node)?.sink.clone()
  }

  /// Writes the state of the control `node` and mirrors it, bringing the platform back
  /// to it whether or not it changed; a node of any other kind is left alone.
  pub fn set_control_state(&mut self, node: NodeId, state: ControlState) {
    let Some(record) = self.record_mut(node)
    else {
      return;
    };

    if record.kind != NodeKind::Control {
      return;
    }

    if record.control.replace(state) != Some(state) {
      self.held.touch();
    }

    self.mirrors.push(Mirror::ControlState { node, state });
  }

  /// Whether the text entry `node` takes edits to its value: it is neither disabled nor
  /// read-only.
  #[must_use]
  pub fn is_editable(&self, node: NodeId) -> bool {
    self.record(node).is_some_and(|record| {
      record.entry
        && !record.control.is_some_and(|state| state.disabled)
        && !record.access.as_ref().is_some_and(|reports| {
          reports
            .own
            .iter()
            .chain(&reports.description)
            .any(|prop| *prop == AccessProp::ReadOnly(true))
        })
    })
  }

  /// The state of the control `node`; `None` for a node of any other kind.
  #[inline(always)]
  #[must_use]
  pub fn control_state(&self, node: NodeId) -> Option<ControlState> {
    self.record(node).and_then(NodeRecord::control_state)
  }

  /// The nearest control at or above `node`.
  #[must_use]
  pub fn enclosing_control(&self, node: NodeId) -> Option<NodeId> {
    let mut current = Some(node);

    while let Some(visited) = current {
      let record = self.record(visited)?;

      if record.kind == NodeKind::Control {
        return Some(visited);
      }

      current = record.parent;
    }

    None
  }

  /// Records `node` as `part` of the control enclosing it.
  #[inline(always)]
  pub fn set_part(&mut self, node: NodeId, part: Part) {
    if let Some(record) = self.record_mut(node) {
      record.part = Some(part);
    }
  }

  /// The first node under the control `control`, in tree order, recorded as `part`;
  /// the controls inside it are passed over with their parts.
  #[must_use]
  pub fn find_part(&self, control: NodeId, part: Part) -> Option<NodeId> {
    let mut stack: Vec<NodeId> = self.children_reversed(control).collect();

    while let Some(node) = stack.pop() {
      let Some(record) = self.record(node)
      else {
        continue;
      };

      if record.part == Some(part) {
        return Some(node);
      }

      if record.kind == NodeKind::Control {
        continue;
      }

      stack.extend(self.children_reversed(node));
    }

    None
  }

  /// The part of its enclosing control `node` is recorded as.
  #[inline(always)]
  #[must_use]
  pub fn part(&self, node: NodeId) -> Option<Part> {
    self.record(node).and_then(NodeRecord::part)
  }

  /// Notes `access` as what the view that made `node` reports, without a write: for
  /// a report the platform received as the node was created.
  pub fn note_access(&mut self, node: NodeId, access: Vec<AccessProp>) {
    if let Some(record) = self.record_mut(node) {
      record.access.get_or_insert_default().own = access;
    }
  }

  /// Replaces what the view that made `node` reports with `access`, returning the
  /// whole report the platform is to receive: the node's own, then its description,
  /// with their relations folded ([`AccessProp::fold`]).
  #[must_use]
  pub fn report_access(&mut self, node: NodeId, access: Vec<AccessProp>) -> Vec<AccessProp> {
    let Some(record) = self.record_mut(node)
    else {
      return access;
    };
    let reports = record.access.get_or_insert_default();

    reports.own = access;

    reports.merged()
  }

  /// What `node` reports to assistive technology: the view's own report, then its
  /// description, with their relations folded ([`AccessProp::fold`]).
  #[must_use]
  pub fn access(&self, node: NodeId) -> Vec<AccessProp> {
    self
      .record(node)
      .and_then(|record| record.access.as_deref())
      .map(AccessReports::merged)
      .unwrap_or_default()
  }

  /// Replaces what a view describing `node` reports with `description`, returning the
  /// whole report the platform is to receive: the node's own, then its description,
  /// with their relations folded ([`AccessProp::fold`]).
  #[must_use]
  pub fn describe(&mut self, node: NodeId, description: Vec<AccessProp>) -> Vec<AccessProp> {
    let Some(record) = self.record_mut(node)
    else {
      return description;
    };
    let reports = record.access.get_or_insert_default();

    reports.description = description;

    reports.merged()
  }
}
