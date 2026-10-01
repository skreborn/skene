//! The views of the terminal's own, each forwarding to the foundation's implementation.

mod container;
mod enhance;
mod error_boundary;
mod flex;
mod flexible;
mod grid;
mod grid_cell;
mod pinned;
mod show;
mod span;
mod stack;
mod suspense;
mod switch;
mod text;
mod theme;

pub use self::{
  container::Container,
  enhance::Enhance,
  error_boundary::ErrorBoundary,
  flex::{Column, Row},
  flexible::Flexible,
  grid::Grid,
  grid_cell::GridCell,
  pinned::Pinned,
  show::Show,
  span::{RichText, Span},
  stack::Stack,
  suspense::Suspense,
  switch::Switch,
  text::Text,
  theme::Theme,
};
