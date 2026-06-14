//! The built-in views. Deliberately a small set for the web skeleton; more can
//! be layered on top of [`El`] without touching the backend abstraction.

use std::{cell::RefCell, rc::Rc};

use skene_reactive::{Effect, Scope};
use skene_renderer::{ElementKind, FlexDirection, NodeId, Style};

use crate::{
  element::El,
  runtime::with_backend,
  view::{AnyView, IntoView, View},
};

/// Static text.
pub fn text(content: impl Into<String>) -> El {
  El::new(ElementKind::Text).text(content)
}

/// Reactive text: re-rendered whenever signals read inside `f` change.
pub fn label(f: impl Fn() -> String + 'static) -> El {
  El::new(ElementKind::Text).text_fn(f)
}

/// A single-child container box.
pub fn container(child: impl IntoView) -> El {
  El::new(ElementKind::Box).child(child)
}

/// An empty box.
pub fn empty() -> El {
  El::new(ElementKind::Box)
}

/// A box holding several children (a flex container by default).
pub fn flex(children: impl Children) -> El {
  El::new(ElementKind::Box).children(children.into_views())
}

/// A vertical (column) flex.
pub fn column(children: impl Children) -> El {
  flex(children).style(Style::new().flex_direction(FlexDirection::Column))
}

/// A horizontal (row) flex.
pub fn row(children: impl Children) -> El {
  flex(children).style(Style::new().flex_direction(FlexDirection::Row))
}

/// A button. Chain [`El::on_click`] to handle presses.
pub fn button(child: impl IntoView) -> El {
  El::new(ElementKind::Button).child(child)
}

/// A reactive subtree: `f` is re-run whenever signals it reads change, and the
/// previously built subtree (and everything its scope owns) is disposed.
pub fn dyn_view(f: impl Fn() -> AnyView + 'static) -> DynView {
  DynView { f: Box::new(f) }
}

pub struct DynView {
  f: Box<dyn Fn() -> AnyView>,
}

impl View for DynView {
  fn build(self) -> NodeId {
    let container = with_backend(|b| b.create_element(ElementKind::Box));
    let f = self.f;
    let parent_scope = Scope::current();
    // Holds the currently mounted child node and the scope that owns its effects.
    let current: Rc<RefCell<Option<(NodeId, Scope)>>> = Rc::new(RefCell::new(None));

    Effect::new(move |_| {
      let child_scope = parent_scope.create_child();
      let new_node = child_scope.enter(|| (f)().build());

      let previous = current.borrow_mut().take();
      with_backend(|b| {
        b.insert_child(container, new_node, None);
        if let Some((old_node, _)) = &previous {
          b.remove_child(container, *old_node);
          b.destroy(*old_node);
        }
      });
      if let Some((_, old_scope)) = previous {
        old_scope.dispose();
      }
      *current.borrow_mut() = Some((new_node, child_scope));
    });

    container
  }
}

/// Conversion for the heterogeneous child collections accepted by [`flex`].
pub trait Children {
  fn into_views(self) -> Vec<AnyView>;
}

impl Children for Vec<AnyView> {
  fn into_views(self) -> Vec<AnyView> {
    self
  }
}

macro_rules! impl_children_tuple {
    ($($t:ident),+) => {
        impl<$($t: IntoView),+> Children for ($($t,)+) {
            fn into_views(self) -> Vec<AnyView> {
                #[allow(non_snake_case)]
                let ($($t,)+) = self;
                vec![$($t.into_view()),+]
            }
        }
    };
}

impl_children_tuple!(A);
impl_children_tuple!(A, B);
impl_children_tuple!(A, B, C);
impl_children_tuple!(A, B, C, D);
impl_children_tuple!(A, B, C, D, E);
impl_children_tuple!(A, B, C, D, E, F);
impl_children_tuple!(A, B, C, D, E, F, G);
impl_children_tuple!(A, B, C, D, E, F, G, H);
