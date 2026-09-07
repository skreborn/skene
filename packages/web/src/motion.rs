//! Transitions and keyframe animations as CSS.

use std::fmt::Write as _;

use skene_core::{Animation, Easing, PropertyKey, StyleProperty, Transition};
use skene_gui_core::GuiProperty;

use crate::{
  Declarations,
  compile::{Key, target},
  interner::content_hash,
};

/// The media query under which motion respecting the reduced-motion preference runs.
pub(crate) const NO_PREFERENCE: &str = "(prefers-reduced-motion: no-preference)";

/// The rule opting a document into cross-document view transitions.
#[cfg(target_arch = "wasm32")]
pub(crate) const VIEW_TRANSITION_RULE: &str = "@view-transition{navigation:auto}";

/// The CSS property name a transition targets.
fn style_property_name(property: StyleProperty<GuiProperty>) -> &'static str {
  let key: Key = match property {
    StyleProperty::All => return "all",
    StyleProperty::Shared(property) => PropertyKey::Shared(property),
    StyleProperty::Extension(property) => PropertyKey::Extension(property),
  };

  target(key).name
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
pub(crate) fn render_transitions<'a>(
  transitions: impl IntoIterator<Item = &'a Transition<GuiProperty>>,
) -> String {
  let mut out = String::new();

  for (index, transition) in transitions.into_iter().enumerate() {
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
pub(crate) fn render_animation_values<'a, S: 'a>(
  animations: impl IntoIterator<Item = (&'a Animation<S>, &'a str)>,
) -> String {
  let mut out = String::new();

  for (index, (animation, name)) in animations.into_iter().enumerate() {
    if index > 0 {
      out.push_str(", ");
    }

    let _ = write!(
      out,
      "{} {}s {} {}s {} {} both",
      name,
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
