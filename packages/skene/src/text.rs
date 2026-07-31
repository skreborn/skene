// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(any(target_arch = "wasm32", feature = "tui")), allow(dead_code))]

use std::rc::Rc;

use skene_core::{
  Animation,
  AnyView,
  Color,
  Condition,
  IntoView,
  NodeId,
  RunStyle,
  Source,
  Transition,
  View,
};
use skene_reactive::{Context, Scope};

/// The text style of a text node: an optional static-or-reactive [`Source`] per property.
#[derive(Clone, Default)]
#[must_use]
pub struct TextStyle {
  /// The text color.
  pub color: Option<Source<Color>>,
  /// The font size, in logical pixels.
  pub font_size: Option<Source<f32>>,
  /// The font weight.
  pub font_weight: Option<Source<f32>>,
}

impl TextStyle {
  /// Creates an empty text style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.font_size = Some(Source::Static(font_size));

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.font_size = Some(Source::Derived(Rc::new(font_size)));

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.font_weight = Some(Source::Static(font_weight));

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.font_weight = Some(Source::Derived(Rc::new(font_weight)));

    self
  }

  /// Overlays each set property of `over` onto `self`, `over` winning.
  #[inline(always)]
  fn overlay(self, over: Self) -> Self {
    Self {
      color: over.color.or(self.color),
      font_size: over.font_size.or(self.font_size),
      font_weight: over.font_weight.or(self.font_weight),
    }
  }
}

/// A run of text.
#[must_use]
pub struct Text {
  pub(crate) source: Source<String>,
  pub(crate) link: Option<String>,
  pub(crate) style: TextStyle,
  pub(crate) dynamics: RunDynamics,
}

/// A run's declarative dynamics: conditional variants, transitions, animations.
#[derive(Default)]
pub(crate) struct RunDynamics {
  pub(crate) variants: Vec<(Vec<Condition>, RunStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<RunStyle>>,
}

impl Text {
  /// Creates a static text label.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      source: Source::Static(content.into()),
      link: None,
      style: TextStyle::new(),
      dynamics: RunDynamics::default(),
    }
  }

  /// Creates a derived text label.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      source: Source::Derived(Rc::new(content)),
      link: None,
      style: TextStyle::new(),
      dynamics: RunDynamics::default(),
    }
  }

  /// Makes the text a link to the given destination.
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

    self
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.style = self.style.set_color(color);

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.style = self.style.derive_color(color);

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_size(font_size);

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_weight(font_weight);

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: RunStyle) -> Self {
    self.dynamics.variants.push((vec![condition], style));

    self
  }

  /// Adds a conditional style variant, applied while all of `conditions` hold.
  #[inline(always)]
  pub fn set_when_all(
    mut self,
    conditions: impl IntoIterator<Item = Condition>,
    style: RunStyle,
  ) -> Self {
    self
      .dynamics
      .variants
      .push((conditions.into_iter().collect(), style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<RunStyle>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }
}

/// An inline run within a [`RichText`]: text, a group of nested spans styled as a unit, or an
/// arbitrary view embedded among the text.
///
/// Spans flow together and wrap as one line, so styling can vary within it—unlike a [`Text`],
/// which is its own block. A [group](Self::group)'s style cascades to its children; a
/// [view](Self::view) is laid out as an inline-level box among the surrounding runs.
#[must_use]
pub struct Span {
  pub(crate) content: SpanContent,
  pub(crate) link: Option<String>,
  pub(crate) style: TextStyle,
  pub(crate) dynamics: RunDynamics,
}

/// What a [`Span`] renders: a text run, nested spans, or an embedded view.
pub(crate) enum SpanContent {
  Text(Source<String>),
  Group(Vec<Span>),
  View(AnyView),
}

impl Span {
  /// A static run of text.
  #[inline(always)]
  pub fn text(content: impl Into<String>) -> Self {
    Self::with_content(SpanContent::Text(Source::Static(content.into())))
  }

  /// A derived run of text.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::with_content(SpanContent::Text(Source::Derived(Rc::new(content))))
  }

  /// A group of nested spans, styled as a unit—the group's style cascades to its children.
  #[inline(always)]
  pub fn group(children: impl IntoIterator<Item = Span>) -> Self {
    Self::with_content(SpanContent::Group(children.into_iter().collect()))
  }

  /// An arbitrary view embedded inline within the run, laid out as an inline-level box among
  /// the surrounding text.
  #[inline(always)]
  pub fn view(view: impl IntoView) -> Self {
    Self::with_content(SpanContent::View(view.into_view()))
  }

  #[inline(always)]
  fn with_content(content: SpanContent) -> Self {
    Self {
      content,
      link: None,
      style: TextStyle::new(),
      dynamics: RunDynamics::default(),
    }
  }

  /// Makes the run a link to the given destination.
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

    self
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.style = self.style.set_color(color);

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.style = self.style.derive_color(color);

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_size(font_size);

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_weight(font_weight);

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: RunStyle) -> Self {
    self.dynamics.variants.push((vec![condition], style));

    self
  }

  /// Adds a conditional style variant, applied while all of `conditions` hold.
  #[inline(always)]
  pub fn set_when_all(
    mut self,
    conditions: impl IntoIterator<Item = Condition>,
    style: RunStyle,
  ) -> Self {
    self
      .dynamics
      .variants
      .push((conditions.into_iter().collect(), style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<RunStyle>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }
}

/// A line of text built from inline [`Span`]s, which flow together and wrap so styling can vary
/// within one line.
#[must_use]
pub struct RichText {
  pub(crate) spans: Vec<Span>,
}

impl RichText {
  /// Creates rich text from a sequence of spans.
  #[inline(always)]
  pub fn new(spans: impl IntoIterator<Item = Span>) -> Self {
    Self {
      spans: spans.into_iter().collect(),
    }
  }
}

/// Provides `base` as the inherited text style for the spans built in `build`, cascading a
/// group's resolved style to its nested children.
#[inline(always)]
pub(crate) fn provide_group_base(base: TextStyle, build: impl FnOnce()) {
  let mut inherited = Context::get::<InheritedText>().unwrap_or_default();

  inherited.base = base;

  let scope = Scope::current().create_child();

  scope.enter(|| {
    Context::provide(inherited);

    build();
  });
}

/// Overrides the inherited text style for its subtree.
///
/// The base style is inherited by every descendant; the `link` style overrides links
/// within the subtree, layered under a link's own overrides. Each style is a full
/// [`TextStyle`], so it can carry static and derived properties alike. Nested themes
/// accumulate.
#[must_use]
pub struct TextTheme {
  pub(crate) child: AnyView,
  pub(crate) base: TextStyle,
  pub(crate) link: TextStyle,
}

impl TextTheme {
  /// Creates a text theme wrapping `child`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      base: TextStyle::new(),
      link: TextStyle::new(),
    }
  }

  /// Sets the base text style inherited by the subtree.
  #[inline(always)]
  pub fn set_base(mut self, style: TextStyle) -> Self {
    self.base = style;

    self
  }

  /// Overrides the default style of links in the subtree.
  #[inline(always)]
  pub fn set_link(mut self, style: TextStyle) -> Self {
    self.link = style;

    self
  }
}

/// Text styling inherited by a subtree, threaded through the reactive `Context`.
#[derive(Clone, Default)]
struct InheritedText {
  base: TextStyle,
  link: TextStyle,
}

/// Resolves a text node's effective style from the inherited theme (via `Context`) and the
/// node's own style. Platform-agnostic; a platform realizes the result. `is_link` layers the
/// theme's link style under the node's own.
#[inline(always)]
pub(crate) fn resolved_text_style(style: TextStyle, is_link: bool) -> TextStyle {
  let inherited = Context::get::<InheritedText>().unwrap_or_default();
  let mut resolved = inherited.base;

  if is_link {
    resolved = resolved.overlay(inherited.link);
  }

  resolved.overlay(style)
}

impl View for TextTheme {
  fn build(self) -> NodeId {
    let mut inherited = Context::get::<InheritedText>().unwrap_or_default();

    inherited.base = inherited.base.overlay(self.base);
    inherited.link = inherited.link.overlay(self.link);

    let scope = Scope::current().create_child();

    scope.enter(|| {
      Context::provide(inherited);

      self.child.build()
    })
  }
}

#[cfg(any(target_arch = "wasm32", feature = "tui"))]
mod realize {
  use skene_core::{NodeId, RunDisplay, RunProp, RunProps, RunStyle, View};

  use super::{
    RichText,
    RunDynamics,
    Span,
    SpanContent,
    Text,
    TextStyle,
    provide_group_base,
    resolved_text_style,
  };
  use crate::{
    driver,
    realize::{Binders, split_prop},
  };

  /// Realizes a text run from its resolved style, returning the node and that resolved
  /// style—the inherited base a group cascades to its children.
  fn build_run(
    display: RunDisplay,
    link: Option<String>,
    style: TextStyle,
    dynamics: RunDynamics,
  ) -> (NodeId, TextStyle) {
    let resolved = resolved_text_style(style, link.is_some());
    let mut statics = RunStyle::default();
    let mut binders = Binders::new();

    split_prop(
      resolved.color.clone(),
      &mut statics.color,
      &mut binders,
      RunProp::Color,
      driver::update_run,
    );

    split_prop(
      resolved.font_size.clone(),
      &mut statics.font_size,
      &mut binders,
      RunProp::FontSize,
      driver::update_run,
    );

    split_prop(
      resolved.font_weight.clone(),
      &mut statics.font_weight,
      &mut binders,
      RunProp::FontWeight,
      driver::update_run,
    );

    let node = driver::create_run(&RunProps {
      display,
      link,
      style: statics,
      variants: dynamics.variants,
      transitions: dynamics.transitions,
      animations: dynamics.animations,
    });

    for binder in binders {
      binder(node);
    }

    (node, resolved)
  }

  impl View for Text {
    fn build(self) -> NodeId {
      // A block run: its own line, unlike an inline `Span`.
      let (node, _) = build_run(RunDisplay::Block, self.link, self.style, self.dynamics);

      self.source.bind(move |text| driver::set_text(node, &text));

      node
    }
  }

  impl View for Span {
    fn build(self) -> NodeId {
      // A run is inline; a view content box is inline-level so it flows among the text.
      let display = if matches!(self.content, SpanContent::View(_)) {
        RunDisplay::InlineBox
      }
      else {
        RunDisplay::Inline
      };

      let (node, resolved) = build_run(display, self.link, self.style, self.dynamics);

      match self.content {
        SpanContent::Text(source) => {
          source.bind(move |text| driver::set_text(node, &text));
        }
        SpanContent::Group(children) => {
          // The group's resolved style becomes the inherited base for its nested spans.
          provide_group_base(resolved, || {
            for child in children {
              let child = child.build();

              driver::attach(node, child, None);
            }
          });
        }
        SpanContent::View(view) => {
          let child = view.build();

          driver::attach(node, child, None);
        }
      }

      node
    }
  }

  impl View for RichText {
    fn build(self) -> NodeId {
      // A block box; the inline spans inside flow and wrap within it.
      let node = driver::create_group();

      for span in self.spans {
        let child = span.build();

        driver::attach(node, child, None);
      }

      node
    }
  }
}
