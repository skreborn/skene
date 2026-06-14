//! Translation of the abstract [`Style`] into CSS.
//!
//! This is the *only* place in the project that knows about CSS. The core and
//! the renderer abstraction deal purely in semantic style properties; the DOM
//! backend turns those into a CSS declaration block here.
//!
//! Declarations are emitted in a fixed order so that the generated text — and
//! therefore its hash and class name — is deterministic. That determinism is
//! what makes the [`stylesheet`](crate::stylesheet) store server-side-rendering
//! friendly: the same `Style` always yields the same class.

use skene_renderer::{
  Axis,
  Color,
  CrossAxisAlignment,
  Cursor,
  Display,
  Length,
  MainAxisAlignment,
  Style,
};

fn length(l: Length) -> String {
  match l {
    Length::Px(v) => format!("{v}px"),
    Length::Pct(v) => format!("{v}%"),
    Length::Auto => "auto".to_string(),
  }
}

fn color(c: Color) -> String {
  if c.a == 255 {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
  }
  else {
    format!("rgba({},{},{},{})", c.r, c.g, c.b, f64::from(c.a) / 255.0)
  }
}

fn display(d: Display) -> &'static str {
  match d {
    Display::Flex => "flex",
    Display::Block => "block",
    Display::None => "none",
  }
}

fn flex_direction(d: Axis) -> &'static str {
  match d {
    Axis::Horizontal => "row",
    Axis::Vertical => "column",
  }
}

fn justify_content(j: MainAxisAlignment) -> &'static str {
  match j {
    MainAxisAlignment::Start => "flex-start",
    MainAxisAlignment::Center => "center",
    MainAxisAlignment::End => "flex-end",
    MainAxisAlignment::SpaceBetween => "space-between",
    MainAxisAlignment::SpaceAround => "space-around",
    MainAxisAlignment::SpaceEvenly => "space-evenly",
  }
}

fn align_items(a: CrossAxisAlignment) -> &'static str {
  match a {
    CrossAxisAlignment::Start => "flex-start",
    CrossAxisAlignment::Center => "center",
    CrossAxisAlignment::End => "flex-end",
    CrossAxisAlignment::Stretch => "stretch",
    CrossAxisAlignment::Baseline => "baseline",
  }
}

fn cursor(c: Cursor) -> &'static str {
  match c {
    Cursor::Default => "default",
    Cursor::Pointer => "pointer",
    Cursor::Text => "text",
  }
}

/// Build the CSS declaration block (without surrounding braces) for a style.
pub fn declarations(style: &Style) -> String {
  let mut out = String::new();
  {
    let mut push = |prop: &str, value: String| {
      out.push_str(prop);
      out.push(':');
      out.push_str(&value);
      out.push(';');
    };

    if let Some(v) = style.display {
      push("display", display(v).to_string());
    }
    if let Some(v) = style.flex_direction {
      push("flex-direction", flex_direction(v).to_string());
    }
    if let Some(v) = style.main_axis_align {
      push("justify-content", justify_content(v).to_string());
    }
    if let Some(v) = style.cross_axis_align {
      push("align-items", align_items(v).to_string());
    }
    if let Some(v) = style.gap {
      push("gap", length(v));
    }
    if let Some(v) = style.flex_grow {
      push("flex-grow", v.to_string());
    }
    if let Some(v) = style.flex_shrink {
      push("flex-shrink", v.to_string());
    }
    if let Some(v) = style.flex_basis {
      push("flex-basis", length(v));
    }

    if let Some(v) = style.width {
      push("width", length(v));
    }
    if let Some(v) = style.height {
      push("height", length(v));
    }
    if let Some(v) = style.min_width {
      push("min-width", length(v));
    }
    if let Some(v) = style.min_height {
      push("min-height", length(v));
    }
    if let Some(v) = style.max_width {
      push("max-width", length(v));
    }
    if let Some(v) = style.max_height {
      push("max-height", length(v));
    }

    if let Some(v) = style.padding_top {
      push("padding-top", length(v));
    }
    if let Some(v) = style.padding_right {
      push("padding-right", length(v));
    }
    if let Some(v) = style.padding_bottom {
      push("padding-bottom", length(v));
    }
    if let Some(v) = style.padding_left {
      push("padding-left", length(v));
    }

    if let Some(v) = style.background {
      push("background-color", color(v));
    }
    if let Some(v) = style.color {
      push("color", color(v));
    }
    if let Some(v) = style.font_size {
      push("font-size", length(v));
    }
    if let Some(v) = style.font_weight {
      push("font-weight", v.to_string());
    }

    if let Some(v) = style.border_width {
      push("border-width", length(v));
      push("border-style", "solid".to_string());
    }
    if let Some(v) = style.border_color {
      push("border-color", color(v));
    }
    if let Some(v) = style.border_radius {
      push("border-radius", length(v));
    }

    if let Some(v) = style.cursor {
      push("cursor", cursor(v).to_string());
    }
  }
  out
}
