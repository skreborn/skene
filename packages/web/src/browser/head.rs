//! The document head in the browser: the registered titles and elements applied to the
//! live document, and its language.

use std::cell::RefCell;

use skene_foundation::web::{Head, HeadElement, HeadId};
use web_sys::{Element, HtmlHeadElement};

thread_local! {
  /// The head as registered and the live elements standing for those in force.
  static HEAD: RefCell<Applied> = RefCell::new(Applied::default());
}

/// The registrations, and the live element standing for each element in force, in
/// order.
#[derive(Default)]
struct Applied {
  head: Head,
  elements: Vec<(HeadElement, Element)>,
}

impl Applied {
  /// Writes the registrations in force into the live document: the title, and each
  /// element as the one already standing for it, an identical one in the head (as a
  /// server render leaves), or a new one placed before the next in force; an element no
  /// longer in force leaves.
  fn apply(&mut self) {
    let Some(document) = web_sys::window().and_then(|window| window.document())
    else {
      return;
    };
    let Some(parent) = document.head()
    else {
      return;
    };

    if let Some(title) = self.head.title()
      && document.title() != title
    {
      document.set_title(title);
    }

    let mut previous = std::mem::take(&mut self.elements);
    let mut created = Vec::new();

    for element in self.head.elements() {
      let live = if let Some(at) = previous.iter().position(|(held, _)| held == element) {
        previous.swap_remove(at).1
      }
      else if let Some(existing) = adoptable(&parent, element, &self.elements, &previous) {
        existing
      }
      else if let Ok(fresh) = document.create_element(&element.tag) {
        for (name, value) in &element.attributes {
          let _ = fresh.set_attribute(name, value);
        }

        created.push(self.elements.len());

        fresh
      }
      else {
        continue;
      };

      self.elements.push((element.clone(), live));
    }

    for (_, stale) in previous {
      stale.remove();
    }

    let mut before: Option<&Element> = None;

    for (at, (_, element)) in self.elements.iter().enumerate().rev() {
      if created.contains(&at) {
        let _ = parent.insert_before(element, before.map(AsRef::as_ref));
      }

      before = Some(element);
    }
  }
}

/// An element of the head identical to `element` that neither `held` nor `previous`
/// holds.
fn adoptable(
  parent: &HtmlHeadElement,
  element: &HeadElement,
  held: &[(HeadElement, Element)],
  previous: &[(HeadElement, Element)],
) -> Option<Element> {
  let children = parent.children();

  (0..children.length())
    .filter_map(|index| children.item(index))
    .find(|existing| {
      existing.tag_name().eq_ignore_ascii_case(&element.tag)
        && element
          .attributes
          .iter()
          .all(|(name, value)| existing.get_attribute(name).as_deref() == Some(value))
        && !held
          .iter()
          .chain(previous)
          .any(|(_, element)| element == existing)
    })
}

/// Runs `change` on the head, then applies what is in force.
fn with_head<R>(change: impl FnOnce(&mut Head) -> R) -> R {
  HEAD.with(|applied| {
    let mut applied = applied.borrow_mut();
    let result = change(&mut applied.head);

    applied.apply();

    result
  })
}

/// Registers `title` for the document, in force while it is the latest live one.
pub fn add_head_title(title: &str) -> HeadId {
  with_head(|head| head.add_title(title.to_owned()))
}

/// Rewrites the title registration `id`.
pub fn set_head_title(id: HeadId, title: &str) {
  with_head(|head| head.set_title(id, title.to_owned()));
}

/// Registers an element for the document head, in force while it is the latest live one
/// of what it stands for.
///
/// See [`Head`]. An identical element already in the head, as a server render leaves,
/// is adopted rather than duplicated.
///
/// # Panics
///
/// Panics when `tag` is not an element a head holds (`base`, `link`, `meta` or
/// `script`) or an attribute name is not one.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) -> HeadId {
  with_head(|head| head.add_element(HeadElement::new(tag, attributes)))
}

/// Removes the head registration `id`, and the element it wrote once none stands for it.
pub fn remove_head_entry(id: HeadId) {
  with_head(|head| head.remove(id));
}

/// Sets the document's language, a BCP 47 tag, as its root's `lang`.
pub fn set_document_language(language: &str) {
  if let Some(root) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.document_element())
  {
    let _ = root.set_attribute("lang", language);
  }
}

/// Forgets every registration and removes the elements standing for them.
pub(super) fn reset() {
  HEAD.with(|applied| {
    for (_, element) in applied.take().elements {
      element.remove();
    }
  });
}
