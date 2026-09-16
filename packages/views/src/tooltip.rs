//! A tooltip: content shown beside the view it describes while the pointer rests on
//! it or it holds focus.

use std::{cell::Cell, rc::Rc, time::Duration};

use skene_core::{
  AccessProp,
  AnyKind,
  AnyView,
  Color,
  Condition,
  DeclaredBoxStyle,
  Driver,
  IntoView,
  NoChild,
  NodeId,
  Part,
  Placement,
  Scene,
  StyleExtension,
  Timer,
  Transition,
  Value,
  View,
};
use skene_reactive::{Effect, Get, Set, Signal};

use crate::{
  Anchor,
  Overlay,
  Text,
  decoration,
  realize::{Source, describe},
};

/// How long the pointer rests on the described view before a tooltip opens.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// What a tooltip shows: text, or a view.
enum Content<D: Driver> {
  Text(Source<String>),
  View(AnyView<D>),
}

/// A tooltip: content shown in a bubble beside the one view it describes, open while
/// the pointer rests on that view or it holds focus.
///
/// The bubble opens after [a delay](Self::set_delay) on hover and at once on keyboard
/// focus, closes on leave, blur and `Escape`, and never takes focus. A text tooltip is
/// reported as the described view's accessible description; a view tooltip is
/// reported as describing it. The bubble is the [`Part::Bubble`] of the described
/// view, decorated through the setters here, and sits where an anchored overlay's
/// [placement](Self::set_placement) puts it. Holds [`NoChild`] until
/// [`set_child`](Self::set_child) supplies the view it describes.
#[must_use]
pub struct Tooltip<D: Driver, C = NoChild> {
  content: Content<D>,
  text_color: Option<Value<Color, Condition<D::Scalar>>>,
  placement: Placement,
  delay: Duration,
  bubble: Overlay<D>,
  child: C,
}

impl<D: Driver> Tooltip<D> {
  fn with(content: Content<D>) -> Self {
    Self {
      content,
      text_color: None,
      placement: Placement::Top,
      delay: TOOLTIP_DELAY,
      bubble: Overlay::hint(),
      child: NoChild,
    }
  }

  /// A tooltip showing `text`.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self::with(Content::Text(Source::Fixed(text.into())))
  }

  /// A tooltip whose text is recomputed whenever a signal `text` reads changes.
  #[inline(always)]
  pub fn derive(text: impl Fn() -> String + 'static) -> Self {
    Self::with(Content::Text(Source::derive(text)))
  }

  /// A tooltip showing `content`, a view of any kind.
  #[inline(always)]
  pub fn view(content: impl IntoView<D>) -> Self {
    Self::with(Content::View(content.into_view()))
  }

  /// Sets the view the tooltip describes.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> Tooltip<D, AnyView<D>> {
    Tooltip {
      content: self.content,
      text_color: self.text_color,
      placement: self.placement,
      delay: self.delay,
      bubble: self.bubble,
      child: child.into_view(),
    }
  }
}

impl<D: Driver, C> Tooltip<D, C> {
  /// The declared style of the bubble, for a facade's medium-specific setters to write
  /// into.
  #[inline(always)]
  pub const fn bubble_style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    self.bubble.style_mut()
  }

  decoration! { bubble_style_mut }

  /// Sets the color of a text tooltip's text; a view tooltip inks its own.
  #[inline(always)]
  pub fn set_text_color(mut self, color: impl Into<Value<Color, Condition<D::Scalar>>>) -> Self {
    self.text_color = Some(color.into());

    self
  }

  /// Adds a property transition on the bubble, interpolating that property whenever
  /// its value changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<D::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.bubble = self.bubble.set_transition(transition);

    self
  }

  /// Sets where the bubble sits against the described view: above it for `Top`, below
  /// it for `Bottom`.
  #[inline(always)]
  pub const fn set_placement(mut self, placement: Placement) -> Self {
    self.placement = placement;

    self
  }

  /// Sets how long the pointer rests on the described view before the bubble opens;
  /// keyboard focus opens it at once.
  #[inline(always)]
  pub const fn set_delay(mut self, delay: Duration) -> Self {
    self.delay = delay;

    self
  }
}

impl<D: Driver, C> AnyKind for Tooltip<D, C> {}

/// Registers what opens and closes the bubble on the described `node`: a pointer
/// resting on it for `delay` or focus reaching it writes `open`, and the pointer or
/// the focus leaving clears it, cancelling a rest still pending.
fn bind_triggers<D: Driver>(node: NodeId, open: Signal<bool>, delay: Duration) {
  let pending: Rc<Cell<Option<Timer>>> = Rc::new(Cell::new(None));
  let cancel: Rc<dyn Fn()> = Rc::new({
    let pending = Rc::clone(&pending);

    move || {
      if let Some(timer) = pending.take() {
        D::default().cancel(timer);
      }
    }
  });
  let closing = |cancel: &Rc<dyn Fn()>| -> Rc<dyn Fn()> {
    let cancel = Rc::clone(cancel);

    Rc::new(move || {
      cancel();
      open.set(false);
    })
  };

  Scene::<D>::with(|scene| {
    scene.handle_pointer_enter(node, {
      let cancel = Rc::clone(&cancel);

      Rc::new(move || {
        cancel();

        let pending_done = Rc::clone(&pending);
        let timer = D::default().schedule(
          delay,
          Box::new(move || {
            pending_done.set(None);
            open.set(true);
          }),
        );

        pending.set(Some(timer));
      })
    });
    scene.handle_pointer_leave(node, closing(&cancel));
    scene.handle_focus(node, {
      let cancel = Rc::clone(&cancel);

      Rc::new(move || {
        cancel();
        open.set(true);
      })
    });
    scene.handle_blur(node, closing(&cancel));
  });
}

impl<D: Driver> View<D> for Tooltip<D, AnyView<D>> {
  fn build(self) -> NodeId {
    let Self {
      content,
      text_color,
      placement,
      delay,
      bubble,
      child,
    } = self;
    let (start, _) = Scene::<D>::with(Scene::create_segment);
    let node = child.build();
    let open = Signal::opaque(false);
    let anchor = Anchor::new();

    anchor.assign(node);

    let (content, text) = match content {
      Content::Text(Source::Fixed(text)) => {
        let mut run = Text::<D>::new(text.clone());

        if let Some(color) = text_color {
          run = run.set_color(color);
        }

        (run.into_view(), Some(Source::Fixed(text)))
      }
      Content::Text(Source::Derived(compute)) => {
        let mut run = Text::<D>::derive({
          let compute = Rc::clone(&compute);

          move || compute()
        });

        if let Some(color) = text_color {
          run = run.set_color(color);
        }

        (run.into_view(), Some(Source::Derived(compute)))
      }
      Content::View(view) => (view, None),
    };
    let bubble = bubble
      .set_anchor(&anchor)
      .set_placement(placement)
      .derive_open(move || open.get())
      .handle_dismiss(move || open.set(false))
      .set_child(content)
      .build();

    Scene::<D>::with(|scene| {
      scene.set_part(bubble, Part::Bubble);
      scene.insert_in_segment(start, node, None);
      scene.insert_in_segment(start, bubble, None);
    });
    bind_triggers::<D>(node, open, delay);

    match text {
      None => describe::<D>(node, vec![AccessProp::DescribedBy(bubble)]),
      Some(Source::Fixed(text)) => describe::<D>(
        node,
        vec![
          AccessProp::DescribedBy(bubble),
          AccessProp::Description(text),
        ],
      ),
      Some(Source::Derived(compute)) => {
        Effect::render(move |_: Option<()>| {
          describe::<D>(
            node,
            vec![
              AccessProp::DescribedBy(bubble),
              AccessProp::Description(compute()),
            ],
          );
        });
      }
    }

    start
  }
}
