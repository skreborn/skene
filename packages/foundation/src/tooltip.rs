//! A tooltip: content shown beside the view it describes while the pointer rests on
//! it or it holds focus.

use std::{cell::Cell, rc::Rc, time::Duration};

use skene_reactive::{Effect, Get, Set, Signal, on_cleanup, untrack};

use crate::{
  AccessProp,
  AnyKind,
  AnyView,
  Color,
  Condition,
  DeclaredBoxStyle,
  Handler,
  NoChild,
  NodeHandle,
  NodeId,
  Overlay,
  Part,
  Placement,
  Platform,
  Scene,
  StyleExtension,
  Text,
  Timer,
  Transition,
  Value,
  View,
  decoration,
  realize::{Source, describe},
};

/// How long the pointer rests on the described view before a tooltip opens.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// How long an open tooltip waits, once the pointer leaves the described view or the
/// bubble, for the pointer to reach either before it closes.
pub const TOOLTIP_GRACE: Duration = Duration::from_millis(300);

/// What a tooltip shows: text, or a view.
enum Content<P: Platform> {
  Text(Source<String>),
  View(AnyView<P>),
}

/// A tooltip: content shown in a bubble beside the one view it describes, open while
/// the pointer rests on that view or it holds focus.
///
/// The bubble opens after [a delay](Self::set_delay) on hover and at once on focus that
/// shows, and never takes focus. It stays open while the pointer rests on the described
/// view or on the bubble, closing [`TOOLTIP_GRACE`] after the pointer leaves both, and
/// closes at once on blur and `Escape`. A text tooltip is reported as the accessible
/// description of the node that takes focus within the described view, the view itself
/// where it does; a view tooltip is reported as describing that node. The bubble is the
/// [`Part::Bubble`] of the described view, decorated through the setters here, and sits where an
/// anchored overlay's [placement](Self::set_placement) puts it. Holds [`NoChild`] until
/// [`set_child`](Self::set_child) supplies the view it describes.
#[must_use]
pub struct Tooltip<P: Platform, C = NoChild> {
  content: Content<P>,
  text_color: Option<Value<Color, Condition<P::Scalar>>>,
  placement: Placement,
  delay: Duration,
  bubble: Overlay<P>,
  child: C,
}

impl<P: Platform, C> std::fmt::Debug for Tooltip<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Tooltip").finish_non_exhaustive()
  }
}

impl<P: Platform> Tooltip<P> {
  fn with(content: Content<P>) -> Self {
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
  pub fn new(text: impl Into<String>) -> Self {
    Self::with(Content::Text(Source::Fixed(text.into())))
  }

  /// A tooltip whose text is recomputed whenever a signal `text` reads changes.
  pub fn derive(text: impl Fn() -> String + 'static) -> Self {
    Self::with(Content::Text(Source::derive(text)))
  }

  /// A tooltip showing `content`, a view of any kind.
  pub fn view(content: impl View<P>) -> Self {
    Self::with(Content::View(content.into_any()))
  }

  /// Sets the view the tooltip describes.
  pub fn set_child(self, child: impl View<P>) -> Tooltip<P, AnyView<P>> {
    Tooltip {
      content: self.content,
      text_color: self.text_color,
      placement: self.placement,
      delay: self.delay,
      bubble: self.bubble,
      child: child.into_any(),
    }
  }
}

impl<P: Platform> Tooltip<P> {
  /// The declared style of the bubble, for a facade's medium-specific setters to write
  /// into.
  #[inline(always)]
  pub const fn bubble_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.bubble.style_mut()
  }

  decoration! { bubble_style_mut, P }

  /// Sets the color of a text tooltip's text; a view tooltip inks its own.
  pub fn set_text_color(mut self, color: impl Into<Value<Color, Condition<P::Scalar>>>) -> Self {
    self.text_color = Some(color.into());

    self
  }

  /// Adds a property transition on the bubble, interpolating that property whenever
  /// its value changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
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
  /// focus that shows opens it at once.
  #[inline(always)]
  pub const fn set_delay(mut self, delay: Duration) -> Self {
    self.delay = delay;

    self
  }
}

impl<P: Platform, C> AnyKind for Tooltip<P, C> {}

/// Registers what opens and closes `bubble`, describing `node`: a pointer resting on
/// the node for `delay` or focus that shows reaching it writes `open`; the pointer
/// leaving the node or the bubble clears it after [`TOOLTIP_GRACE`] unless it reaches
/// either first, and the focus leaving clears it at once. Each cancels what is pending,
/// as the view's disposal does.
fn bind_triggers<P: Platform>(node: NodeId, bubble: NodeId, open: Signal<bool>, delay: Duration) {
  let pending: Rc<Cell<Option<Timer>>> = Rc::new(Cell::new(None));
  let cancel: Rc<dyn Fn()> = Rc::new({
    let pending = Rc::clone(&pending);

    move || {
      if let Some(timer) = pending.take() {
        P::cancel(timer);
      }
    }
  });

  on_cleanup({
    let cancel = Rc::clone(&cancel);

    move || cancel()
  });
  let after = |wait: Duration, value: bool| -> Rc<dyn Fn()> {
    let cancel = Rc::clone(&cancel);
    let pending = Rc::clone(&pending);

    Rc::new(move || {
      cancel();

      let done = Rc::clone(&pending);
      let timer = P::schedule(
        wait,
        Box::new(move || {
          done.set(None);
          open.set(value);
        }),
      );

      pending.set(Some(timer));
    })
  };
  let at_once = |value: bool| -> Rc<dyn Fn()> {
    let cancel = Rc::clone(&cancel);

    Rc::new(move || {
      cancel();
      open.set(value);
    })
  };

  Scene::<P>::with(|scene| {
    scene.handle(node, Handler::PointerEnter(after(delay, true)));
    scene.handle(node, Handler::PointerLeave(after(TOOLTIP_GRACE, false)));
    scene.handle(bubble, Handler::PointerEnter(Rc::clone(&cancel)));
    scene.handle(bubble, Handler::PointerLeave(after(TOOLTIP_GRACE, false)));
    scene.handle(
      node,
      Handler::Focus({
        let open_at_once = at_once(true);

        Rc::new(move || {
          if P::focus_visible() {
            open_at_once();
          }
        })
      }),
    );
    scene.handle(node, Handler::Blur(at_once(false)));
  });
}

impl<P: Platform> View<P> for Tooltip<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let Self {
      content,
      text_color,
      placement,
      delay,
      bubble,
      child,
    } = self;
    let (start, _) = Scene::<P>::create_segment();
    let node = child.build();
    let open = Signal::transient(false);
    let anchor = NodeHandle::<P>::new();

    anchor.assign(node);

    let (content, text) = match content {
      Content::Text(Source::Fixed(text)) => {
        let mut run = Text::<P>::new(text.clone());

        if let Some(color) = text_color {
          run = run.set_color(color);
        }

        (run.into_any(), Some(Source::Fixed(text)))
      }
      Content::Text(Source::Derived(compute)) => {
        let mut run = Text::<P>::derive({
          let compute = Rc::clone(&compute);

          move || compute()
        });

        if let Some(color) = text_color {
          run = run.set_color(color);
        }

        (run.into_any(), Some(Source::Derived(compute)))
      }
      Content::View(view) => (view, None),
    };
    let bubble = bubble
      .set_anchor(anchor)
      .set_placement(placement)
      .derive_open(move || open.get())
      .handle_dismiss(move || open.set(false))
      .set_child(content)
      .build();

    Scene::<P>::with(|scene| {
      scene.set_part(bubble, Part::Bubble);
      scene.insert_in_segment(start, node, None);
      scene.insert_in_segment(start, bubble, None);
    });
    bind_description::<P>(node, bubble, text);
    bind_triggers::<P>(node, bubble, open, delay);

    start
  }
}

/// Describes the node holding focus within `node` by `bubble` and its `text`, where it
/// has one: the node found as the tooltip builds, then the one focus reaches, so a
/// description follows content rebuilt in place.
fn bind_description<P: Platform>(node: NodeId, bubble: NodeId, text: Option<Source<String>>) {
  let report: Rc<dyn Fn() -> Vec<AccessProp>> = match text {
    None => Rc::new(move || vec![AccessProp::DescribedBy(vec![bubble])]),
    Some(Source::Fixed(text)) => Rc::new(move || {
      vec![
        AccessProp::DescribedBy(vec![bubble]),
        AccessProp::Description(text.clone()),
      ]
    }),
    Some(Source::Derived(compute)) => Rc::new(move || {
      vec![
        AccessProp::DescribedBy(vec![bubble]),
        AccessProp::Description(compute()),
      ]
    }),
  };
  let described = Rc::new(Cell::new(Scene::<P>::with(|scene| {
    scene.focus_holder_within(node)
  })));

  Effect::render({
    let described = Rc::clone(&described);
    let report = Rc::clone(&report);

    move |_: Option<()>| describe::<P>(described.get(), report())
  });
  Scene::<P>::with(|scene| {
    scene.handle(
      node,
      Handler::Focus(Rc::new(move || {
        let holder = Scene::<P>::with(|scene| scene.focus_holder_within(node));
        let previous = described.replace(holder);

        if previous != holder {
          describe::<P>(previous, Vec::new());
          describe::<P>(holder, untrack(|| report()));
        }
      })),
    );
  });
}
