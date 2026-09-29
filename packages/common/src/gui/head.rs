//! The document head as an app registers it: titles and elements, each live until
//! removed, the latest of what each stands for in force.

/// An element registered for the document head: its tag and its attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct HeadElement {
  /// The element's tag.
  pub tag: String,
  /// The element's attributes, in registration order.
  pub attributes: Vec<(String, String)>,
}

impl HeadElement {
  /// An element of `tag` with `attributes`.
  pub fn new(tag: &str, attributes: &[(&str, &str)]) -> Self {
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

/// The document head as registered: titles and elements, each live until removed.
///
/// The latest live title is the one in force, and so is the latest live element of
/// what it stands for, in its registration's place: a nested route's description
/// replaces its parent's until it leaves, and identical registrations write one
/// element.
#[derive(Debug, Default)]
pub struct Head {
  entries: Vec<(HeadId, Entry)>,
  next: u64,
}

/// One registration.
#[derive(Debug)]
enum Entry {
  Title(String),
  Element(HeadElement),
}

impl Head {
  /// Registers `title`.
  pub fn add_title(&mut self, title: String) -> HeadId {
    self.add(Entry::Title(title))
  }

  /// Registers `element`.
  pub fn add_element(&mut self, element: HeadElement) -> HeadId {
    self.add(Entry::Element(element))
  }

  /// Rewrites the title registration `id`; any other registration is left as it is.
  pub fn set_title(&mut self, id: HeadId, title: String) {
    if let Some((_, Entry::Title(current))) = self.entries.iter_mut().find(|(at, _)| *at == id) {
      *current = title;
    }
  }

  /// Removes the registration `id`.
  pub fn remove(&mut self, id: HeadId) {
    self.entries.retain(|(at, _)| *at != id);
  }

  /// The title in force.
  #[must_use]
  pub fn title(&self) -> Option<&str> {
    self
      .entries
      .iter()
      .rev()
      .find_map(|(_, entry)| match entry {
        Entry::Title(title) => Some(title.as_str()),
        Entry::Element(_) => None,
      })
  }

  /// The elements in force, in their registrations' order.
  pub fn elements(&self) -> impl Iterator<Item = &HeadElement> {
    self
      .entries
      .iter()
      .enumerate()
      .filter_map(|(at, (_, entry))| {
        let Entry::Element(element) = entry
        else {
          return None;
        };
        let replaced = self.entries[at + 1..]
          .iter()
          .any(|(_, later)| matches!(later, Entry::Element(later) if later.stands_for(element)));

        (!replaced).then_some(element)
      })
  }

  /// Registers `entry`.
  fn add(&mut self, entry: Entry) -> HeadId {
    let id = HeadId(self.next);

    self.next += 1;
    self.entries.push((id, entry));

    id
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

  /// The latest live title is in force, and removing it restores the one before.
  #[test]
  fn titles_stack() {
    let mut head = Head::default();
    let outer = head.add_title("Outer".to_owned());
    let inner = head.add_title("Inner".to_owned());

    assert_eq!(head.title(), Some("Inner"));

    head.set_title(outer, "Outer 2".to_owned());
    head.remove(inner);

    assert_eq!(head.title(), Some("Outer 2"));

    head.remove(outer);

    assert_eq!(head.title(), None);
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
