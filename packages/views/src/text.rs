//! A text run.

use std::rc::Rc;

use skene_core::{
  Color,
  LinkPolicy,
  RunDisplay,
  next::{
    AnyKind,
    Condition,
    DeclaredRunStyle,
    Driver,
    Leaf,
    NodeId,
    NodeKind,
    RunDynamics,
    RunProps,
    Scene,
    Value,
    View,
  },
};
use skene_reactive::Effect;

use crate::realize::{bind_run_slot, with_sink};

/// A run's text: fixed, or recomputed from the signals it reads.
enum Content {
  Fixed(String),
  Derived(Box<dyn Fn() -> String>),
}

/// A run of text on its own line.
#[must_use]
pub struct Text<D: Driver> {
  content: Content,
  style: DeclaredRunStyle<D>,
}

impl<D: Driver> Text<D> {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: Content::Fixed(content.into()),
      style: DeclaredRunStyle::<D>::default(),
    }
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      content: Content::Derived(Box::new(content)),
      style: DeclaredRunStyle::<D>::default(),
    }
  }

  /// Sets the text color.
  #[inline(always)]
  pub fn set_color(mut self, color: impl Into<Value<Color, Condition<D::Scalar>>>) -> Self {
    self.style.color = Some(color.into());

    self
  }

  /// Derives the text color from the signals `color` reads.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.style.color = Some(Value::new(Leaf::Derived(Rc::new(color))));

    self
  }
}

impl<D: Driver> AnyKind for Text<D> {}

impl<D: Driver> View<D> for Text<D> {
  fn build(self) -> NodeId {
    let (style, slots) = self.style.split();
    let props = RunProps {
      display: RunDisplay::Block,
      selectable: false,
      link: None,
      link_policy: LinkPolicy::new(),
      style,
      dynamics: RunDynamics::default(),
    };
    let driver = D::default();
    let sink = driver.create_run(&props);
    let node = Scene::<D>::with(|scene| scene.create(NodeKind::Run, sink));

    for slot in slots {
      bind_run_slot::<D>(node, slot);
    }

    match self.content {
      Content::Fixed(text) => {
        with_sink::<D, _>(node, |sink| driver.set_text(sink, &text));
      }
      Content::Derived(content) => {
        Effect::render(move |_: Option<()>| {
          let text = content();

          with_sink::<D, _>(node, |sink| driver.set_text(sink, &text));
        });
      }
    }

    node
  }
}
