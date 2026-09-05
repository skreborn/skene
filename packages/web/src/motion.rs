//! Transitions and keyframe animations as CSS.

use std::fmt::Write as _;

use skene_core::{Animation, Easing, StyleProperty, Transition};

use crate::{Declarations, interner::content_hash};

/// The CSS property name a [`StyleProperty`] transitions.
const fn style_property_name(property: StyleProperty) -> &'static str {
  match property {
    StyleProperty::All => "all",
    StyleProperty::Width => "width",
    StyleProperty::Height => "height",
    StyleProperty::MinWidth => "min-width",
    StyleProperty::MinHeight => "min-height",
    StyleProperty::MaxWidth => "max-width",
    StyleProperty::MaxHeight => "max-height",
    StyleProperty::AspectRatio => "aspect-ratio",
    StyleProperty::Margin => "margin",
    StyleProperty::Padding => "padding",
    StyleProperty::Inset => "inset",
    StyleProperty::Background => "background-color",
    StyleProperty::BorderColor => "border-color",
    StyleProperty::BorderWidth => "border-width",
    StyleProperty::BorderRadius => "border-radius",
    StyleProperty::Spacing => "gap",
    StyleProperty::Flex => "flex",
    StyleProperty::TextColor => "color",
    StyleProperty::FontSize => "font-size",
    StyleProperty::FontWeight => "font-weight",
  }
}

/// A timing curve's CSS text.
fn render_easing(easing: Easing) -> String {
  match easing {
    Easing::Linear => "linear".to_owned(),
    Easing::Ease => "ease".to_owned(),
    Easing::EaseIn => "ease-in".to_owned(),
    Easing::EaseOut => "ease-out".to_owned(),
    Easing::EaseInOut => "ease-in-out".to_owned(),
    Easing::CubicBezier(x1, y1, x2, y2) => format!("cubic-bezier({x1}, {y1}, {x2}, {y2})"),
  }
}

/// The `transition` declaration value for a set of transitions.
pub(crate) fn render_transitions(transitions: &[Transition]) -> String {
  let mut out = String::new();

  for (index, transition) in transitions.iter().enumerate() {
    if index > 0 {
      out.push_str(", ");
    }

    out.push_str(style_property_name(transition.property));

    let _ = write!(
      out,
      " {}s {} {}s",
      transition.duration,
      render_easing(transition.easing),
      transition.delay,
    );
  }

  out
}

/// Appends one animation's `@keyframes` block, named by the hash of its body so the name
/// is identical on every leaf, and returns that name.
pub(crate) fn render_keyframes<S>(
  out: &mut String,
  animation: &Animation<S>,
  declarations: &impl Fn(&S) -> Declarations,
) -> String {
  let mut body = String::new();

  for (percent, style) in &animation.keyframes {
    let _ = write!(body, "{percent}%");

    body.push('{');
    body.push_str(&declarations(style).render());
    body.push('}');
  }

  let name = format!("skene-k-{:032x}", content_hash(&body));

  out.push_str("@keyframes ");
  out.push_str(&name);
  out.push('{');
  out.push_str(&body);
  out.push('}');

  name
}

/// The `animation` declaration value for a set of animations and their keyframes names.
pub(crate) fn render_animation_values<S>(animations: &[Animation<S>], names: &[String]) -> String {
  let mut out = String::new();

  for (index, animation) in animations.iter().enumerate() {
    if index > 0 {
      out.push_str(", ");
    }

    let _ = write!(
      out,
      "{} {}s {} {}s {} {} both",
      names[index],
      animation.duration,
      render_easing(animation.easing),
      animation.delay,
      match animation.iterations {
        Some(count) => count.to_string(),
        None => "infinite".to_owned(),
      },
      if animation.alternate {
        "alternate"
      }
      else {
        "normal"
      },
    );
  }

  out
}
