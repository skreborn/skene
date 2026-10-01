//! The document head: the window's title in force, and the elements an app registers,
//! each live until removed, the latest of what each stands for in force.

/// An element registered for the document head: its tag and its attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct HeadElement {
  /// The element's tag.
  pub tag: String,
  /// The element's attributes, in registration order.
  pub attributes: Vec<(String, String)>,
}

/// The elements a head holds that a registration adds.
const HEAD_TAGS: [&str; 4] = ["base", "link", "meta", "script"];

impl HeadElement {
  /// An element of `tag` with `attributes`.
  ///
  /// # Panics
  ///
  /// Panics when `tag` is not an element a head holds (`base`, `link`, `meta` or
  /// `script`) or an attribute name is not one as HTML reads it: a letter, then
  /// letters, digits, `-`, `_`, `.` and `:`.
  pub fn new(tag: &str, attributes: &[(&str, &str)]) -> Self {
    assert!(
      HEAD_TAGS.contains(&tag),
      "a head element is one of {HEAD_TAGS:?}, not {tag:?}"
    );

    for (name, _) in attributes {
      let mut characters = name.chars();
      let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
          character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        });

      assert!(valid, "{name:?} is not an attribute name");
    }

    Self {
      tag: tag.to_owned(),
      attributes: attributes
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect(),
    }
  }

  /// Whether `self` stands for what `other` does, so the later of the two replaces
  /// the earlier: a `meta` of one name, property, `http-equiv` or `charset`, a `base`,
  /// or any element whose attributes all match.
  fn stands_for(&self, other: &Self) -> bool {
    if self.tag != other.tag {
      return false;
    }

    match (self.tag.as_str(), self.meta_key(), other.meta_key()) {
      ("base", ..) => true,
      ("meta", Some(key), Some(other)) => key == other,
      _ => self.attributes == other.attributes,
    }
  }

  /// The attribute a `meta` is known by and its value, when it carries one.
  fn meta_key(&self) -> Option<(&str, &str)> {
    self
      .attributes
      .iter()
      .find(|(name, _)| {
        matches!(
          name.as_str(),
          "name" | "property" | "http-equiv" | "charset"
        )
      })
      .map(|(name, value)| (name.as_str(), value.as_str()))
  }
}

/// A registration in a [`Head`], to rewrite or remove it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub struct HeadId(u64);

/// The document head: the window's title in force, and the elements registered, each
/// live until removed.
///
/// The latest live element of what it stands for is in force, in its registration's
/// place: a nested route's description replaces its parent's until it leaves, and
/// identical registrations write one element.
#[derive(Debug, Default)]
pub struct Head {
  title: Option<String>,
  entries: Vec<(HeadId, HeadElement)>,
  next: u64,
}

impl Head {
  /// Sets the window's title in force; `None` sets none.
  pub fn set_title(&mut self, title: Option<String>) {
    self.title = title;
  }

  /// The window's title in force.
  #[must_use]
  pub fn title(&self) -> Option<&str> {
    self.title.as_deref()
  }

  /// Registers `element`.
  pub fn add_element(&mut self, element: HeadElement) -> HeadId {
    let id = HeadId(self.next);

    self.next += 1;
    self.entries.push((id, element));

    id
  }

  /// Removes the registration `id`.
  pub fn remove(&mut self, id: HeadId) {
    self.entries.retain(|(at, _)| *at != id);
  }

  /// The elements in force, in their registrations' order.
  pub fn elements(&self) -> impl Iterator<Item = &HeadElement> {
    self
      .entries
      .iter()
      .enumerate()
      .filter_map(|(at, (_, element))| {
        let replaced = self.entries[at + 1..]
          .iter()
          .any(|(_, later)| later.stands_for(element));

        (!replaced).then_some(element)
      })
  }
}

#[cfg(test)]
mod tests {
  use super::{Head, HeadElement};

  fn meta(name: &str, content: &str) -> HeadElement {
    HeadElement::new("meta", &[("name", name), ("content", content)])
  }

  fn stylesheet(href: &str) -> HeadElement {
    HeadElement::new("link", &[("rel", "stylesheet"), ("href", href)])
  }

  /// A later meta of one name replaces the earlier in its own place until it leaves,
  /// and identical elements are written once while either lives.
  #[test]
  fn elements_replace_what_they_stand_for() {
    let mut head = Head::default();
    let parent = head.add_element(meta("description", "parent"));
    let first = head.add_element(stylesheet("/site.css"));
    let child = head.add_element(meta("description", "child"));
    let second = head.add_element(stylesheet("/site.css"));
    let _other = head.add_element(stylesheet("/other.css"));

    assert_eq!(
      head.elements().cloned().collect::<Vec<_>>(),
      [
        meta("description", "child"),
        stylesheet("/site.css"),
        stylesheet("/other.css")
      ]
    );

    head.remove(child);
    head.remove(second);

    assert_eq!(
      head.elements().cloned().collect::<Vec<_>>(),
      [
        meta("description", "parent"),
        stylesheet("/site.css"),
        stylesheet("/other.css")
      ]
    );

    head.remove(parent);
    head.remove(first);

    assert_eq!(
      head.elements().cloned().collect::<Vec<_>>(),
      [stylesheet("/other.css")]
    );
  }
}
