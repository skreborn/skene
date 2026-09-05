//! The document head in the browser: the title and the elements an app registers.

/// Sets the document title.
pub fn set_document_title(title: &str) {
  if let Some(document) = web_sys::window().and_then(|window| window.document()) {
    document.set_title(title);
  }
}

/// Adds an element to the document head, unless an identical one is already present
/// (as after hydrating a server render that registered the same element).
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  let Some(head) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.head())
  else {
    return;
  };

  let children = head.children();

  for index in 0..children.length() {
    let Some(existing) = children.item(index)
    else {
      continue;
    };

    let matches = existing.tag_name().eq_ignore_ascii_case(tag)
      && attributes
        .iter()
        .all(|(name, value)| existing.get_attribute(name).as_deref() == Some(*value));

    if matches {
      return;
    }
  }

  let Some(document) = web_sys::window().and_then(|window| window.document())
  else {
    return;
  };

  let Ok(element) = document.create_element(tag)
  else {
    return;
  };

  for (name, value) in attributes {
    let _ = element.set_attribute(name, value);
  }

  let _ = head.append_child(&element);
}
