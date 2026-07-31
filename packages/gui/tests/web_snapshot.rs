//! Byte-exact SSR snapshot: the rendered styles, body, and state for a representative
//! tree. Guards output parity (interned class names, hydration markers, markup shape)
//! across refactors of the realization pipeline.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Border,
  Container,
  HasChildren,
  Inset,
  RichText,
  Role,
  Span,
  Text,
  TextStyle,
  TextTheme,
  View,
  core::{Color, CrossAxisAlignment, MainAxisAlignment, MouseCursor},
  empty,
  render_to_string,
};

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, theme cascade, span groups,
/// inline views, and the empty view.
fn representative_tree() -> impl skene_gui::IntoView {
  TextTheme::new(
    Container::column()
      .set_main_axis_alignment(MainAxisAlignment::Center)
      .set_cross_axis_alignment(CrossAxisAlignment::Stretch)
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
        Container::row()
          .set_role(Role::Button)
          .set_cursor(MouseCursor::Click)
          .set_width(120.)
          .set_height(40.)
          .derive_color(|| Color::srgb(0.5, 0.5, 0.5))
          .handle_click(|| {})
          .set_child(Text::new("Press")),
        Container::grid(3).set_spacing(4.).set_children((
          Text::new("a"),
          Text::new("b"),
          Text::new("c"),
        )),
        Container::stack().set_children((Text::new("under"), Text::new("over"))),
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
        empty().into_any(),
      )),
  )
  .set_base(TextStyle::new().set_color(Color::srgb(0.8, 0.8, 0.8)))
  .set_link(TextStyle::new().set_font_weight(500.))
}

#[test]
fn representative_render_is_stable() {
  let rendered = render_to_string(representative_tree);

  assert_eq!(rendered.state(), "[]");
  assert!(rendered.styles().starts_with("<style>"));
  assert!(
    rendered.styles().ends_with(SNAPSHOT_RULES),
    "interned rules changed:\n{}",
    rendered.styles(),
  );
  assert_eq!(rendered.body(), SNAPSHOT_BODY);
}

/// The interned rule block (the styles after the reset), sorted by class name.
const SNAPSHOT_RULES: &str = ".skene-17a1b6ee{display:flow-root;padding:8px 8px 8px 8px;}.skene-3b7ac76f{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:block;font-weight:500;}.skene-672c937e{display:flow-root;height:10px;width:10px;}.skene-7e7afc41{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);font-weight:500;}.skene-7f1a3d7c{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-7f8fd1e5{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:inline-block;}.skene-8512280b{align-items:stretch;background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border-color:oklch(0.5 0.1 200 / 1);border-radius:6px;border-style:solid;border-width:2px;display:flex;flex-direction:column;gap:12px;justify-content:center;max-height:800px;min-width:200px;padding:24px 16px 24px 16px;}.skene-8686c52a{color:oklch(0.7311429 0.18611379 52.775707 / 1);font-weight:600;}.skene-98c7435c{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:block;}.skene-9953c877{display:grid;}.skene-ad435bb4{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);}.skene-ae79fa8d{display:grid;gap:4px;grid-template-columns:repeat(3, 1fr);}.skene-c00b701c{cursor:pointer;display:flex;flex-direction:row;height:40px;width:120px;}.skene-f446f81f{color:oklch(0.9234231 0.000000059604645 0 / 1);display:block;font-size:24px;font-weight:700;}</style>";

const SNAPSHOT_BODY: &str = "<div data-h=\"1\" class=\"skene-8512280b\"><span data-h=\"2\" class=\"skene-f446f81f\">Title &amp; &lt;escaped&gt;</span><a data-h=\"3\" href=\"https://example.com/?a=1&amp;b=2\" class=\"skene-3b7ac76f\">linked</a><button data-h=\"4\" type=\"button\" class=\"skene-c00b701c\" style=\"background-color:oklch(0.5981807 0.00000011920929 0 / 1);\"><span data-h=\"5\" class=\"skene-98c7435c\">Press</span></button><div data-h=\"6\" class=\"skene-ae79fa8d\"><span data-h=\"7\" class=\"skene-98c7435c\">a</span><span data-h=\"8\" class=\"skene-98c7435c\">b</span><span data-h=\"9\" class=\"skene-98c7435c\">c</span></div><div data-h=\"10\" class=\"skene-9953c877\"><span data-h=\"11\" class=\"skene-98c7435c\" style=\"grid-area:1 / 1;\">under</span><span data-h=\"12\" class=\"skene-98c7435c\" style=\"grid-area:1 / 1;\">over</span></div><div data-h=\"13\"><span data-h=\"14\" class=\"skene-ad435bb4\">plain </span><span data-h=\"15\" class=\"skene-7f1a3d7c\"><span data-h=\"16\" class=\"skene-7f1a3d7c\">in </span><span data-h=\"17\" class=\"skene-8686c52a\">group</span></span><span data-h=\"18\" class=\"skene-7f8fd1e5\"><div data-h=\"19\" class=\"skene-672c937e\"></div></span><a data-h=\"20\" href=\"/rel\" class=\"skene-7e7afc41\"> tail</a></div><div data-h=\"21\" class=\"skene-17a1b6ee\"></div><div data-h=\"22\" class=\"skene-17a1b6ee\"></div><div data-h=\"23\"></div></div>";
