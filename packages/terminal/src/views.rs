//! The views of the terminal's own, each forwarding to the foundation's implementation.

mod container;
mod flex;
mod flexible;
mod grid;
mod grid_cell;
mod pinned;
mod span;
mod stack;
mod text;

pub use self::{
  container::Container,
  flex::{Column, Row},
  flexible::Flexible,
  grid::Grid,
  grid_cell::GridCell,
  pinned::Pinned,
  span::{RichText, Span},
  stack::Stack,
  text::Text,
};
