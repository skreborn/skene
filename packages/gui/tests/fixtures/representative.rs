// The representative tree both of the web's leaves build, and the bytes the server leaf
// renders for it: its style block, its body and its state script. Not a test target of
// its own; a test includes it into a module.

use skene_foundation::{
  Animation,
  Arrangement,
  BoxStyle,
  Color,
  ColumnBase,
  ContainerBase,
  DecorationBase,
  DecorationWithCornerRadius,
  FlexBase,
  GridBase,
  ItemAlignment,
  Keyframe,
  MotionBase,
  MouseCursor,
  Property,
  RichTextBase,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  SpanBase,
  StackBase,
  TextBase,
  ThemeBase,
  Transition,
};
use skene_gui::{
  Column,
  Condition,
  Container,
  Definition,
  Grid,
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
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  Typography,
  View,
  WebStyle,
  When,
  empty,
};

/// A plain box style rounding its corners by `radius`, for a keyframe.
fn rounded(radius: f32) -> BoxStyle<f32, Keyframe, WebStyle> {
  let mut style: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();

  style.ext.corner_radius = Some(radius.into());

  style
}

/// A component library's palette whose reference carries its default, answering the
/// scheme: what a node below no theme naming its token defines at the root.
#[derive(Debug, Clone, PartialEq)]
struct Swatch {
  /// The swatch's color.
  swatch: Token<Color>,
}

impl Swatch {
  const SWATCH: TokenRef<Color> = TokenRef::of::<Self, f32>("fixture-swatch");
}

impl Default for Swatch {
  fn default() -> Self {
    Self {
      swatch: Token::new(Self::SWATCH, Color::srgb(0.2, 0.6, 0.4))
        .define_when(Media::Dark, Color::srgb(0.1, 0.3, 0.2)),
    }
  }
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for Swatch {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.swatch.define()]
  }
}

/// A tree exercising every built-in view and property path: static and derived box
/// properties, roles, stack placement, text styling and links, the typography tokens,
/// span groups, inline views, the empty view, and a linked palette's default at the
/// root.
pub(super) fn tree() -> impl View {
  Theme::new(
    Typography::default()
      .set_ink(Color::srgb(0.8, 0.8, 0.8))
      .set_link_ink(Color::srgb(0.4, 0.6, 1.)),
  )
  .set_child_builder(|| {
    Container::new()
      .set_background(Color::srgb(0.1, 0.2, 0.3))
      .set_border_width(Inset::all(2))
      .set_border_color(Color::oklch(0.5, 0.1, 200.))
      .set_corner_radius(6)
      .set_min_width(200)
      .set_max_height(800)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Stretch)
          .set_spacing(12)
          .set_padding(Inset::symmetric(16, 24))
          .set_children((
            Text::new("Title & <escaped>")
              .set_font_size(24.)
              .set_font_weight(700.)
              .set_color(Color::srgb(0.9, 0.9, 0.9)),
            Text::new("linked").set_link("https://example.com/?a=1&b=2"),
            Container::new()
              .set_role(Role::Region)
              .set_cursor(MouseCursor::Click)
              .set_width(120)
              .set_height(40)
              .derive_background(|| Color::srgb(0.5, 0.5, 0.5))
              .handle_click(|| {})
              .set_child(Row::new().set_spacing(4).add_child(Text::new("Press"))),
            Grid::uniform(3).set_spacing(4).set_children((
              Text::new("a"),
              Text::new("b"),
              Text::new("c"),
            )),
            Stack::new().set_children((Text::new("under"), Text::new("over"))),
            RichText::new().add_spans([
              Span::new("plain "),
              Span::group([Span::new("in "), Span::new("group").set_font_weight(600.)])
                .set_color(Color::srgb(1.0, 0.5, 0.0)),
              Span::view(
                Container::new()
                  .set_width(10)
                  .set_height(10),
              ),
              Span::new(" tail").set_link("/rel"),
            ]),
            // Two identically-styled boxes: their class must intern once.
            Container::new().set_padding(Inset::all(8)),
            Container::new().set_padding(Inset::all(8)),
            // Declarative dynamics: a hover variant, a media variant, a transition, and an
            // animation, all compiled into the interned class.
            Container::new()
              .set_width(
                50
                  .when(Condition::from(State::Hovered), 60)
                  .when(
                    Condition::from(Media::max_width(480)),
                    40,
                  ),
              )
              .add_transition(Transition::new(Property::Width, 0.2))
              .add_animation(Animation::new(1., [(0., rounded(0.)), (100., rounded(8.))])),
            empty().into_any(),
            // A linked token below no theme: its palette's default defined at the root.
            Container::new()
              .set_height(4)
              .set_background(Swatch::SWATCH)
              .into_any(),
          )),
      )
  })
}

/// The `<style>` block the server leaf renders for [`tree`]: the reset, the platform's
/// colors at the root, the slot and token registrations, and the interned rules in
/// interning order.
pub(super) const STYLES: &str = concat!(
  "<style id=\"skene-styles\">",
  include_str!("../../src/reset.css"),
  "@property --system-ink{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-muted-ink{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-window{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-control{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-control-hover{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-control-pressed{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-border{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-accent{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-accent-hover{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-accent-pressed{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-on-accent{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-track{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-knob{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-field{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-selection{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --system-selection-inactive{syntax:\"<color>\";inherits:true;initial-value:transparent}@layer skene.reset{:where(:root){color-scheme:light;--system-ink:CanvasText;--system-muted-ink:GrayText;--system-window:Canvas;--system-control:ButtonFace;--system-control-hover:color-mix(in oklab,var(--system-control),var(--system-ink) 8%);--system-control-pressed:color-mix(in oklab,var(--system-control),var(--system-ink) 16%);--system-border:ButtonBorder;--system-accent:AccentColor;--system-accent-hover:color-mix(in oklab,var(--system-accent),var(--system-ink) 12%);--system-accent-pressed:color-mix(in oklab,var(--system-accent),var(--system-ink) 24%);--system-on-accent:AccentColorText;--system-track:color-mix(in oklab,var(--system-window),var(--system-ink) 20%);--system-knob:light-dark(Canvas,CanvasText);--system-field:Field;--system-selection:Highlight;--system-selection-inactive:color-mix(in oklab,GrayText 35%,transparent);}@supports not (color:AccentColor){:where(:root){--system-accent:light-dark(#0075ff,#99c8ff);--system-on-accent:light-dark(#ffffff,#1b1b1b);}}@media (prefers-color-scheme: dark){:root:where(:not([data-skene-scheme=\"light\"])){color-scheme:dark;}}:root:where([data-skene-scheme=\"dark\"]){color-scheme:dark;}}",
  "@property --skene-width{syntax:\"*\";inherits:false}@property --skene-height{syntax:\"*\";inherits:false}@property --skene-min-width{syntax:\"*\";inherits:false}@property --skene-min-height{syntax:\"*\";inherits:false}@property --skene-max-width{syntax:\"*\";inherits:false}@property --skene-max-height{syntax:\"*\";inherits:false}@property --skene-aspect-ratio{syntax:\"*\";inherits:false}@property --skene-margin-block{syntax:\"*\";inherits:false}@property --skene-margin-inline{syntax:\"*\";inherits:false}@property --skene-padding-block{syntax:\"*\";inherits:false}@property --skene-padding-inline{syntax:\"*\";inherits:false}@property --skene-offset-block{syntax:\"*\";inherits:false}@property --skene-offset-inline{syntax:\"*\";inherits:false}@property --skene-pinned-block{syntax:\"*\";inherits:false}@property --skene-pinned-inline{syntax:\"*\";inherits:false}@property --skene-background{syntax:\"*\";inherits:false}@property --skene-border-width-block{syntax:\"*\";inherits:false}@property --skene-border-width-inline{syntax:\"*\";inherits:false}@property --skene-border-color{syntax:\"*\";inherits:false}@property --skene-cursor{syntax:\"*\";inherits:false}@property --skene-spacing{syntax:\"*\";inherits:false}@property --skene-wrap{syntax:\"*\";inherits:false}@property --skene-flex{syntax:\"*\";inherits:false}@property --skene-arrangement{syntax:\"*\";inherits:false}@property --skene-item-alignment{syntax:\"*\";inherits:false}@property --skene-line-arrangement{syntax:\"*\";inherits:false}@property --skene-item-horizontal-alignment{syntax:\"*\";inherits:false}@property --skene-self-alignment{syntax:\"*\";inherits:false}@property --skene-self-horizontal-alignment{syntax:\"*\";inherits:false}@property --skene-grid-columns{syntax:\"*\";inherits:false}@property --skene-grid-rows{syntax:\"*\";inherits:false}@property --skene-grid-areas{syntax:\"*\";inherits:false}@property --skene-grid-implicit-columns{syntax:\"*\";inherits:false}@property --skene-grid-implicit-rows{syntax:\"*\";inherits:false}@property --skene-grid-flow{syntax:\"*\";inherits:false}@property --skene-grid-row-placement{syntax:\"*\";inherits:false}@property --skene-grid-column-placement{syntax:\"*\";inherits:false}@property --skene-overflow{syntax:\"*\";inherits:false}@property --skene-hidden{syntax:\"*\";inherits:false}@property --skene-corner-radius{syntax:\"*\";inherits:false}@property --skene-text-color{syntax:\"*\";inherits:false}@property --skene-selection-color{syntax:\"*\";inherits:false}@property --skene-selection-background{syntax:\"*\";inherits:false}@property --skene-decoration{syntax:\"*\";inherits:false}@property --skene-decoration-style{syntax:\"*\";inherits:false}@property --skene-decoration-color{syntax:\"*\";inherits:false}@property --skene-font-size{syntax:\"*\";inherits:false}@property --skene-font-weight{syntax:\"*\";inherits:false}@property --skene-line-height{syntax:\"*\";inherits:false}@property --skene-letter-spacing{syntax:\"*\";inherits:false}@property --typography-ink{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --typography-link-ink{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --typography-selection-ink{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --typography-selection-surface{syntax:\"<color>\";inherits:true;initial-value:transparent}@property --typography-size{syntax:\"<length>\";inherits:true;initial-value:0px}@property --typography-weight{syntax:\"<number>\";inherits:true;initial-value:0}@property --typography-line-height{syntax:\"<number>\";inherits:true;initial-value:0}@property --typography-letter-spacing{syntax:\"<length>\";inherits:true;initial-value:0px}@property --fixture-swatch{syntax:\"<color>\";inherits:true;initial-value:transparent}",
  "@layer skene.style{.skene-9194ab83dca5ec80eafe918a1a809d90{--typography-ink:oklch(0.8452223 0 0 / 1);--typography-link-ink:oklch(0.6940848 0.16008389 262.85895 / 1);--typography-selection-ink:oklch(1 0 0 / 1);--typography-selection-surface:oklch(0.598701 0.19566482 261.77704 / 1);--typography-size:16px;--typography-weight:400;--typography-line-height:1.5;--typography-letter-spacing:0px;}@media (prefers-color-scheme: dark){:where(:root:not([data-skene-scheme=\"light\"])) .skene-9194ab83dca5ec80eafe918a1a809d90{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}}:where(:root[data-skene-scheme=\"dark\"]) .skene-9194ab83dca5ec80eafe918a1a809d90{--typography-selection-ink:oklch(0.17225757 0.0105220685 285.24274 / 1);--typography-selection-surface:oklch(0.7083985 0.14304367 263.9679 / 1);}.skene-f5ca84dc3be8d2ca19984bca66c573ff{background-color:oklch(0.3138341 0.055889927 250.02753 / 1);border-block-width:2px 2px;border-color:oklch(0.5 0.1 200 / 1);border-inline-width:2px 2px;border-radius:6px;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;max-height:800px;min-width:200px;}.skene-d6a57d9564e5cc40582207ddf309d8f9{align-items:stretch;display:flex;flex-direction:column;gap:12px 12px;justify-content:center;padding-block:24px 24px;padding-inline:16px 16px;}.skene-bb2edfc61d71a10d2704779e66f7cd70:where(:empty){min-block-size:var(--skene-line);}.skene-bb2edfc61d71a10d2704779e66f7cd70{color:oklch(0.9234231 0 0 / 1);display:block;font-size:24px;font-weight:700;}.skene-bb2edfc61d71a10d2704779e66f7cd70::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-1f4868b27b7ead5590cd8f15f207ac20:where(:empty){min-block-size:var(--skene-line);}.skene-1f4868b27b7ead5590cd8f15f207ac20{color:var(--typography-link-ink);display:block;}.skene-1f4868b27b7ead5590cd8f15f207ac20::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-c472571f692861c15d28fbba2b8da4e7{background-color:var(--skene-background);cursor:pointer;display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:40px;width:120px;}.skene-e7842b834680d95f5a9f8ab966722001{display:flex;flex-direction:row;gap:4px 4px;}.skene-4ef5bd697e3d7d2c4474163edb55c283:where(:empty){min-block-size:var(--skene-line);}.skene-4ef5bd697e3d7d2c4474163edb55c283{color:var(--typography-ink);display:block;}.skene-4ef5bd697e3d7d2c4474163edb55c283::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-ad0524ef57797d00baa729d0f42983f2{display:grid;gap:4px 4px;grid-template-columns:1fr 1fr 1fr;}.skene-3372e730cc8303e483d94ab118c41731{display:grid;position:relative;}.skene-3372e730cc8303e483d94ab118c41731>*{grid-area:1 / 1;}.skene-22f2f66e5410849080fc4ef06a3c7c13{display:flow-root;}.skene-ca02ab46bf146e54d0d57dec0894642f{color:var(--typography-ink);}.skene-ca02ab46bf146e54d0d57dec0894642f::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-2c7d06b11e0320b500440aa0c0c9d868{color:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-2c7d06b11e0320b500440aa0c0c9d868::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-e32aea12a98a68a45a0da4019652555f{--typography-ink:oklch(0.7311429 0.18611379 52.775707 / 1);}.skene-ccc2bd1c473339f80176733b30b9d7bc{color:var(--typography-ink);font-weight:600;}.skene-ccc2bd1c473339f80176733b30b9d7bc::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-013f7d3b3443ac6dd173672cf7783721{color:var(--typography-ink);display:inline-block;}.skene-013f7d3b3443ac6dd173672cf7783721::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-f10e7dac2b5f563add93f61f18da86cc{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:10px;width:10px;}.skene-f877c18c6b757c6806b1ec647daea8a4{color:var(--typography-link-ink);}.skene-f877c18c6b757c6806b1ec647daea8a4::selection{background-color:var(--typography-selection-surface);color:var(--typography-selection-ink);}.skene-c61f360b61591e752aa58cd67aba72f2{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;padding-block:8px 8px;padding-inline:8px 8px;}@keyframes skene-k-be46dbea7553eec2cc3d8acbc907d23a{0%{border-radius:0px;}100%{border-radius:8px;}}.skene-72e67175860fb8f93ba30c75e785bcb7{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;width:50px;}.skene-72e67175860fb8f93ba30c75e785bcb7:where(:hover){width:60px;}@media (max-width: 480px){.skene-72e67175860fb8f93ba30c75e785bcb7{width:40px;}}@media (prefers-reduced-motion: no-preference){.skene-72e67175860fb8f93ba30c75e785bcb7{animation:skene-k-be46dbea7553eec2cc3d8acbc907d23a 1s ease 0s 1 normal both;transition:width 0.2s ease 0s;}}@media (prefers-reduced-motion: reduce){.skene-72e67175860fb8f93ba30c75e785bcb7{animation:skene-k-be46dbea7553eec2cc3d8acbc907d23a 0s ease 0s 1 normal both;}}:root{--fixture-swatch:oklch(0.61088526 0.12206415 157.55716 / 1);}@media (prefers-color-scheme: dark){:root:where(:not([data-skene-scheme=\"light\"])){--fixture-swatch:oklch(0.37628132 0.07027438 158.34131 / 1);}}:root:where([data-skene-scheme=\"dark\"]){--fixture-swatch:oklch(0.37628132 0.07027438 158.34131 / 1);}.skene-4f9b0a1e748ebc263cb0c8296033e17c{background-color:var(--fixture-swatch);display:grid;grid-template-columns:1fr;grid-template-rows:1fr;height:4px;}}",
  "</style>",
);

/// The body the server leaf renders for [`tree`].
pub(super) const BODY: &str = "<div data-h=\"1\" data-skene-scope class=\"skene-9194ab83dca5ec80eafe918a1a809d90\" style=\"display:contents;\"><div data-h=\"2\" class=\"skene-f5ca84dc3be8d2ca19984bca66c573ff\"><!--s:0--><div data-h=\"3\" class=\"skene-d6a57d9564e5cc40582207ddf309d8f9\"><!--s:1--><span data-h=\"4\" class=\"skene-bb2edfc61d71a10d2704779e66f7cd70\">Title &amp; &lt;escaped&gt;</span><a data-h=\"5\" href=\"https://example.com/?a=1&amp;b=2\" target=\"_blank\" rel=\"noopener noreferrer\" class=\"skene-1f4868b27b7ead5590cd8f15f207ac20\">linked</a><section data-h=\"6\" tabindex=\"0\" class=\"skene-c472571f692861c15d28fbba2b8da4e7\" style=\"--skene-background:oklch(0.5981807 0 0 / 1);\"><!--s:2--><div data-h=\"7\" class=\"skene-e7842b834680d95f5a9f8ab966722001\"><!--s:3--><span data-h=\"8\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">Press</span><!--e:3--></div><!--e:2--></section><div data-h=\"9\" class=\"skene-ad0524ef57797d00baa729d0f42983f2\"><!--s:4--><span data-h=\"10\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">a</span><span data-h=\"11\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">b</span><span data-h=\"12\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">c</span><!--e:4--></div><div data-h=\"13\" class=\"skene-3372e730cc8303e483d94ab118c41731\"><!--s:5--><span data-h=\"14\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">under</span><span data-h=\"15\" class=\"skene-4ef5bd697e3d7d2c4474163edb55c283\">over</span><!--e:5--></div><div data-h=\"16\" class=\"skene-22f2f66e5410849080fc4ef06a3c7c13\"><span data-h=\"17\" class=\"skene-ca02ab46bf146e54d0d57dec0894642f\">plain </span><span data-h=\"18\" class=\"skene-2c7d06b11e0320b500440aa0c0c9d868 skene-e32aea12a98a68a45a0da4019652555f\"><span data-h=\"19\" class=\"skene-ca02ab46bf146e54d0d57dec0894642f\">in </span><span data-h=\"20\" class=\"skene-ccc2bd1c473339f80176733b30b9d7bc\">group</span></span><span data-h=\"21\" class=\"skene-013f7d3b3443ac6dd173672cf7783721\"><div data-h=\"22\" class=\"skene-f10e7dac2b5f563add93f61f18da86cc\"></div></span><a data-h=\"23\" href=\"/rel\" class=\"skene-f877c18c6b757c6806b1ec647daea8a4\"> tail</a></div><div data-h=\"24\" class=\"skene-c61f360b61591e752aa58cd67aba72f2\"></div><div data-h=\"25\" class=\"skene-c61f360b61591e752aa58cd67aba72f2\"></div><div data-h=\"26\" class=\"skene-72e67175860fb8f93ba30c75e785bcb7\"></div><!--s:6--><!--e:6--><div data-h=\"27\" class=\"skene-4f9b0a1e748ebc263cb0c8296033e17c\"></div><!--e:1--></div><!--e:0--></div></div>";

/// The state script the server leaf renders for [`tree`].
pub(super) const STATE_SCRIPT: &str =
  "<script id=\"skene-state\" type=\"application/json\">[[],[],[]]</script>";
