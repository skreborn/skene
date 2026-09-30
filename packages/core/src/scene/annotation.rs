//! The per-child properties a wrapper asks for the next box built under it.

use std::fmt::Debug;

use super::Scene;
use crate::{
  Condition,
  DeclaredBoxStyle,
  Dimension,
  Flexibility,
  GridPlacement,
  Inset,
  ItemAlignment,
  Line,
  NodeId,
  Platform,
  Value,
};

/// The per-child properties a wrapper asked for the next box built under it.
///
/// Folded into the child's own declared style before the child is created, so each
/// takes conditions, tokens and derived values like any property.
#[must_use]
pub struct Annotation<P: Platform> {
  /// The child's flexibility in a flex container.
  pub flex: Option<Annotated<P, Flexibility<P::Scalar>>>,
  /// The child's own cross- or block-axis alignment.
  pub self_alignment: Option<Annotated<P, ItemAlignment>>,
  /// The child's own inline-axis alignment in a grid.
  pub self_horizontal_alignment: Option<Annotated<P, ItemAlignment>>,
  /// The edges the child is pinned to in a stack.
  pub pinned: Option<Annotated<P, Inset<Dimension<P::Scalar>>>>,
  /// The child's row placement in a grid.
  pub grid_row_placement: Option<Annotated<P, Line<GridPlacement>>>,
  /// The child's column placement in a grid.
  pub grid_column_placement: Option<Annotated<P, Line<GridPlacement>>>,
}

/// A property value as an annotation carries it: declared under the platform's
/// conditions.
pub type Annotated<P, T> = Value<T, Condition<<P as Platform>::Scalar>>;

impl<P: Platform> Annotation<P> {
  /// This annotation as a box style holding its properties and nothing else.
  pub fn into_style(self) -> DeclaredBoxStyle<P> {
    let mut style = DeclaredBoxStyle::<P>::default();

    self.apply_to(&mut style);

    style
  }

  /// Writes every property set here into `style`, replacing what it held.
  pub fn apply_to(self, style: &mut DeclaredBoxStyle<P>) {
    if let Some(flex) = self.flex {
      style.flex = Some(flex);
    }

    if let Some(alignment) = self.self_alignment {
      style.self_alignment = Some(alignment);
    }

    if let Some(alignment) = self.self_horizontal_alignment {
      style.self_horizontal_alignment = Some(alignment);
    }

    if let Some(pinned) = self.pinned {
      style.pinned = Some(pinned);
    }

    if let Some(placement) = self.grid_row_placement {
      style.grid_row_placement = Some(placement);
    }

    if let Some(placement) = self.grid_column_placement {
      style.grid_column_placement = Some(placement);
    }
  }
}

impl<P: Platform> Default for Annotation<P> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      flex: None,
      self_alignment: None,
      self_horizontal_alignment: None,
      pinned: None,
      grid_row_placement: None,
      grid_column_placement: None,
    }
  }
}

impl<P: Platform> Clone for Annotation<P> {
  fn clone(&self) -> Self {
    Self {
      flex: self.flex.clone(),
      self_alignment: self.self_alignment.clone(),
      self_horizontal_alignment: self.self_horizontal_alignment.clone(),
      pinned: self.pinned.clone(),
      grid_row_placement: self.grid_row_placement.clone(),
      grid_column_placement: self.grid_column_placement.clone(),
    }
  }
}

impl<P: Platform> PartialEq for Annotation<P> {
  fn eq(&self, other: &Self) -> bool {
    self.flex == other.flex
      && self.self_alignment == other.self_alignment
      && self.self_horizontal_alignment == other.self_horizontal_alignment
      && self.pinned == other.pinned
      && self.grid_row_placement == other.grid_row_placement
      && self.grid_column_placement == other.grid_column_placement
  }
}

impl<P: Platform> Debug for Annotation<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Annotation")
      .field("flex", &self.flex)
      .field("self_alignment", &self.self_alignment)
      .field("self_horizontal_alignment", &self.self_horizontal_alignment)
      .field("pinned", &self.pinned)
      .field("grid_row_placement", &self.grid_row_placement)
      .field("grid_column_placement", &self.grid_column_placement)
      .finish()
  }
}

impl<P: Platform> Scene<P> {
  /// Builds the node `build` returns under `annotation`, where there is one: the first
  /// box, control, run or low-level node `build` creates takes it, and no node built
  /// after `build` returns does.
  pub fn annotate(
    annotation: impl Into<Option<Annotation<P>>>,
    build: impl FnOnce() -> NodeId,
  ) -> NodeId {
    let annotation = annotation.into();

    Self::with(|scene| scene.pending_annotation = annotation);

    let node = build();

    Self::with(|scene| scene.pending_annotation = None);

    node
  }

  /// Stores the annotation the next box, control, run or low-level node created takes;
  /// [`annotate`](Self::annotate) scopes it to one build.
  #[inline(always)]
  pub fn set_pending_annotation(&mut self, annotation: Annotation<P>) {
    self.pending_annotation = Some(annotation);
  }

  /// Takes the pending annotation, for the node about to be created: a box folds it
  /// into its style and records it, any other node records it.
  #[inline(always)]
  pub const fn take_pending_annotation(&mut self) -> Option<Annotation<P>> {
    self.pending_annotation.take()
  }

  /// Records the annotation a box was built with.
  #[inline(always)]
  pub fn set_annotation(&mut self, node: NodeId, annotation: Annotation<P>) {
    if let Some(record) = self.record_mut(node) {
      record.annotation = Some(annotation);
    }
  }
}
