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
// The csr and ssr halves each use a different subset of the surface traits.
#[allow(unused_imports)]
use skene_gui::{HasDecoration, HasFocus};

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, theme cascade, span groups,
/// inline views, and the empty view.
fn representative_tree() -> impl skene_gui::IntoView {
  TextTheme::new()
    .set_child(
      Container::new()
        .set_color(Color::srgb(0.1, 0.2, 0.3))
        .set_border(Border::new(2., Color::oklch(0.5, 0.1, 200.)))
        .set_border_radius(6.)
        .set_min_width(200.)
        .set_max_height(800.)
        .set_child(
          Column::new()
            .set_arrangement(Arrangement::Center)
            .set_item_alignment(ItemAlignment::Stretch)
            .set_spacing(12.)
            .set_padding(Inset::symmetric(16., 24.))
            .set_children((
              Text::new("Title & <escaped>")
                .set_font_size(24.)
                .set_font_weight(700.)
                .set_color(Color::srgb(0.9, 0.9, 0.9)),
              Text::new("linked").set_link("https://example.com/?a=1&b=2"),
              Container::new()
                .set_role(Role::Section)
                .set_cursor(MouseCursor::Click)
                .set_width(120.)
                .set_height(40.)
                .derive_color(|| Color::srgb(0.5, 0.5, 0.5))
                .handle_click(|| {})
                .set_child(Row::new().set_spacing(4.).set_child(Text::new("Press"))),
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
                .set_width_when(Condition::Hovered, 60.)
                .set_width_when(Condition::MaxWidth(480.), 40.)
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
        ),
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

/// The interned rule block (the styles after the reset), in interning order.
const SNAPSHOT_RULES: &str = ".skene-724477ae60473c1c9fa4d726dc94333a{background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border-color:oklch(0.5 0.1 200 / 1);border-radius:6px;border-style:solid;border-width:2px;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;max-height:800px;min-width:200px;}.skene-becc59dceaee8efaef2006f2ecea3515{align-items:stretch;column-gap:0px;display:flex;flex-direction:column;justify-content:center;padding:24px 16px 24px 16px;row-gap:12px;}.skene-95524721ac58c9490ba5f45340bfc5a7{color:oklch(0.9234231 0.000000059604645 0 / 1);display:block;font-size:24px;font-weight:700;}.skene-df4689990dd402a0c362ffa9092704f4{display:block;font-weight:500;}.skene-0f71e67d4b6b250ab4e4f3103c9baed1{cursor:pointer;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:40px;width:120px;}.skene-02aeabf62770f71c6f84daec54b3d382{column-gap:4px;display:flex;flex-direction:row;row-gap:0px;}.skene-3c1a9074e0ae4dd23c42d406382096f9{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:block;}.skene-364525340079d54896621b7a9d017d0c{column-gap:4px;display:grid;grid-template-columns:1fr 1fr 1fr;row-gap:4px;}.skene-829dba803277982936be2a74a896eb85{display:grid;position:relative;}.skene-950d83bfa8dfd60cfb3f4c92d378392b{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);}.skene-7f5322e9eb3d27e76a3bb4928e6c79d3{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-b6e63de7c15f4051acac0bf6ecd8c660{color:oklch(0.7311429 0.18611379 52.775707 / 1);font-weight:600;}.skene-b0b70068b405f9a159e53ce3bcddb2f9{color:oklch(0.8452223 0.00000006664002 116.56505 / 1);display:inline-block;}.skene-a4a1c8bdbfb03d27a40dacf906ed7d1e{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:10px;width:10px;}.skene-a31a8224191a5af8bde3d56b00abaafe{font-weight:500;}.skene-79e24d102c727cb6f45ff3040941cc05{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;padding:8px 8px 8px 8px;}@keyframes skene-k-c792549e{0%{border-radius:0px;}100%{border-radius:8px;}}.skene-a0712251029b125aef07c5b101ec3ae2{animation:skene-k-c792549e 1s ease 0s 1 normal both;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;transition:width 0.2s ease 0s;width:50px;}.skene-a0712251029b125aef07c5b101ec3ae2:hover{width:60px;}@media (max-width: 480px){.skene-a0712251029b125aef07c5b101ec3ae2{width:40px;}}</style>";

const SNAPSHOT_BODY: &str = "<div data-h=\"1\" class=\"skene-724477ae60473c1c9fa4d726dc94333a\"><div data-h=\"2\" class=\"skene-becc59dceaee8efaef2006f2ecea3515\"><span data-h=\"3\" class=\"skene-95524721ac58c9490ba5f45340bfc5a7\">Title &amp; &lt;escaped&gt;</span><a data-h=\"4\" href=\"https://example.com/?a=1&amp;b=2\" target=\"_blank\" rel=\"noopener noreferrer\" class=\"skene-df4689990dd402a0c362ffa9092704f4\">linked</a><section data-h=\"5\" class=\"skene-0f71e67d4b6b250ab4e4f3103c9baed1\" style=\"background-color:oklch(0.5981807 0.00000011920929 0 / 1);\"><div data-h=\"6\" class=\"skene-02aeabf62770f71c6f84daec54b3d382\"><span data-h=\"7\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\">Press</span></div></section><div data-h=\"8\" class=\"skene-364525340079d54896621b7a9d017d0c\"><span data-h=\"9\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\">a</span><span data-h=\"10\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\">b</span><span data-h=\"11\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\">c</span></div><div data-h=\"12\" class=\"skene-829dba803277982936be2a74a896eb85\"><span data-h=\"13\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\" style=\"grid-area:1 / 1;\">under</span><span data-h=\"14\" class=\"skene-3c1a9074e0ae4dd23c42d406382096f9\" style=\"grid-area:1 / 1;\">over</span></div><div data-h=\"15\"><span data-h=\"16\" class=\"skene-950d83bfa8dfd60cfb3f4c92d378392b\">plain </span><span data-h=\"17\" class=\"skene-7f5322e9eb3d27e76a3bb4928e6c79d3\"><span data-h=\"18\" class=\"skene-7f5322e9eb3d27e76a3bb4928e6c79d3\">in </span><span data-h=\"19\" class=\"skene-b6e63de7c15f4051acac0bf6ecd8c660\">group</span></span><span data-h=\"20\" class=\"skene-b0b70068b405f9a159e53ce3bcddb2f9\"><div data-h=\"21\" class=\"skene-a4a1c8bdbfb03d27a40dacf906ed7d1e\"></div></span><a data-h=\"22\" href=\"/rel\" class=\"skene-a31a8224191a5af8bde3d56b00abaafe\"> tail</a></div><div data-h=\"23\" class=\"skene-79e24d102c727cb6f45ff3040941cc05\"></div><div data-h=\"24\" class=\"skene-79e24d102c727cb6f45ff3040941cc05\"></div><div data-h=\"25\" class=\"skene-a0712251029b125aef07c5b101ec3ae2\"></div><div data-h=\"26\"></div></div></div>";
