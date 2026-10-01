//! Expressions as CSS math and color functions.

use skene_core::{Edge, ExprNode};

use crate::declarations::Declarations;

/// The CSS text of an expression: a literal, `var()`, a math function or a color
/// function.
pub(crate) fn render_expr(node: &ExprNode) -> String {
  match node {
    ExprNode::Add(..) | ExprNode::Sub(..) | ExprNode::Mul(..) | ExprNode::Div(..) => {
      format!("calc({})", sum(node))
    }
    _ => operand(node),
  }
}

/// An operand inside a math function: a literal, a reference, a nested function, or a
/// parenthesized sum.
fn operand(node: &ExprNode) -> String {
  match node {
    ExprNode::Number(value) => value.to_string(),
    ExprNode::Length(value) => format!("{value}px"),
    ExprNode::ViewportWidth(percent) => format!("{percent}vw"),
    ExprNode::ViewportHeight(percent) => format!("{percent}vh"),
    ExprNode::ContainerWidth(percent) => format!("{percent}cqw"),
    ExprNode::ContainerHeight(percent) => format!("{percent}cqh"),
    ExprNode::Em(multiple) => format!("{multiple}em"),
    ExprNode::SafeArea(edge) => format!(
      "env(safe-area-inset-{}, 0px)",
      match edge {
        Edge::Top => "top",
        Edge::Right => "right",
        Edge::Bottom => "bottom",
        Edge::Left => "left",
      }
    ),
    ExprNode::Token(name) => format!("var(--{name})"),
    ExprNode::Color(color) => Declarations::render_color(*color),
    ExprNode::Add(..) | ExprNode::Sub(..) | ExprNode::Mul(..) | ExprNode::Div(..) => {
      format!("({})", sum(node))
    }
    ExprNode::Min(lhs, rhs) => format!("min({}, {})", sum(lhs), sum(rhs)),
    ExprNode::Max(lhs, rhs) => format!("max({}, {})", sum(lhs), sum(rhs)),
    ExprNode::Clamp(value, lower, upper) => {
      format!("clamp({}, {}, {})", sum(lower), sum(value), sum(upper))
    }
    ExprNode::Mix(lhs, rhs, amount) => format!(
      "color-mix(in oklab, {}, {} {}%)",
      operand(lhs),
      operand(rhs),
      percent(*amount),
    ),
    ExprNode::WithAlpha(color, alpha) => {
      format!("oklch(from {} l c h / {alpha})", operand(color))
    }
    ExprNode::Lighter(color, amount) => {
      format!(
        "oklch(from {} calc(l + {amount}) c h / alpha)",
        operand(color)
      )
    }
    ExprNode::Darker(color, amount) => {
      format!(
        "oklch(from {} calc(l - {amount}) c h / alpha)",
        operand(color)
      )
    }
    ExprNode::WithChroma(color, chroma) => {
      format!("oklch(from {} l {chroma} h / alpha)", operand(color))
    }
    ExprNode::WithHue(color, hue) => format!("oklch(from {} l c {hue} / alpha)", operand(color)),
  }
}

/// A calc-sum: the arithmetic of a node without the enclosing `calc()`, or its operand.
fn sum(node: &ExprNode) -> String {
  match node {
    ExprNode::Add(lhs, rhs) => format!("{} + {}", operand(lhs), operand(rhs)),
    ExprNode::Sub(lhs, rhs) => format!("{} - {}", operand(lhs), operand(rhs)),
    ExprNode::Mul(lhs, rhs) => format!("{} * {}", operand(lhs), operand(rhs)),
    ExprNode::Div(lhs, rhs) => format!("{} / {}", operand(lhs), operand(rhs)),
    _ => operand(node),
  }
}

/// A fraction (`0.` to `1.`) as a percentage number, narrowed so `0.1` reads `10`.
fn percent(value: f32) -> f32 {
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a fraction's percentage is precise enough in f32"
  )]
  let percent = (f64::from(value) * 100.) as f32;

  percent
}

#[cfg(test)]
mod tests {
  use skene_core::{Color, Edge, Expr, Length, TokenRef};

  use super::render_expr;
  use crate::declarations::Declarations;

  /// Ring arithmetic renders as `calc()`, nested sums parenthesized, and the math
  /// functions take their operands bare.
  #[test]
  fn arithmetic_renders_as_calc() {
    let width = (Expr::<Length>::vw(50.) + 10.) * Expr::from(TokenRef::<f32>::new("ratio"));

    assert_eq!(
      render_expr(width.node()),
      "calc((50vw + 10px) * var(--ratio))"
    );
    assert_eq!(
      render_expr(
        Expr::from(TokenRef::<Length>::new("gutter"))
          .clamp(Expr::cqw(10.), 40.)
          .node()
      ),
      "clamp(10cqw, var(--gutter), 40px)"
    );
    assert_eq!(
      render_expr(Expr::<Length>::em(1.).min(Expr::vh(2.) / 3.).node()),
      "min(1em, 2vh / 3)"
    );
  }

  /// A safe-area inset renders as the environment variable, zero where the display
  /// keeps none, inside the arithmetic too.
  #[test]
  fn a_safe_area_renders_as_its_environment_variable() {
    assert_eq!(
      render_expr((Expr::<Length>::safe_area(Edge::Bottom) + 8.).node()),
      "calc(env(safe-area-inset-bottom, 0px) + 8px)"
    );
  }

  /// Color functions render as `color-mix()` and relative `oklch()`.
  #[test]
  fn color_functions_render_as_color_mix_and_relative_color() {
    let ink = TokenRef::<Color>::new("ink");

    assert_eq!(
      render_expr(Expr::from(ink).mix(Color::TRANSPARENT, 0.25).node()),
      format!(
        "color-mix(in oklab, var(--ink), {} 25%)",
        Declarations::render_color(Color::TRANSPARENT)
      )
    );
    assert_eq!(
      render_expr(Expr::from(ink).with_alpha(0.5).node()),
      "oklch(from var(--ink) l c h / 0.5)"
    );
    assert_eq!(
      render_expr(Expr::from(ink).lighter(0.1).with_hue(90.).node()),
      "oklch(from oklch(from var(--ink) calc(l + 0.1) c h / alpha) l c 90 / alpha)"
    );
  }
}
