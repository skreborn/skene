//! Byte-exact SSR snapshot: the rendered styles, body, and state for a representative
//! tree. Guards output parity (interned class names, hydration markers, markup shape)
//! across refactors of the realization pipeline.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Border,
  Column,
  Container,
  Grid,
  HasChildren,
  Inset,
  RichText,
  Role,
  Row,
  Span,
  Stack,
  Text,
  TextStyle,
  TextTheme,
  View,
  core::{
    Animation,
    Arrangement,
    BoxStyle,
    Color,
    Condition,
    ItemAlignment,
    MouseCursor,
    StyleProperty,
    Transition,
  },
  empty,
  render_to_string,
};

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, theme cascade, span groups,
/// inline views, and the empty view.
fn representative_tree() -> impl skene_gui::IntoView {
  TextTheme::new(
    Column::new()
      .set_arrangement(Arrangement::Center)
      .set_item_alignment(ItemAlignment::Stretch)
      .set_spacing(12.)
      .set_padding(Inset::symmetric(16., 24.))
      .set_color(Color::srgb(0.1, 0.2, 0.3))
      .set_border(Border::new(2., Color::oklch(0.5, 0.1, 200.)))
      .set_border_radius(6.)
      .set_min_width(200.)
      .set_max_height(800.)
      .set_children((
        Text::new("Title & <escaped>")
          .set_font_size(24.)
          .set_font_weight(700.)
          .set_color(Color::srgb(0.9, 0.9, 0.9)),
        Text::new("linked").set_link("https://example.com/?a=1&b=2"),
        Row::new()
          .set_role(Role::Button)
          .set_cursor(MouseCursor::Click)
          .set_width(120.)
          .set_height(40.)
          .derive_color(|| Color::srgb(0.5, 0.5, 0.5))
          .handle_click(|| {})
          .set_child(Text::new("Press")),
        Grid::uniform(3).set_spacing(4.).set_children((
          Text::new("a"),
          Text::new("b"),
          Text::new("c"),
        )),
        Stack::new().set_children((Text::new("under"), Text::new("over"))),
        RichText::new([
          Span::text("plain "),
          Span::group([Span::text("in "), Span::text("group").set_font_weight(600.)])
            .set_color(Color::srgb(1.0, 0.5, 0.0)),
          Span::view(Container::new().set_width(10.).set_height(10.)),
          Span::text(" tail").set_link("/rel"),
        ]),
        // Two identically-styled boxes: their class must intern once.
        Container::new().set_padding(Inset::all(8.)),
        Container::new().set_padding(Inset::all(8.)),
        // Declarative dynamics: a hover variant, a media variant, a transition, and an
        // animation, all compiled into the interned class.
        Container::new()
          .set_width(50.)
          .set_when(Condition::Hovered, BoxStyle::new().set_width(60.))
          .set_when(Condition::MaxWidth(480.), BoxStyle::new().set_width(40.))
          .set_transition(Transition::new(StyleProperty::Width, 0.2))
          .set_animation(Animation::new(
            1.,
            [
              (0., BoxStyle::new().set_border_radius(0.)),
              (100., BoxStyle::new().set_border_radius(8.)),
            ],
          )),
        empty().into_any(),
      )),
  )
  .set_base(TextStyle::new().set_color(Color::srgb(0.8, 0.8, 0.8)))
  .set_link(TextStyle::new().set_font_weight(500.))
}

#[test]
fn representative_render_is_stable() {
  let rendered = render_to_string(representative_tree);

  assert_eq!(rendered.state(), "[[],[]]");
  assert!(rendered.styles().starts_with("<style>"));
  assert!(
    rendered.styles().ends_with(SNAPSHOT_RULES),
    "interned rules changed:\n{}",
    rendered.styles(),
  );

  assert_eq!(rendered.body(), SNAPSHOT_BODY);
}

/// The interned rule block (the styles after the reset), sorted by class name.
const SNAPSHOT_RULES: &str = ".skene-1e21c5dd{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);}.skene-2e87257c{column-gap:4px;display:grid;grid-template-columns:1fr 1fr 1fr;row-gap:4px;}.skene-2f32df5c{display:grid;position:relative;}.skene-34816c7a{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);font-weight:500;}.skene-363f1d58{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-3ebd287d{cursor:pointer;display:flex;flex-direction:row;height:40px;width:120px;}.skene-4a0198c2{display:flow-root;height:10px;width:10px;}.skene-5f645ac1{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:inline-block;}.skene-688fd045{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:block;}.skene-6e356e0f{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:block;font-weight:500;}.skene-74b5828{align-items:stretch;background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border-color:oklch(0.5 0.1 200 / 1);border-radius:6px;border-style:solid;border-width:2px;column-gap:0px;display:flex;flex-direction:column;justify-content:center;max-height:800px;min-width:200px;padding:24px 16px 24px 16px;row-gap:12px;}.skene-7757becd{display:flow-root;padding:8px 8px 8px 8px;}.skene-b88e6449{color:oklch(0.9234231 0.000000059604645 0 / 1);display:block;font-size:24px;font-weight:700;}@keyframes skene-k-c792549e{0%{border-radius:0px;}100%{border-radius:8px;}}.skene-bc803f39{animation:skene-k-c792549e 1s ease 0s 1 normal both;display:flow-root;transition:width 0.2s ease 0s;width:50px;}.skene-bc803f39:hover{width:60px;}@media (max-width: 480px){.skene-bc803f39{width:40px;}}.skene-e97c44ab{color:oklch(0.7311429 0.18611379 52.775707 / 1);font-weight:600;}</style>";

const SNAPSHOT_BODY: &str = "<div data-h=\"1\" class=\"skene-74b5828\"><span data-h=\"2\" class=\"skene-b88e6449\">Title &amp; &lt;escaped&gt;</span><a data-h=\"3\" href=\"https://example.com/?a=1&amp;b=2\" class=\"skene-6e356e0f\">linked</a><button data-h=\"4\" type=\"button\" class=\"skene-3ebd287d\" style=\"background-color:oklch(0.5981807 0.00000011920929 0 / 1);\"><span data-h=\"5\" class=\"skene-688fd045\">Press</span></button><div data-h=\"6\" class=\"skene-2e87257c\"><span data-h=\"7\" class=\"skene-688fd045\">a</span><span data-h=\"8\" class=\"skene-688fd045\">b</span><span data-h=\"9\" class=\"skene-688fd045\">c</span></div><div data-h=\"10\" class=\"skene-2f32df5c\"><span data-h=\"11\" class=\"skene-688fd045\" style=\"grid-area:1 / 1;\">under</span><span data-h=\"12\" class=\"skene-688fd045\" style=\"grid-area:1 / 1;\">over</span></div><div data-h=\"13\"><span data-h=\"14\" class=\"skene-1e21c5dd\">plain </span><span data-h=\"15\" class=\"skene-363f1d58\"><span data-h=\"16\" class=\"skene-363f1d58\">in </span><span data-h=\"17\" class=\"skene-e97c44ab\">group</span></span><span data-h=\"18\" class=\"skene-5f645ac1\"><div data-h=\"19\" class=\"skene-4a0198c2\"></div></span><a data-h=\"20\" href=\"/rel\" class=\"skene-34816c7a\"> tail</a></div><div data-h=\"21\" class=\"skene-7757becd\"></div><div data-h=\"22\" class=\"skene-7757becd\"></div><div data-h=\"23\" class=\"skene-bc803f39\"></div><div data-h=\"24\"></div></div>";
