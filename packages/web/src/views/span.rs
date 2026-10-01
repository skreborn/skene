//! Inline runs, and the paragraph they flow in.

use crate::ActivePlatform;

/// An inline run within a [`RichText`]: text, a group of spans styled as a unit, or a
/// view embedded among the text.
///
/// Spans flow together and wrap as one paragraph, so styling can vary within a line. A
/// property left unset is inherited through its [`typography`](crate::typography)
/// token. A [group](Self::group) redefines the typography tokens of the properties it
/// sets for the spans beneath it, its type metrics among them, on its own run: a derived
/// leaf is redefined with each value it computes, and a variant conditioned on a state
/// applies to the group's own run alone. A [view](Self::view) is laid out as an
/// inline-level box among the surrounding runs.
#[must_use]
pub struct Span(skene_foundation::Span<ActivePlatform>);

skene_foundation::impl_span!(Span, ActivePlatform);

/// A paragraph of [`Span`]s, flowing together and wrapping as one.
///
/// Carries no properties of its own; decorating it means wrapping it in a
/// [`Container`](crate::Container).
#[must_use]
pub struct RichText(skene_foundation::RichText<ActivePlatform>);

skene_foundation::impl_rich_text!(RichText, Span, ActivePlatform);
