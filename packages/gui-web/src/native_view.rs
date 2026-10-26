//! The native door on the web: an element of the DOM inside a Skene tree.

use std::borrow::Cow;

use skene_foundation::{AnyKind, BoxFacts, Layout, LowLevel, NodeFacts, NodeId, NodeSpec};

use crate::Web;

/// The markup a [`NativeView`] describes: a tag and its attributes, what the server
/// renders as the view's placeholder and the browser's element starts with.
///
/// The server leaf's native view
/// ([`Realization::Native`](skene_foundation::Realization::Native)): its native door
/// adopts the placeholder, rendered as markup a hydrating browser adopts in place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct NativeMarkup {
  tag: &'static str,
  attributes: Vec<(&'static str, Cow<'static, str>)>,
}

impl NativeMarkup {
  /// The tag of the element.
  #[inline(always)]
  #[must_use]
  pub const fn tag(&self) -> &'static str {
    self.tag
  }

  /// Each attribute of the element by name, in the order their names were first added.
  pub fn attributes(&self) -> impl Iterator<Item = (&'static str, &str)> {
    self
      .attributes
      .iter()
      .map(|(name, value)| (*name, value.as_ref()))
  }
}

/// An element of the DOM (an `HTMLElement`) inside a Skene tree, through the native
/// door.
///
/// It is an ordinary node, placed, styled by the layout a wrapper gives it and removed
/// as any other, the element keeping its own look. It is built from a tag and its
/// attributes (`createElement`, `setAttribute`), its [`NativeMarkup`]. The server
/// renders that markup as the view's placeholder, and the browser creates the element
/// from it or, while hydrating, adopts the server's placeholder in place, writing each
/// attribute the markup gives it another value, so the rest of the page hydrates as it
/// would without it.
///
/// On the browser leaf the element is handed, once adopted, to its `handle_adopt`, to
/// draw into or listen on; a view holding a value registers a
/// [`ControlSink`](skene_foundation::ControlSink) of its own there
/// ([`LowLevel::register_sink`]), so a [`Controller`](skene_foundation::Controller) of a
/// [`ContentKey::door`](skene_foundation::ContentKey::door) key controls the value under
/// a built-in's contract.
///
/// Raw CSS reaches a native view through its own `style` attribute.
///
/// ```
/// use skene_foundation::{Container, DecorationBase, HasChild};
/// use skene_gui_web::{NativeView, Web};
///
/// let well = Container::<Web>::new()
///   .set_width(48)
///   .set_height(24)
///   .set_child(
///     NativeView::new("input")
///       .add_attribute("type", "color")
///       .add_attribute("aria-label", "Ink"),
///   );
/// ```
#[doc(alias = "HTMLElement")]
#[must_use]
pub struct NativeView {
  markup: NativeMarkup,
  #[cfg(not(skene_server))]
  adopted: Option<Box<Adopt>>,
}

/// What a [`NativeView`] hands its element to once adopted, with its node.
#[cfg(not(skene_server))]
type Adopt = dyn FnOnce(NodeId, &web_sys::Element);

impl std::fmt::Debug for NativeView {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("NativeView")
      .field("markup", &self.markup)
      .finish_non_exhaustive()
  }
}

impl NativeView {
  /// An element of `tag` with no attributes.
  #[doc(alias = "createElement")]
  #[inline(always)]
  pub const fn new(tag: &'static str) -> Self {
    Self {
      markup: NativeMarkup {
        tag,
        attributes: Vec::new(),
      },
      #[cfg(not(skene_server))]
      adopted: None,
    }
  }

  /// Adds the attribute `name` of `value` to the element's markup, replacing a value
  /// added for `name` before.
  #[doc(alias = "setAttribute")]
  pub fn add_attribute(mut self, name: &'static str, value: impl Into<Cow<'static, str>>) -> Self {
    let value = value.into();
    let attributes = &mut self.markup.attributes;

    match attributes
      .iter_mut()
      .find(|(existing, _)| *existing == name)
    {
      Some((_, existing)) => *existing = value,
      None => attributes.push((name, value)),
    }

    self
  }

  /// Hands the element, once adopted as the view's node, to `adopt`, with the node: to
  /// draw into, listen on, or register the sink of a value it holds.
  #[cfg(not(skene_server))]
  pub fn handle_adopt(mut self, adopt: impl FnOnce(NodeId, &web_sys::Element) + 'static) -> Self {
    self.adopted = Some(Box::new(adopt));

    self
  }

  /// The spec the view's node is adopted as: a box taking the layout a wrapper gives it.
  const fn spec() -> NodeSpec {
    NodeSpec::new(
      None,
      None,
      None,
      NodeFacts::Box(BoxFacts {
        layout: Layout::Document,
        container: None,
        transition_name: None,
        link: false,
        acting: false,
      }),
    )
  }
}

impl AnyKind for NativeView {}

impl skene_foundation::View<Web> for NativeView {
  #[cfg(not(skene_server))]
  fn build(self) -> NodeId {
    let element = Web::served(self.markup.tag).unwrap_or_else(|| {
      crate::browser::document()
        .create_element(self.markup.tag)
        .expect("an element")
    });

    for (name, value) in self.markup.attributes() {
      if element.get_attribute(name).as_deref() != Some(value) {
        let _ = element.set_attribute(name, value);
      }
    }

    let node = LowLevel::<Web>::adopt(&Self::spec(), element.clone());

    if let Some(adopt) = self.adopted {
      adopt(node, &element);
    }

    node
  }

  #[cfg(skene_server)]
  fn build(self) -> NodeId {
    LowLevel::<Web>::adopt(&Self::spec(), self.markup)
  }
}
