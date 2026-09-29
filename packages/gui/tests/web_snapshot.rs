//! Byte-exact SSR snapshot: the rendered styles, body, and state for a representative
//! tree. Guards output parity (interned class names, hydration markers, markup shape)
//! across refactors of the realization pipeline.

#![cfg(skene_leaf = "server")]

use skene_gui::{
  Border,
  Column,
  Condition,
  Container,
  Grid,
  GuiExt,
  HasChildren,
  Inset,
  Media,
  RichText,
  Role,
  Row,
  Span,
  Stack,
  State,
  Text,
  Theme,
  Typography,
  View,
  When,
  core::{
    Animation,
    Arrangement,
    BoxStyle,
    Color,
    Dimension,
    ItemAlignment,
    Keyframe,
    MouseCursor,
    Property,
    Transition,
  },
  empty,
  render_to_string,
};

/// A plain box style rounding its corners by `radius`, for a keyframe.
fn rounded(radius: f32) -> BoxStyle<f32, Keyframe, GuiExt> {
  let mut style: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();

  style.ext.corner_radius = Some(radius.into());

  style
}

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, the typography tokens,
/// span groups, inline views, and the empty view.
fn representative_tree() -> impl skene_gui::IntoView {
  Theme::new(
    Typography::default()
      .set_ink(Color::srgb(0.8, 0.8, 0.8))
      .set_link_ink(Color::srgb(0.4, 0.6, 1.)),
  )
  .set_child_builder(|| {
    Container::new()
      .set_color(Color::srgb(0.1, 0.2, 0.3))
      .set_border(Border::new(2., Color::oklch(0.5, 0.1, 200.)))
      .set_corner_radius(6.)
      .set_min_width(Dimension::Length(200.))
      .set_max_height(Dimension::Length(800.))
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
              .set_width(Dimension::Length(120.))
              .set_height(Dimension::Length(40.))
              .derive_color(|| Color::srgb(0.5, 0.5, 0.5))
              .handle_click(|| {})
              .set_child(Row::new().set_spacing(4.).set_child(Text::new("Press"))),
            Grid::uniform(3).set_spacing(4.).set_children((
              Text::new("a"),
              Text::new("b"),
              Text::new("c"),
            )),
            Stack::new().set_children((Text::new("under"), Text::new("over"))),
            RichText::new().set_spans([
              Span::new("plain "),
              Span::group([Span::new("in "), Span::new("group").set_font_weight(600.)])
                .set_color(Color::srgb(1.0, 0.5, 0.0)),
              Span::view(
                Container::new()
                  .set_width(Dimension::Length(10.))
                  .set_height(Dimension::Length(10.)),
              ),
              Span::new(" tail").set_link("/rel"),
            ]),
            // Two identically-styled boxes: their class must intern once.
            Container::new().set_padding(Inset::all(8.)),
            Container::new().set_padding(Inset::all(8.)),
            // Declarative dynamics: a hover variant, a media variant, a transition, and an
            // animation, all compiled into the interned class.
            Container::new()
              .set_width(
                Dimension::Length(50.)
                  .when(Condition::from(State::Hovered), Dimension::Length(60.))
                  .when(
                    Condition::from(Media::MaxWidth(480.)),
                    Dimension::Length(40.),
                  ),
              )
              .set_transition(Transition::new(Property::Width, 0.2))
              .set_animation(Animation::new(1., [(0., rounded(0.)), (100., rounded(8.))])),
            empty().into_any(),
          )),
      )
  })
}

#[test]
fn representative_render_is_stable() {
  let rendered = render_to_string(representative_tree);

  assert_eq!(
    rendered.state_script(),
    "<script id=\"skene-state\" type=\"application/json\">[[],[],[]]</script>"
  );
  assert!(rendered.styles().starts_with("<style id=\"skene-styles\">"));
  assert!(
    rendered.styles().ends_with(SNAPSHOT_RULES),
    "interned rules changed:\n{}",
    rendered.styles(),
  );

  assert_eq!(rendered.body(), SNAPSHOT_BODY);
}

/// The style layer holding the interned rules (the styles after the reset and the slot
/// registrations), in interning order.
const SNAPSHOT_RULES: &str = "@layer skene.style{.skene-1562bb8886f4f1a6220e7f7c0562f17b{--typography-ink:oklch(0.8452223 0 0 / 1);--typography-link-ink:oklch(0.6940848 0.16008389 262.85895 / 1);--typography-selection-ink:oklch(1 0 0 / 1);--typography-selection-surface:oklch(0.598701 0.19566482 261.77704 / 1);--typography-size:16px;--typography-weight:400;--typography-line-height:1.5;--typography-letter-spacing:0px;}@media (prefers-color-scheme: dark){:where(:root:not([data-skene-scheme=\"light\"])) .skene-1562bb8886f4f1a6220e7f7c0562f17b{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}}:where(:root[data-skene-scheme=\"dark\"]) .skene-1562bb8886f4f1a6220e7f7c0562f17b{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}.skene-d4c4032694527bb8dec2dbbd5bbfda45{background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border:2px solid oklch(0.5 0.1 200 / 1);border-radius:6px;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;max-height:800px;min-width:200px;}.skene-184a4ee79eeced6666de2eab3621a15f{align-items:stretch;display:flex;flex-direction:column;gap:12px 12px;justify-content:center;padding:24px 16px 24px 16px;}.skene-ade22e6c8daed11f084c147d18ce509d{color:oklch(0.9234231 0 0 / 1);display:block;font-size:24px;font-weight:700;}.skene-ade22e6c8daed11f084c147d18ce509d::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-3d4161193dce5f9f4d7c8e3c735fdb60{color:var(--typography-link-ink);display:block;}.skene-3d4161193dce5f9f4d7c8e3c735fdb60::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-661b20a203dce58168202ad302aebd34{background-color:var(--skene-color);cursor:pointer;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:40px;width:120px;}.skene-728b07255fd8e205c74ba2eb010a1c9b{display:flex;flex-direction:row;gap:4px 4px;}.skene-192caaf1c24a93affaf93c345a5626c1{color:var(--typography-ink);display:block;}.skene-192caaf1c24a93affaf93c345a5626c1::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-e7d93f2e888f043c41a3f2a699c4dccd{display:grid;gap:4px 4px;grid-template-columns:1fr 1fr 1fr;}.skene-10d7c809f0d866668b624a0959d212b2{display:grid;position:relative;}.skene-10d7c809f0d866668b624a0959d212b2>*{grid-area:1 / 1;}.skene-2dec8200e24e87fd393c28f671610b88{display:flow-root;}.skene-72f98cf449c8cc2023ddb52c7dfde077{color:var(--typography-ink);}.skene-72f98cf449c8cc2023ddb52c7dfde077::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-e4a56d0e246b4ffe18b44544750a6402{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-e4a56d0e246b4ffe18b44544750a6402::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-2bf900ccb708a24e72d55e0ad3841e4c{--typography-ink:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-a15bcf57cecc68e85e61d45ca25484e8{color:var(--typography-ink);font-weight:600;}.skene-a15bcf57cecc68e85e61d45ca25484e8::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-15d26de5dfb9838e9b513afe0c61995f{color:var(--typography-ink);display:inline-block;}.skene-15d26de5dfb9838e9b513afe0c61995f::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-a4a1c8bdbfb03d27a40dacf906ed7d1e{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:10px;width:10px;}.skene-0704dd5a371a783c71c17e46d8213a7e{color:var(--typography-link-ink);}.skene-0704dd5a371a783c71c17e46d8213a7e::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-79e24d102c727cb6f45ff3040941cc05{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;padding:8px 8px 8px 8px;}@keyframes skene-k-a5c6344e07d25374b76d16a97cd1a03c{0%{border-radius:0px;}100%{border-radius:8px;}}.skene-d34edec81268a00acfa8ff2cd109289f{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;width:50px;}.skene-d34edec81268a00acfa8ff2cd109289f:where(:hover){width:60px;}@media (max-width: 480px){.skene-d34edec81268a00acfa8ff2cd109289f{width:40px;}}@media (prefers-reduced-motion: no-preference){.skene-d34edec81268a00acfa8ff2cd109289f{animation:skene-k-a5c6344e07d25374b76d16a97cd1a03c 1s ease 0s 1 normal both;transition:width 0.2s ease 0s;}}@media (prefers-reduced-motion: reduce){.skene-d34edec81268a00acfa8ff2cd109289f{animation:skene-k-a5c6344e07d25374b76d16a97cd1a03c 0s ease 0s 1 normal both;}}}</style>";

const SNAPSHOT_BODY: &str = "<div data-h=\"1\" data-skene-scope class=\"skene-1562bb8886f4f1a6220e7f7c0562f17b\" style=\"display:contents;\"><div data-h=\"2\" class=\"skene-d4c4032694527bb8dec2dbbd5bbfda45\"><!--s:0--><div data-h=\"3\" class=\"skene-184a4ee79eeced6666de2eab3621a15f\"><!--s:1--><span data-h=\"4\" class=\"skene-ade22e6c8daed11f084c147d18ce509d\">Title &amp; &lt;escaped&gt;</span><a data-h=\"5\" href=\"https://example.com/?a=1&amp;b=2\" target=\"_blank\" rel=\"noopener noreferrer\" class=\"skene-3d4161193dce5f9f4d7c8e3c735fdb60\">linked</a><section data-h=\"6\" class=\"skene-661b20a203dce58168202ad302aebd34\" style=\"--skene-color:oklch(0.5981807 0 0 / 1);\"><!--s:2--><div data-h=\"7\" class=\"skene-728b07255fd8e205c74ba2eb010a1c9b\"><!--s:3--><span data-h=\"8\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">Press</span><!--e:3--></div><!--e:2--></section><div data-h=\"9\" class=\"skene-e7d93f2e888f043c41a3f2a699c4dccd\"><!--s:4--><span data-h=\"10\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">a</span><span data-h=\"11\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">b</span><span data-h=\"12\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">c</span><!--e:4--></div><div data-h=\"13\" class=\"skene-10d7c809f0d866668b624a0959d212b2\"><!--s:5--><span data-h=\"14\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">under</span><span data-h=\"15\" class=\"skene-192caaf1c24a93affaf93c345a5626c1\">over</span><!--e:5--></div><div data-h=\"16\" class=\"skene-2dec8200e24e87fd393c28f671610b88\"><span data-h=\"17\" class=\"skene-72f98cf449c8cc2023ddb52c7dfde077\">plain </span><span data-h=\"18\" class=\"skene-e4a56d0e246b4ffe18b44544750a6402 skene-2bf900ccb708a24e72d55e0ad3841e4c\"><span data-h=\"19\" class=\"skene-72f98cf449c8cc2023ddb52c7dfde077\">in </span><span data-h=\"20\" class=\"skene-a15bcf57cecc68e85e61d45ca25484e8\">group</span></span><span data-h=\"21\" class=\"skene-15d26de5dfb9838e9b513afe0c61995f\"><div data-h=\"22\" class=\"skene-a4a1c8bdbfb03d27a40dacf906ed7d1e\"></div></span><a data-h=\"23\" href=\"/rel\" class=\"skene-0704dd5a371a783c71c17e46d8213a7e\"> tail</a></div><div data-h=\"24\" class=\"skene-79e24d102c727cb6f45ff3040941cc05\"></div><div data-h=\"25\" class=\"skene-79e24d102c727cb6f45ff3040941cc05\"></div><div data-h=\"26\" class=\"skene-d34edec81268a00acfa8ff2cd109289f\"></div><!--s:6--><!--e:6--><!--e:1--></div><!--e:0--></div></div>";
