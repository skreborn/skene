//! The realization seam: what crosses between the facade's shared views and each pixel
//! platform's realization, what the shared half (written once, here) tells a realization, what
//! the realization reports back, and what each side owns.
//!
//! **One record under Skene's uniform names.** Every property a view sets crosses as an entry
//! of a [`record::Record`] keyed by [`record::Prop`], and every content value under a
//! [`content::Content`] name, never as a typed function per control per platform. Both are
//! typed where the view writes them ([`record::Key`], [`content::ContentKey`]), so a value of
//! the wrong kind does not compile, and erased where the realization reads them. A
//! realization is a dispatcher over `(kind, name)`: the compiler does not prove it total, so
//! [`testing::contents_gate`] proves, on the real platform, that every content a kind takes
//! ([`node::NodeKind::contents`]) is shown and read back.
//!
//! **Layout belongs to the foundation's engine.** On a platform whose toolkit is not CSS, the
//! layout names never reach the toolkit: the shared half resolves them and hands them to the
//! realization's host container ([`Realization::apply_layout`]), which runs
//! [`crate::layout::ContainerLayout`] over the toolkit's own measuring
//! ([`crate::layout::LayoutHost`]). The web's toolkit is CSS, whose flexbox and grid are what
//! taffy implements, so it would take the layout names as style ([`LayoutOwner::Toolkit`]).
//!
//! **Style crosses unresolved, as far as the toolkit resolves it.** A realization names one of
//! three levels ([`style::Keep`]): the web keeps everything; a toolkit with a style language
//! short of CSS's may keep color tokens as named colors and states as its own conditions,
//! derived leaves as per-node slots; a realization that draws (Linux) keeps nothing. The
//! shared half compiles a node's record to that level in one effect and hands the realization
//! a [`style::CompiledStyle`] only when it changed, its address a function of the style's
//! content alone.
//!
//! **Controls are controlled.** A report of a user change is synchronous and answered with a
//! [`content::Verdict`]. A value constrained by another travels with it as one value
//! ([`content::Range`], [`content::Choice`], [`content::Edit`]). The echo rule, the
//! equal-skip, the hold during composition, the repair of a change the toolkit applied before
//! asking, and the text arithmetic in the realization's unit live here, once
//! ([`content::Controller`]).
//!
//! **No double.** Every platform has one realization, the real one. Tests drive it through
//! [`testing::Driver`], which each realization implements over its own accessibility layer and
//! input injection, waiting on [`activity`].

pub mod access;
pub mod activity;
pub mod content;
pub mod door;
pub mod node;
pub mod picture;
pub mod realization;
pub mod record;
pub mod report;
pub mod runtime;
pub mod style;
pub mod testing;
pub mod tree;

pub use self::realization::{LayoutOwner, Realization};

#[cfg(test)]
mod tests;
