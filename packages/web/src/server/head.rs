//! The page head a server render serializes: the title, the elements added through the
//! head module, the language, and the document around the body.

use skene_common::gui::{HeadElement, HeadId};
use skene_core::ColorScheme;

use super::{PREEMPT_SCHEME, VIEW_TRANSITIONS, served_document, with_tree};

/// Marks the page as one whose reader may choose a scheme.
///
/// Its head then carries the blocking script applying the stored choice before the
/// first paint; the choice itself is the reader's and never known to a server.
pub fn set_scheme(_scheme: Option<ColorScheme>) {
  PREEMPT_SCHEME.with(|preempt| preempt.set(true));
}

/// The color scheme a previous visit persisted; never known to a server.
#[inline(always)]
#[must_use]
pub const fn persisted_scheme() -> Option<ColorScheme> {
  None
}

/// Opts the pages into view transitions, this render and every one after on the thread.
///
/// The stylesheet carries `@view-transition { navigation: auto }`, so a browser that
/// has them animates the cross-document navigations between the served pages; one that
/// does not cuts.
pub fn enable_view_transitions() {
  VIEW_TRANSITIONS.with(|enabled| enabled.set(true));
}

/// Registers `title` for the page, in force while it is the latest live one.
pub fn add_head_title(title: &str) -> HeadId {
  with_tree(|tree| tree.head.add_title(title.to_owned()))
}

/// Rewrites the title registration `id`.
pub fn set_head_title(id: HeadId, title: &str) {
  with_tree(|tree| tree.head.set_title(id, title.to_owned()));
}

/// Removes the head registration `id`.
pub fn remove_head_entry(id: HeadId) {
  with_tree(|tree| tree.head.remove(id));
}

/// Sets the page's language, a BCP 47 tag written as its root's `lang`.
pub fn set_document_language(language: &str) {
  with_tree(|tree| tree.language = Some(language.to_owned()));
}

/// The opening of a served document up to its body's content, in `language` when one
/// is set: see [`Document::opening`], the document carrying the bundle, the dev loop
/// and the nonce set for the renders.
///
/// [`Document::opening`]: crate::Document::opening
#[must_use]
pub fn document_opening(language: Option<&str>, head: &str, styles: &str) -> String {
  served_document(language).opening(head, styles)
}

/// The closing of a served document from the hydration `state`: see
/// [`Document::closing`].
///
/// [`Document::closing`]: crate::Document::closing
#[must_use]
pub fn document_closing(state: &str) -> String {
  served_document(None).closing(Some(state))
}

/// Registers an element for the page head, in force while it is the latest live one
/// of what it stands for; see [`Head`].
///
/// # Panics
///
/// Panics when `tag` is not an element a head holds (`base`, `link`, `meta` or
/// `script`) or an attribute name is not one.
///
/// [`Head`]: skene_common::gui::Head
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) -> HeadId {
  with_tree(|tree| tree.head.add_element(HeadElement::new(tag, attributes)))
}
