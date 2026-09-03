//! # Built-in views
//!
//! Every built-in view once, generic over the [`Driver`] a facade names. A facade
//! aliases them (`type Container = skene_views::Container<ActiveDriver>`); a component
//! library writes against them exactly as a facade does.
//!
//! A view's build folds the annotation a wrapper left pending into its declared style,
//! splits the style into the static part a driver compiles and the derived slots render
//! effects write, creates its node, registers it in the [`Scene`], binds each slot,
//! registers its handlers, mounts its children and returns its id. [`LayoutBox`] is that
//! build for a box; [`Container`] and [`Column`] are thin vocabularies over it, and a
//! third party's layout view is built from it the same way.

mod column;
mod container;
mod flexible;
mod layout_box;
pub mod realize;
mod text;
mod theme;

pub use skene_core::next::{
  AnyKind,
  AnyView,
  Driver,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  IntoView,
  NoChild,
  Scene,
  Show,
  View,
};

pub use crate::{
  column::Column,
  container::Container,
  flexible::Flexible,
  layout_box::LayoutBox,
  text::Text,
  theme::{ChildBuilder, Theme},
};
