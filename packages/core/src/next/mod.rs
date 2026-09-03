//! The property, condition, expression and token model.
//!
//! Transitional home of the model that replaces the crate's flat style exports
//! ([`BoxStyle`](crate::BoxStyle), [`Condition`](crate::Condition),
//! [`Adaptive`](crate::Adaptive), [`Token`](crate::Token) and their kin): both stay until
//! every facade builds on this module, after which it is flattened into the crate root
//! under these same names.
//!
//! A property's value is a [`Value`]: a [`Leaf`]—fixed, a token reference, an expression,
//! or derived from signals—under conditions ([`Condition`]) that pick a variant. A
//! [`Token`]'s [`Definition`] is the same shape without the derived leaf and with only
//! [`Media`] conditions, so a palette is plain data. A style ([`BoxStyle`], [`RunStyle`])
//! holds one field per property in a [`Shape`]: what a builder collects, what a driver
//! receives, or plain values. Every facade fixes the [`Scalar`] its lengths are measured
//! in and the [`StyleExtension`] holding the properties only its medium has.

mod border;
mod box_style;
mod condition;
mod expr;
mod extension;
mod leaf;
mod property;
mod run_style;
mod scalar;
mod shape;
mod token;

pub use border::Border;
pub use box_style::{
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxSlot,
  BoxStyle,
  BoxValue,
  BoxVisitor,
  BoxVisitorMut,
  IntoBoxValue,
  box_fields,
};
pub use condition::{Condition, ContainerName, ContainerQuery, Media, MediaEnv, State};
pub use expr::{Expr, ExprEnv, ExprNode, ExprOutput, ExprValue, Length, Number, Quantity};
pub use extension::{NoProperty, NoValue, StyleExtension};
pub use leaf::{
  Conditional,
  Definition,
  DerivedSlot,
  DriverLeaf,
  Leaf,
  Slot,
  StaticLeaf,
  StaticValue,
  Value,
  When,
};
pub use property::{Property, PropertyKey, PropertyName};
pub use run_style::{
  IntoRunValue,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunSlot,
  RunStyle,
  RunValue,
  RunVisitor,
  RunVisitorMut,
  run_fields,
};
pub use scalar::Scalar;
pub use shape::{Declared, Plain, Shape, Static};
pub use token::{Definitions, IntoTokenValue, Themed, Token, TokenName, TokenRef, TokenSet};
