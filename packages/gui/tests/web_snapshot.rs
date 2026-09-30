//! Byte-exact SSR snapshot: the rendered styles, body, and state for a representative
//! tree. Guards output parity (interned class names, hydration markers, markup shape)
//! across refactors of the realization pipeline.

#![cfg(skene_gui_platform = "server")]

use skene_gui::{
  Column,
  Condition,
  Container,
  Grid,
  GuiStyle,
  HasChild,
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
fn rounded(radius: f32) -> BoxStyle<f32, Keyframe, GuiStyle> {
  let mut style: BoxStyle<f32, Keyframe, GuiStyle> = BoxStyle::default();

  style.ext.corner_radius = Some(radius.into());

  style
}

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, the typography tokens,
/// span groups, inline views, and the empty view.
fn representative_tree() -> impl skene_gui::View {
  Theme::new(
    Typography::default()
      .set_ink(Color::srgb(0.8, 0.8, 0.8))
      .set_link_ink(Color::srgb(0.4, 0.6, 1.)),
  )
  .set_child_builder(|| {
    Container::new()
      .set_color(Color::srgb(0.1, 0.2, 0.3))
      .set_border_width(Inset::all(2.))
      .set_border_color(Color::oklch(0.5, 0.1, 200.))
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
              .set_child(Row::new().set_spacing(4.).add_child(Text::new("Press"))),
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
const SNAPSHOT_RULES: &str = "@layer skene.style{.skene-9194ab83dca5ec80eafe918a1a809d90{--typography-ink:oklch(0.8452223 0 0 / 1);--typography-link-ink:oklch(0.6940848 0.16008389 262.85895 / 1);--typography-selection-ink:oklch(1 0 0 / 1);--typography-selection-surface:oklch(0.598701 0.19566482 261.77704 / 1);--typography-size:16px;--typography-weight:400;--typography-line-height:1.5;--typography-letter-spacing:0px;}@media (prefers-color-scheme: dark){:where(:root:not([data-skene-scheme=\"light\"])) .skene-9194ab83dca5ec80eafe918a1a809d90{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}}:where(:root[data-skene-scheme=\"dark\"]) .skene-9194ab83dca5ec80eafe918a1a809d90{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}.skene-85dcc8a1d92bd4d20096ca87f95eb718{background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border-color:oklch(0.5 0.1 200 / 1);border-radius:6px;border-width:2px 2px 2px 2px;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;max-height:800px;min-width:200px;}.skene-c1ff0c2ec262a8fb0df124849e420eda{align-items:stretch;display:flex;flex-direction:column;gap:12px 12px;justify-content:center;padding:24px 16px 24px 16px;}.skene-7bf1b2a99ff57edb412640cc16f1c595{color:oklch(0.9234231 0 0 / 1);display:block;font-size:24px;font-weight:700;}.skene-7bf1b2a99ff57edb412640cc16f1c595::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-e84875542aaef46d5e501133c3627eaf{color:var(--typography-link-ink);display:block;}.skene-e84875542aaef46d5e501133c3627eaf::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-645dbb5320f06b45819113831d7d1876{background-color:var(--skene-color);cursor:pointer;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:40px;width:120px;}.skene-e7842b834680d95f5a9f8ab966722001{display:flex;flex-direction:row;gap:4px 4px;}.skene-eeb30266b2ef222a17d455dd848fb6dc{color:var(--typography-ink);display:block;}.skene-eeb30266b2ef222a17d455dd848fb6dc::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-ad0524ef57797d00baa729d0f42983f2{display:grid;gap:4px 4px;grid-template-columns:1fr 1fr 1fr;}.skene-3372e730cc8303e483d94ab118c41731{display:grid;position:relative;}.skene-3372e730cc8303e483d94ab118c41731>*{grid-area:1 / 1;}.skene-22f2f66e5410849080fc4ef06a3c7c13{display:flow-root;}.skene-ca02ab46bf146e54d0d57dec0894642f{color:var(--typography-ink);}.skene-ca02ab46bf146e54d0d57dec0894642f::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-2c7d06b11e0320b500440aa0c0c9d868{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-2c7d06b11e0320b500440aa0c0c9d868::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-e32aea12a98a68a45a0da4019652555f{--typography-ink:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-ccc2bd1c473339f80176733b30b9d7bc{color:var(--typography-ink);font-weight:600;}.skene-ccc2bd1c473339f80176733b30b9d7bc::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-013f7d3b3443ac6dd173672cf7783721{color:var(--typography-ink);display:inline-block;}.skene-013f7d3b3443ac6dd173672cf7783721::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-f10e7dac2b5f563add93f61f18da86cc{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:10px;width:10px;}.skene-f877c18c6b757c6806b1ec647daea8a4{color:var(--typography-link-ink);}.skene-f877c18c6b757c6806b1ec647daea8a4::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-be05c8114969276a2b3898e78dd8981a{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;padding:8px 8px 8px 8px;}@keyframes skene-k-be46dbea7553eec2cc3d8acbc907d23a{0%{border-radius:0px;}100%{border-radius:8px;}}.skene-72e67175860fb8f93ba30c75e785bcb7{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;width:50px;}.skene-72e67175860fb8f93ba30c75e785bcb7:where(:hover){width:60px;}@media (max-width: 480px){.skene-72e67175860fb8f93ba30c75e785bcb7{width:40px;}}@media (prefers-reduced-motion: no-preference){.skene-72e67175860fb8f93ba30c75e785bcb7{animation:skene-k-be46dbea7553eec2cc3d8acbc907d23a 1s ease 0s 1 normal both;transition:width 0.2s ease 0s;}}@media (prefers-reduced-motion: reduce){.skene-72e67175860fb8f93ba30c75e785bcb7{animation:skene-k-be46dbea7553eec2cc3d8acbc907d23a 0s ease 0s 1 normal both;}}}</style>";

const SNAPSHOT_BODY: &str = "<div data-h=\"1\" data-skene-scope class=\"skene-9194ab83dca5ec80eafe918a1a809d90\" style=\"display:contents;\"><div data-h=\"2\" class=\"skene-85dcc8a1d92bd4d20096ca87f95eb718\"><!--s:0--><div data-h=\"3\" class=\"skene-c1ff0c2ec262a8fb0df124849e420eda\"><!--s:1--><span data-h=\"4\" class=\"skene-7bf1b2a99ff57edb412640cc16f1c595\">Title &amp; &lt;escaped&gt;</span><a data-h=\"5\" href=\"https://example.com/?a=1&amp;b=2\" target=\"_blank\" rel=\"noopener noreferrer\" class=\"skene-e84875542aaef46d5e501133c3627eaf\">linked</a><section data-h=\"6\" class=\"skene-645dbb5320f06b45819113831d7d1876\" style=\"--skene-color:oklch(0.5981807 0 0 / 1);\"><!--s:2--><div data-h=\"7\" class=\"skene-e7842b834680d95f5a9f8ab966722001\"><!--s:3--><span data-h=\"8\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">Press</span><!--e:3--></div><!--e:2--></section><div data-h=\"9\" class=\"skene-ad0524ef57797d00baa729d0f42983f2\"><!--s:4--><span data-h=\"10\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">a</span><span data-h=\"11\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">b</span><span data-h=\"12\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">c</span><!--e:4--></div><div data-h=\"13\" class=\"skene-3372e730cc8303e483d94ab118c41731\"><!--s:5--><span data-h=\"14\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">under</span><span data-h=\"15\" class=\"skene-eeb30266b2ef222a17d455dd848fb6dc\">over</span><!--e:5--></div><div data-h=\"16\" class=\"skene-22f2f66e5410849080fc4ef06a3c7c13\"><span data-h=\"17\" class=\"skene-ca02ab46bf146e54d0d57dec0894642f\">plain </span><span data-h=\"18\" class=\"skene-2c7d06b11e0320b500440aa0c0c9d868 skene-e32aea12a98a68a45a0da4019652555f\"><span data-h=\"19\" class=\"skene-ca02ab46bf146e54d0d57dec0894642f\">in </span><span data-h=\"20\" class=\"skene-ccc2bd1c473339f80176733b30b9d7bc\">group</span></span><span data-h=\"21\" class=\"skene-013f7d3b3443ac6dd173672cf7783721\"><div data-h=\"22\" class=\"skene-f10e7dac2b5f563add93f61f18da86cc\"></div></span><a data-h=\"23\" href=\"/rel\" class=\"skene-f877c18c6b757c6806b1ec647daea8a4\"> tail</a></div><div data-h=\"24\" class=\"skene-be05c8114969276a2b3898e78dd8981a\"></div><div data-h=\"25\" class=\"skene-be05c8114969276a2b3898e78dd8981a\"></div><div data-h=\"26\" class=\"skene-72e67175860fb8f93ba30c75e785bcb7\"></div><!--s:6--><!--e:6--><!--e:1--></div><!--e:0--></div></div>";
