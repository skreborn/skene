//! The surfaces several views share: a box's decoration and sizing, the run of text it holds
//! or its texts inherit, the colors its text entries draw in, and its motion.

use crate::{
  Border, Color, Corners, Dimension, Fill, FontFamily, FontStyle, FontWeight, Inset, Offset, Ring,
  Shadows,
  seam::record::{Animation, Transition},
  view::{HasRecord, Wraps, trait_properties},
};

/// A box's decoration and sizing: what a `Container` and every control's own box carry.
pub trait DecorationBase: Wraps<Inner: HasRecord> {
  trait_properties! {
    /// Space between the box's edges and what it holds, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// The fill behind what the box holds: a color or a gradient.
    set_background, derive_background: BACKGROUND, Fill;
    /// The corners' radii, in pixels.
    set_corner_radius, derive_corner_radius: CORNER_RADIUS, Corners;
    /// A line around the box, inside its edges.
    set_border, derive_border: BORDER, Border;
    /// The fill of the line around the box (a color or a gradient), its text color where
    /// unset.
    set_border_color, derive_border_color: BORDER_COLOR, Fill;
    /// The shadows the box casts.
    set_shadow, derive_shadow: SHADOW, Shadows;
    /// The box's opacity, 0 to 1.
    set_opacity, derive_opacity: OPACITY, f32;
    /// A line around the box that takes no room and is never hit, as a focus ring:
    /// `Ring::NONE.when(State::FocusVisible, Ring::new(2))`.
    set_ring, derive_ring: RING, Ring;
    /// The ring's color, the box's text color where unset.
    set_ring_color, derive_ring_color: RING_COLOR, Color;
    /// How far the box and what it holds are drawn from where they were laid out, taking no
    /// room: in pixels, or in fractions of the box's own size (`Offset::of_size(1., 0.)`).
    set_offset, derive_offset: OFFSET, Offset;
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
    /// The least width.
    set_min_width, derive_min_width: MIN_WIDTH, Dimension;
    /// The least height.
    set_min_height, derive_min_height: MIN_HEIGHT, Dimension;
    /// The greatest width.
    set_max_width, derive_max_width: MAX_WIDTH, Dimension;
    /// The greatest height.
    set_max_height, derive_max_height: MAX_HEIGHT, Dimension;
  }
}

/// The run of text a view draws, or the one every text beneath it inherits.
pub trait RunBase: Wraps<Inner: HasRecord> {
  trait_properties! {
    /// The text's color: what every text and mark beneath inherits.
    set_color, derive_color: COLOR, Color;
    /// The font's size, in pixels.
    set_font_size, derive_font_size: FONT_SIZE, f32;
    /// The font's weight.
    set_font_weight, derive_font_weight: FONT_WEIGHT, FontWeight;
    /// Upright or slanted.
    set_font_style, derive_font_style: FONT_STYLE, FontStyle;
    /// The typeface.
    set_font_family, derive_font_family: FONT_FAMILY, FontFamily;
    /// The height of a line, as a multiple of the font's size.
    set_line_height, derive_line_height: LINE_HEIGHT, f32;
    /// Extra space between letters, in pixels.
    set_letter_spacing, derive_letter_spacing: LETTER_SPACING, f32;
  }
}

/// The colors the text entries inside a box draw their caret, selection and placeholder in,
/// which every entry beneath inherits.
pub trait RunWithEntries: RunBase {
  trait_properties! {
    /// A text entry's caret, the text's color where unset.
    set_caret_color, derive_caret_color: CARET_COLOR, Color;
    /// The fill behind a text entry's selection.
    set_selection_color, derive_selection_color: SELECTION_COLOR, Color;
    /// An empty text entry's placeholder.
    set_placeholder_color, derive_placeholder_color: PLACEHOLDER_COLOR, Color;
  }
}

/// How a view's own look moves: its transitions and its keyframe animations.
pub trait MotionBase: Wraps<Inner: HasRecord> {
  /// Animates changes of a property of the view's own look as `transition` says
  /// (`Transition::new(keys::BACKGROUND, 200)`); one of the same property is replaced.
  fn add_transition(mut self, transition: Transition) -> Self {
    self.inner_mut().record_mut().add_transition(transition);

    self
  }

  /// Plays `animation`'s keyframes over the view's own look from when it first takes it,
  /// after the animations added before:
  /// `Animation::new(400).add_keyframe(0., Keyframe::new().set(keys::OPACITY, 0))`.
  fn add_animation(mut self, animation: Animation) -> Self {
    self.inner_mut().record_mut().add_animation(animation);

    self
  }
}
