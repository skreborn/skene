//! The builder traits views share: the decoration of a box, its motion and the run of a
//! text, over what each view holds.

use crate::{
  Animation,
  AspectRatio,
  Axes,
  BoxKeyframe,
  Color,
  Condition,
  CornerRadius,
  Declared,
  DeclaredBoxStyle,
  DeclaredRunStyle,
  Dimension,
  Inset,
  LayoutBox,
  Leaf,
  Length,
  LinkPolicy,
  MouseCursor,
  Overflow,
  Platform,
  Run,
  RunKeyframe,
  StaticLeaf,
  StyleExtension,
  TextDecoration,
  TextDecorationStyle,
  Transition,
  TypeMetrics,
  Value,
  Wraps,
};

/// A view whose box takes a declared style, which the decoration of a box writes into.
pub trait HasBoxStyle<P: Platform> {
  /// The declared style.
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P>;
}

impl<P: Platform, T: Wraps<Inner: HasBoxStyle<P>>> HasBoxStyle<P> for T {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner_mut().box_style_mut()
  }
}

/// A view over a layout box, which the members a box carries reach through.
pub trait HasLayoutBox<P: Platform>: Sized {
  /// The child kind the layout box accepts.
  type Kind: 'static;

  /// The view with its layout box replaced by what `map` makes of it.
  #[must_use]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<P, Self::Kind>) -> LayoutBox<P, Self::Kind>,
  ) -> Self;
}

impl<P: Platform, T: Wraps<Inner: HasLayoutBox<P>>> HasLayoutBox<P> for T {
  type Kind = <T::Inner as HasLayoutBox<P>>::Kind;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<P, Self::Kind>) -> LayoutBox<P, Self::Kind>,
  ) -> Self {
    Self::wrap(self.into_inner().map_layout_box(map))
  }
}

/// A text view over a run, which the members every text view shares reach through.
pub trait HasRun<P: Platform>: Sized {
  /// The run's declared style.
  fn run_style_mut(&mut self) -> &mut DeclaredRunStyle<P>;

  /// The view with its run replaced by what `map` makes of it.
  #[must_use]
  fn map_run(self, map: impl FnOnce(Run<P>) -> Run<P>) -> Self;
}

impl<P: Platform, T: Wraps<Inner: HasRun<P>>> HasRun<P> for T {
  #[inline(always)]
  fn run_style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    self.inner_mut().run_style_mut()
  }

  #[inline(always)]
  fn map_run(self, map: impl FnOnce(Run<P>) -> Run<P>) -> Self {
    Self::wrap(self.into_inner().map_run(map))
  }
}

/// The decoration and sizing of a box.
///
/// Its size, its insets, its colors, its border, its cursor, its overflow and whether it
/// is hidden.
pub trait DecorationBase<P: Platform>: HasBoxStyle<P> + Sized {
  crate::trait_properties! { box_style_mut, Condition<P::Scalar>;
    set_width / derive_width (width) => width: Dimension<P::Scalar>,
      "the width";
    set_height / derive_height (height) => height: Dimension<P::Scalar>,
      "the height";
    set_min_width / derive_min_width (min_width) => min_width: Dimension<P::Scalar>,
      "the minimum width";
    set_min_height / derive_min_height (min_height) => min_height: Dimension<P::Scalar>,
      "the minimum height";
    set_max_width / derive_max_width (max_width) => max_width: Dimension<P::Scalar>,
      "the maximum width";
    set_max_height / derive_max_height (max_height) => max_height: Dimension<P::Scalar>,
      "the maximum height";
    set_aspect_ratio / derive_aspect_ratio (aspect_ratio) => aspect_ratio: AspectRatio,
      "the width-to-height ratio the unsized axis follows";
    set_padding / derive_padding (padding) => padding: Inset<P::Scalar>,
      "the padding on each edge";
    set_margin / derive_margin (margin) => margin: Inset<Dimension<P::Scalar>>,
      "the margin on each edge";
    set_offset / derive_offset (offset) => offset: Inset<Dimension<P::Scalar>>,
      "the shift from each edge, keeping the box in flow";
    set_color / derive_color (color) => color: Color,
      "the background color";
    set_border_width / derive_border_width (width) => border_width: Inset<P::Scalar>,
      "the border's width on each edge; zero draws none";
    set_border_color / derive_border_color (color) => border_color: Color,
      "the border's color";
    set_cursor / derive_cursor (cursor) => cursor: MouseCursor,
      "the mouse cursor shown over the box";
    set_overflow / derive_overflow (overflow) => overflow: Axes<Overflow>,
      "how content exceeding the box's bounds is treated on each axis",
      "`Scroll` and `Auto` make an axis scrollable, and one overflow sets both.";
    set_hidden / derive_hidden (hidden) => hidden: bool,
      "whether the box and its content go undrawn",
      "A hidden box keeps its place in the layout.";
  }
}

/// The corner radius of a decorated box, where the platform's style rounds corners.
pub trait DecorationWithCornerRadius<P: Platform>: DecorationBase<P>
where
  P::StyleExt: CornerRadius,
{
  /// Sets the corner radius, a length in the medium's unit.
  #[must_use]
  fn set_corner_radius(mut self, radius: impl Into<Value<Length, Condition<P::Scalar>>>) -> Self {
    <P::StyleExt as CornerRadius>::set_corner_radius::<Declared<Condition<P::Scalar>>>(
      &mut self.box_style_mut().ext,
      Some(radius.into()),
    );

    self
  }

  /// Derives the corner radius from the signals `radius` reads: a value, a token or an
  /// expression.
  #[must_use]
  fn derive_corner_radius<R: Into<StaticLeaf<Length>>>(
    mut self,
    radius: impl Fn() -> R + 'static,
  ) -> Self {
    <P::StyleExt as CornerRadius>::set_corner_radius::<Declared<Condition<P::Scalar>>>(
      &mut self.box_style_mut().ext,
      Some(Value::new(Leaf::derive(radius))),
    );

    self
  }
}

impl<P: Platform, T: DecorationBase<P>> DecorationWithCornerRadius<P> for T where
  P::StyleExt: CornerRadius
{
}

/// The motion of a box.
///
/// The transitions of its properties, its keyframe animations and its name across a
/// navigation.
pub trait MotionBase<P: Platform>: HasLayoutBox<P> {
  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[must_use]
  fn set_transition(
    self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.map_layout_box(|layout| layout.set_transition(transition))
  }

  /// Adds a keyframe animation.
  #[must_use]
  fn set_animation(self, animation: Animation<BoxKeyframe<P>>) -> Self {
    self.map_layout_box(|layout| layout.set_animation(animation))
  }

  /// Names the box for view transitions: it is paired with the box of the same name
  /// across a navigation, where the platform animates between them.
  #[must_use]
  fn set_transition_name(self, name: impl Into<String>) -> Self {
    self.map_layout_box(|layout| layout.set_transition_name(name))
  }
}

/// What every text view declares of its run.
///
/// Its colors, its decoration, whether it is selectable, its link and its motion.
pub trait RunBase<P: Platform>: HasRun<P> {
  crate::trait_properties! { run_style_mut, Condition<P::Scalar>;
    set_color / derive_color (color) => color: Color,
      "the text color";
    set_selection_color / derive_selection_color (color) => selection_color: Color,
      "the text color while selected";
    set_selection_background / derive_selection_background (color) => selection_background: Color,
      "the background behind selected text";
    set_decoration / derive_decoration (decoration) => decoration: TextDecoration,
      "the line the text is decorated with: beneath, above or through it; a link is underlined unless set";
    set_decoration_style / derive_decoration_style (style) => decoration_style: TextDecorationStyle,
      "how the decoration line is drawn";
    set_decoration_color / derive_decoration_color (color) => decoration_color: Color,
      "the decoration line's color; the text color unless set";
  }

  /// Sets whether the reader can select and copy the text; off by default.
  #[must_use]
  fn set_selectable(self, selectable: bool) -> Self {
    self.map_run(|run| run.set_selectable(selectable))
  }

  /// Makes the text a link to `destination`, inked in the link color unless styled and
  /// opening per its [`LinkPolicy`]; a destination that would run script when followed,
  /// a `javascript:` URL, is never followed.
  #[must_use]
  fn set_link(self, destination: impl Into<String>) -> Self {
    self.map_run(|run| run.set_link(destination))
  }

  /// Sets how the link opens and what it discloses.
  #[must_use]
  fn set_link_policy(self, link_policy: LinkPolicy) -> Self {
    self.map_run(|run| run.set_link_policy(link_policy))
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[must_use]
  fn set_transition(
    self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.map_run(|run| run.set_transition(transition))
  }

  /// Adds a keyframe animation.
  #[must_use]
  fn set_animation(self, animation: Animation<RunKeyframe<P>>) -> Self {
    self.map_run(|run| run.set_animation(animation))
  }
}

/// Generates a provided `set_`/`derive_` pair per type metric on a trait over a run,
/// each writing through the platform's [`TypeMetrics`].
macro_rules! type_metric_pairs {
  ($($set:ident / $derive:ident ($param:ident: $value:ty) => $write:ident, $doc:literal;)+) => {$(
    #[doc = concat!("Sets ", $doc, ".")]
    #[must_use]
    fn $set(mut self, $param: impl Into<Value<$value, Condition<P::Scalar>>>) -> Self {
      <P::StyleExt as TypeMetrics>::$write::<Declared<Condition<P::Scalar>>>(
        &mut self.run_style_mut().ext,
        Some($param.into()),
      );

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, a token or an expression.")]
    #[must_use]
    fn $derive<R: Into<StaticLeaf<$value>>>(mut self, $param: impl Fn() -> R + 'static) -> Self {
      <P::StyleExt as TypeMetrics>::$write::<Declared<Condition<P::Scalar>>>(
        &mut self.run_style_mut().ext,
        Some(Value::new(Leaf::derive($param))),
      );

      self
    }
  )+};
}

/// The type metrics of a run, where the platform's style sizes type.
///
/// Its font size, its weight, its line height and its letter spacing.
pub trait RunWithTypeMetrics<P: Platform>: RunBase<P>
where
  P::StyleExt: TypeMetrics,
{
  type_metric_pairs! {
    set_font_size / derive_font_size (size: Length) => set_font_size,
      "the font size, a length in the medium's unit";
    set_font_weight / derive_font_weight (weight: f32) => set_font_weight,
      "the font weight";
    set_line_height / derive_line_height (height: f32) => set_line_height,
      "the line height, as a multiple of the font size";
    set_letter_spacing / derive_letter_spacing (spacing: Length) => set_letter_spacing,
      "the extra space between letters, a length in the medium's unit";
  }
}

impl<P: Platform, T: RunBase<P>> RunWithTypeMetrics<P> for T where P::StyleExt: TypeMetrics {}
