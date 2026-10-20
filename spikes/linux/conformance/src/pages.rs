//! The suite's pages: views built with skene-gui's shared views alone, each publishing probes
//! of the signals it binds (the only state a scenario reads that a reader does not).

use acme_rating::star_rating;
use skene_gui::{
  Animation, BasicButton, Curve, Keyframe, PointerEvent, PointerPhase, Transition,
  behavior::Activate,
  keys,
  painted::shapes::Rect,
  prelude::*,
  testing::{publish, register as register_page},
};
use skene_simple_gui::{
  SimpleButton, SimpleCheckbox, SimpleDate, SimpleDatePicker, SimplePalette, SimpleToggle,
};

/// Every page's name.
pub const NAMES: &[&str] = &[
  "toggles",
  "text",
  "sliders",
  "picker",
  "box",
  "focus",
  "order",
  "rows",
  "shortcut",
  "scheme",
  "width",
  "library",
  "painted",
  "buttons",
  "checkbox",
  "rings",
  "disabled",
  "simple",
  "accent",
  "fruit",
  "caption",
  "editor",
  "rating",
  "stacking",
  "gradients",
  "motion",
  "keyframes",
  "hover",
  "autofocus",
  "popup",
  "overflow",
  "unchosen",
  "reorder",
  "listbox",
  "lazy",
  "long",
];

/// Registers every page under its name.
pub fn register() {
  register_page("toggles", toggles);
  register_page("text", text);
  register_page("sliders", sliders);
  register_page("picker", picker);
  register_page("box", pressed_box);
  register_page("focus", focus);
  register_page("order", order);
  register_page("rows", rows);
  register_page("shortcut", shortcut);
  register_page("scheme", scheme);
  register_page("width", width);
  register_page("library", library);
  register_page("painted", painted);
  register_page("buttons", buttons);
  register_page("checkbox", checkbox);
  register_page("rings", rings);
  register_page("disabled", disabled);
  register_page("simple", simple);
  register_page("accent", accent);
  register_page("fruit", fruit);
  register_page("caption", caption);
  register_page("editor", editor);
  register_page("rating", rating);
  register_page("stacking", stacking);
  register_page("gradients", gradients);
  register_page("motion", motion);
  register_page("keyframes", keyframes);
  register_page("hover", hover);
  register_page("autofocus", autofocus);
  register_page("popup", popup);
  register_page("overflow", overflow);
  register_page("unchosen", unchosen);
  register_page("reorder", reorder);
  register_page("listbox", listbox);
  register_page("lazy", lazy);
  register_page("long", long);
}

/// The sizes a picker and a radio group choose among.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Size {
  Small,
  Medium,
  Large,
}

impl Size {
  /// Every size and its name, in order.
  const OPTIONS: [(Self, &'static str); 3] = [
    (Self::Small, "Small"),
    (Self::Medium, "Medium"),
    (Self::Large, "Large"),
  ];
}

/// The tones a radio group chooses between.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Tone {
  Warm,
  Cool,
}

/// The fruit a picker and the list boxes choose among, each named as `Debug` writes it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Fruit {
  Apple,
  Banana,
  Cherry,
  Date,
  Elderberry,
}

impl Fruit {
  /// Every fruit, in order.
  const ALL: [Self; 5] = [
    Self::Apple,
    Self::Banana,
    Self::Cherry,
    Self::Date,
    Self::Elderberry,
  ];

  /// Every fruit and its name, in order.
  fn options() -> [(Self, String); 5] {
    Self::ALL.map(|fruit| (fruit, format!("{fruit:?}")))
  }
}

/// Publishes `signal`'s value as the probe `name`, as `Debug` writes it.
fn probe_debug<T: Clone + std::fmt::Debug + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || format!("{:?}", signal.get_untracked()));
}

/// Publishes `signal`'s value as the probe `name`, as `Display` writes it.
fn probe<T: Clone + std::fmt::Display + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || signal.get_untracked().to_string());
}

/// A switch whose handler takes each flip, and one whose handler takes none.
fn toggles() -> impl View {
  let wifi = Signal::new(false);
  let locked = Signal::new(false);
  let asked = Signal::new(0_u32);

  probe("wifi", wifi);
  probe("locked", locked);
  probe("locked asked", asked);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Toggle::new("Wi-Fi").bind_on(wifi))
    .add_child(
      Toggle::new("Locked")
        .derive_on(move || locked.get())
        .handle_change(move |_| asked.update(|asked| *asked += 1)),
    )
}

/// A field that takes what is typed, one that makes each edit upper case without digits, and
/// one that refuses an edit holding an `x`.
fn text() -> impl View {
  let name = Signal::new(String::new());
  let code = Signal::new(String::new());
  let letters = Signal::new(String::new());

  probe("name", name);
  probe("code", code);
  probe("letters", letters);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Name").bind_text(name))
    .add_child(
      TextField::new("Code")
        .derive_text(move || code.get())
        .handle_change(move |proposed| {
          code.set(
            proposed
              .chars()
              .filter(|character| !character.is_ascii_digit())
              .collect::<String>()
              .to_uppercase(),
          );
        }),
    )
    .add_child(
      TextField::new("Letters")
        .derive_text(move || letters.get())
        .handle_change(move |proposed| {
          if !proposed.contains('x') {
            letters.set(proposed);
          }
        }),
    )
}

/// A slider that takes each step, and one whose handler holds it at 40 or below.
fn sliders() -> impl View {
  let volume = Signal::new(30.0_f32);
  let limited = Signal::new(10.0_f32);

  probe("volume", volume);
  probe("limited", limited);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Slider::new("Volume")
        .set_range(0., 100.)
        .set_step(1.)
        .bind_value(volume),
    )
    .add_child(
      Slider::new("Limited")
        .set_range(0., 100.)
        .set_step(1.)
        .derive_value(move || limited.get())
        .handle_change(move |value| limited.set(value.min(40.))),
    )
}

/// A choice of three.
fn picker() -> impl View {
  let size = Signal::new(Size::Small);

  probe_debug("size", size);

  Column::new().set_padding(16).add_child(
    Picker::new("Size")
      .bind_chosen(size)
      .set_options(Size::OPTIONS),
  )
}

/// A box that hears presses, holding a text and a button.
fn pressed_box() -> impl View {
  let presses = Signal::new(0_u32);
  let inside = Signal::new(0_u32);

  probe("box presses", presses);
  probe("inside", inside);

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_padding(16)
      .set_label("Card")
      .handle_pointer(move |event: &PointerEvent| {
        if event.phase == PointerPhase::Up {
          presses.update(|presses| *presses += 1);
        }
      })
      .set_child(
        Row::new()
          .set_spacing(12)
          .set_item_alignment(AxisAlignment::Center)
          .add_child(
            Flexible::new()
              .set_flexibility(1)
              .set_child(Text::new("Card body")),
          )
          .add_child(
            Button::new("Inside").handle_activate(move || inside.update(|inside| *inside += 1)),
          ),
      ),
  )
}

/// Four controls in reading order.
fn focus() -> impl View {
  let first = Signal::new(String::new());
  let second = Signal::new(false);
  let fourth = Signal::new(5.0_f32);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("First").bind_text(first))
    .add_child(Toggle::new("Second").bind_on(second))
    .add_child(Button::new("Third"))
    .add_child(Slider::new("Fourth").bind_value(fourth))
}

/// The swatch's color: white, black where the scheme is dark.
const SWATCH: TokenRef<Color> = TokenRef::new("conformance-swatch");

/// The scheme page's palette.
struct Swatches(Token<Color>);

impl TokenSet for Swatches {
  fn define(&self, definitions: &mut Definitions) {
    definitions.add(&self.0);
  }
}

/// A swatch in a token's color, and a field holding what was typed into it.
fn scheme() -> impl View {
  let kept = Signal::new(String::new());

  probe("kept", kept);

  Theme::new(Swatches(
    Token::new(SWATCH, Color::WHITE).when(Media::Dark, Color::BLACK),
  ))
  .set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .add_child(
        Container::new()
          .set_label("Swatch")
          .set_width(60)
          .set_height(60)
          .set_background(SWATCH),
      )
      .add_child(TextField::new("Kept").bind_text(kept)),
  )
}

/// A frame whose padding is a width condition, and a text that follows the width.
fn width() -> impl View {
  let wide = media::watch(Media::MinWidth(600));

  Container::new()
    .set_label("Frame")
    .set_padding(8.when(Media::MinWidth(600), 32))
    .set_child(
      Column::new()
        .set_spacing(4)
        .add_child(Container::new().set_label("Inner").set_height(20))
        .add_child(Text::derive(
          move || if wide.get() { "wide" } else { "narrow" },
        )),
    )
}

/// A field, then three buttons in a grid that draws the scene's second before its first.
fn order() -> impl View {
  let start = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(
      Grid::new()
        .set_spacing(8)
        .set_columns([GridTrack::Auto, GridTrack::Auto, GridTrack::Auto])
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 2))
            .set_child(Button::new("One")),
        )
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 1))
            .set_child(Button::new("Two")),
        )
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 3))
            .set_child(Button::new("Three")),
        ),
    )
}

/// An activatable row holding a text and a switch, after a field.
fn rows() -> impl View {
  let before = Signal::new(String::new());
  let opened = Signal::new(0_u32);
  let muted = Signal::new(false);

  probe("opened", opened);
  probe("muted", muted);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Before").bind_text(before))
    .add_child(
      Container::new()
        .set_padding(12)
        .set_label("Inbox")
        .handle_click(move || opened.update(|opened| *opened += 1))
        .set_child(
          Row::new()
            .set_spacing(12)
            .set_item_alignment(AxisAlignment::Center)
            .add_child(
              Flexible::new()
                .set_flexibility(1)
                .set_child(Text::new("Inbox")),
            )
            .add_child(
              Toggle::new("Muted")
                .set_accessible_name("Mute Inbox")
                .bind_on(muted),
            ),
        ),
    )
}

/// A field, and a button its shortcut (`Ctrl+S`) activates wherever focus is.
fn shortcut() -> impl View {
  let title = Signal::new(String::new());
  let saved = Signal::new(0_u32);

  probe("saved", saved);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Title").bind_text(title))
    .add_child(
      Button::new("Save")
        .set_shortcut(Shortcut::primary('s'))
        .handle_activate(move || saved.update(|saved| *saved += 1)),
    )
}

/// A library's date picker (Simple's), controlled, that refuses weekends.
fn library() -> impl View {
  let due = Signal::new("2026-10-14".to_owned());
  let picks = Signal::new(0_u32);

  probe("due", due);
  probe("picks", picks);

  Column::new().set_padding(16).add_child(
    SimpleDatePicker::new("Due")
      .derive_date(move || due.get())
      .handle_change(move |date| {
        picks.update(|picks| *picks += 1);

        if SimpleDate::parse(&date).is_some_and(|date| !date.is_weekend()) {
          due.set(date);
        }
      }),
  )
}

/// A described picture, and one with no description.
fn painted() -> impl View {
  let paint = |canvas: &mut Canvas<'_>| {
    let (width, height) = (canvas.size().width, canvas.size().height);

    canvas.fill(&Rect::new(0., 0., width / 2., height), Color::rgb(0xff0000));
    canvas.fill(
      &Rect::new(width / 2., 0., width, height),
      Color::rgb(0x0000ff),
    );
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Painted::new(paint)
        .set_width(200)
        .set_height(50)
        .set_description("Sales by half"),
    )
    .add_child(Painted::new(paint).set_width(200).set_height(50))
}

/// A field, then a button counting its activations.
fn buttons() -> impl View {
  let start = Signal::new(String::new());
  let acts = Signal::new(0_u32);

  probe("acts", acts);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Button::new("Act").handle_activate(move || acts.update(|acts| *acts += 1)))
}

/// A checkbox, alone in a row so its caption is as wide as its text.
fn checkbox() -> impl View {
  let remember = Signal::new(false);

  probe("remember", remember);

  Column::new()
    .set_padding(16)
    .add_child(Row::new().add_child(Checkbox::new("Remember me").bind_checked(remember)))
}

/// A field, a button showing the default focus ring, and one whose ring is none.
fn rings() -> impl View {
  let start = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(16)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Row::new().add_child(Button::new("Ringed")))
    .add_child(Row::new().add_child(Button::new("Plain").set_ring(Ring::NONE)))
}

/// A field, a disabled checkbox, and an enabled one.
fn disabled() -> impl View {
  let start = Signal::new(String::new());
  let off = Signal::new(false);
  let on = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Checkbox::new("Off").set_disabled(true).bind_checked(off))
    .add_child(Checkbox::new("On").bind_checked(on))
}

/// Simple's button, checkbox and toggle, and a plain switch, beneath Simple's palette.
fn simple() -> impl View {
  let agree = Signal::new(true);
  let sync = Signal::new(true);
  let plain = Signal::new(true);

  Theme::new(SimplePalette::default()).set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .add_child(Row::new().add_child(SimpleButton::new("Save")))
      .add_child(Row::new().add_child(SimpleCheckbox::new("Agree").bind_checked(agree)))
      .add_child(SimpleToggle::new("Sync").bind_on(sync))
      .add_child(Toggle::new("Plain").bind_on(plain)),
  )
}

/// A theme defining the platform's accent, orange or green.
struct Accent(Color);

impl TokenSet for Accent {
  fn define(&self, definitions: &mut Definitions) {
    definitions.add(&Token::new(SystemColors::ACCENT, self.0));
  }
}

/// A switch, a checkbox, a full slider and a field beneath a theme redefining the accent, and
/// a button flipping it from orange to green.
fn accent() -> impl View {
  let green = Signal::new(false);
  let sync = Signal::new(true);
  let mail = Signal::new(true);
  let level = Signal::new(1.0_f32);
  let note = Signal::new(String::new());

  probe("note", note);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Button::new("Flip accent").handle_activate(move || green.update(|green| *green = !*green)),
    )
    .add_child(
      Theme::derive(move || {
        Accent(if green.get() {
          Color::rgb(0x34c759)
        } else {
          Color::rgb(0xff9500)
        })
      })
      .set_child(
        Column::new()
          .set_spacing(12)
          .add_child(Toggle::new("Sync").bind_on(sync))
          .add_child(Row::new().add_child(Checkbox::new("Mail").bind_checked(mail)))
          .add_child(Slider::new("Level").bind_value(level))
          .add_child(TextField::new("Note").bind_text(note)),
      ),
    )
}

/// A choice of five fruit, in a row so the picker is as wide as it likes.
fn fruit() -> impl View {
  let fruit = Signal::new(Fruit::Banana);

  probe_debug("fruit", fruit);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Text::new("Pick one"))
    .add_child(
      Row::new().add_child(
        Picker::new("Fruit")
          .bind_chosen(fruit)
          .set_options(Fruit::options()),
      ),
    )
}

/// A field whose caption sits beside it.
fn caption() -> impl View {
  let email = Signal::new(String::new());

  probe("email", email);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Email").bind_text(email))
}

/// An editor 120 pixels tall at most.
fn editor() -> impl View {
  let notes = Signal::new(String::new());

  Column::new().set_padding(16).set_spacing(12).add_child(
    TextEditor::new("Notes")
      .set_max_height(120)
      .bind_text(notes),
  )
}

/// A library's drawn control (acme-rating's), between two buttons, and a disabled copy.
fn rating() -> impl View {
  let stars = Signal::new(0_u8);
  let locked = Signal::new(4_u8);

  probe("stars", stars);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Before"))
    .add_child(Row::new().add_child(star_rating("Rating", stars)))
    .add_child(Row::new().add_child(star_rating("Locked", locked).set_disabled(true)))
    .add_child(Button::new("After"))
}

/// A text in red under an opaque blue box covering its start, and the same text under a
/// black scrim at half opacity covering all of it: what a stack draws later covers what it
/// draws before.
fn stacking() -> impl View {
  let words = || {
    Text::new("MMMMMMMM")
      .set_font_size(30)
      .set_color(Color::rgb(0xff0000))
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .set_item_alignment(AxisAlignment::Start)
    .add_child(
      Container::new().set_label("Covered").set_child(
        Stack::new().add_child(words()).add_child(
          Container::new()
            .set_label("Cover")
            .set_width(100)
            .set_height(40)
            .set_background(Color::rgb(0x0000ff)),
        ),
      ),
    )
    .add_child(
      Container::new().set_label("Dimmed").set_child(
        Stack::new().add_child(words()).add_child(
          Pinned::new().set_edges(Edges::all(0)).set_child(
            Container::new()
              .set_label("Scrim")
              .set_background(Color::rgba(0x0000_0080)),
          ),
        ),
      ),
    )
}

/// A box filled along a line, one filled out from its center, one filled around it, one
/// whose border runs along a line, and one whose gradient starts at a token's color.
fn gradients() -> impl View {
  let red = Color::rgb(0xff0000);
  let blue = Color::rgb(0x0000ff);
  let swatch = |name: &str| {
    Container::new()
      .set_label(name)
      .set_width(200)
      .set_height(60)
  };

  Theme::new(Swatches(
    Token::new(SWATCH, Color::WHITE).when(Media::Dark, Color::BLACK),
  ))
  .set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .set_item_alignment(AxisAlignment::Start)
      .add_child(
        swatch("Linear").set_background(
          LinearGradient::new(Angle::degrees(90.))
            .add_stop(0., red)
            .add_stop(1., blue),
        ),
      )
      .add_child(
        swatch("Radial").set_height(120).set_background(
          RadialGradient::new()
            .add_stop(0., Color::WHITE)
            .add_stop(1., Color::BLACK),
        ),
      )
      .add_child(
        swatch("Sweep").set_height(120).set_background(
          SweepGradient::new()
            .add_stop(0., red)
            .add_stop(0.5, blue)
            .add_stop(1., red),
        ),
      )
      .add_child(
        swatch("Bordered")
          .set_background(Color::WHITE)
          .set_border(Border::new(8))
          .set_border_color(
            LinearGradient::new(Angle::degrees(90.))
              .add_stop(0., red)
              .add_stop(1., blue),
          ),
      )
      .add_child(
        swatch("Themed").set_background(
          LinearGradient::new(Angle::degrees(90.))
            .add_stop(0., SWATCH)
            .add_stop(1., red),
        ),
      ),
  )
}

/// How long the motion pages' transitions and animations take, in milliseconds.
pub const MOTION: u32 = 400;

/// A fill moving linearly over [`MOTION`] milliseconds.
const fn moving() -> Transition {
  Transition::new(keys::BACKGROUND, MOTION).set_curve(Curve::Linear)
}

/// Three boxes whose fills move from white under a transition, each from a change of its own
/// source: a condition flipping (the pointer over the first turns it black), a token
/// redefined (a button switches the theme's swatch to black) and a derived value written (a
/// button turns the third red, and the text of a fourth box from black to red). Probes count
/// the third's derivations and the theme's.
fn motion() -> impl View {
  let dark = Signal::new(false);
  let warm = Signal::new(false);
  let derived = Signal::new(0_u32);
  let themed = Signal::new(0_u32);
  let swatch = |name: &str| {
    Container::new()
      .set_label(name)
      .set_width(120)
      .set_height(40)
      .add_transition(moving())
  };

  probe("derived", derived);
  probe("themed", themed);

  Theme::derive(move || {
    themed.update(|count| *count += 1);
    Swatches(Token::new(
      SWATCH,
      if dark.get() {
        Color::BLACK
      } else {
        Color::WHITE
      },
    ))
  })
  .set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .set_item_alignment(AxisAlignment::Start)
      .add_child(swatch("Hovered").set_background(Color::WHITE.when(State::Hovered, Color::BLACK)))
      .add_child(swatch("Themed").set_background(SWATCH))
      .add_child(swatch("Derived").derive_background(move || {
        derived.update(|count| *count += 1);

        if warm.get() {
          Color::rgb(0xff0000)
        } else {
          Color::WHITE
        }
      }))
      .add_child(
        Container::new()
          .set_label("Inked")
          .derive_color(move || {
            if warm.get() {
              Color::rgb(0xff0000)
            } else {
              Color::BLACK
            }
          })
          .add_transition(Transition::new(keys::COLOR, MOTION).set_curve(Curve::Linear))
          .set_child(Text::new("MMM").set_font_size(32)),
      )
      .add_child(Button::new("Retheme").handle_activate(move || dark.update(|dark| *dark = !*dark)))
      .add_child(Button::new("Warm").handle_activate(move || warm.update(|warm| *warm = !*warm))),
  )
}

/// A box that, from when it is first shown, fades its fill from black to its own white over
/// [`MOTION`] milliseconds, once, as a keyframe animation.
fn keyframes() -> impl View {
  Column::new().set_padding(16).add_child(
    Container::new()
      .set_label("Fading")
      .set_width(120)
      .set_height(40)
      .set_background(Color::WHITE)
      .add_animation(
        Animation::new(MOTION)
          .set_curve(Curve::Linear)
          .add_keyframe(0., Keyframe::new().set(keys::BACKGROUND, Color::BLACK)),
      ),
  )
}

/// A checkbox, a picker whose caption sits beside it, a radio group, and a box outside every
/// control whose fill changes under the pointer.
fn hover() -> impl View {
  let remember = Signal::new(false);
  let size = Signal::new(Size::Small);
  let tone = Signal::new(Tone::Warm);

  Column::new()
    .set_padding(16)
    .set_spacing(16)
    .add_child(Row::new().add_child(Checkbox::new("Remember me").bind_checked(remember)))
    .add_child(
      Row::new().add_child(
        Picker::new("Size")
          .bind_chosen(size)
          .set_options(Size::OPTIONS),
      ),
    )
    .add_child(
      RadioGroup::new("Tone")
        .bind_chosen(tone)
        .add_option(Tone::Warm, "Warm")
        .add_option(Tone::Cool, "Cool"),
    )
    .add_child(
      Row::new().add_child(
        Container::new()
          .set_label("Plain box")
          .set_width(120)
          .set_height(32)
          .set_padding(Inset::new(0, 0, 0, 40))
          .set_background(Value::from(Color::WHITE).when(State::Hovered, Color::rgb(0x00a0ff)))
          .set_child(Text::new("Plain")),
      ),
    )
}

/// Two fields, the second asking for focus as the window opens.
fn autofocus() -> impl View {
  let first = Signal::new(String::new());
  let second = Signal::new(String::new());

  probe("second", second);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("First").bind_text(first))
    .add_child(
      TextField::new("Second")
        .set_autofocus(true)
        .bind_text(second),
    )
}

/// A button opening a popup whose field asks for focus as it opens, and whose button closes
/// it.
fn popup() -> impl View {
  let open = Signal::new(false);
  let note = Signal::new(String::new());
  let opener = NodeHandle::new();

  probe("note", note);
  probe("open", open);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Row::new().add_child(
        Button::new("Open")
          .set_handle(&opener)
          .handle_activate(move || open.set(true)),
      ),
    )
    .add_child(
      Overlay::popup()
        .set_anchor(&opener)
        .bind_open(open)
        .set_child(
          Container::new()
            .set_padding(12)
            .set_background(SystemColors::FIELD)
            .set_child(
              Column::new()
                .set_spacing(8)
                .add_child(TextField::new("Note").set_autofocus(true).bind_text(note))
                .add_child(Button::new("Done").handle_activate(move || open.set(false))),
            ),
        ),
    )
}

/// A command whose mark is drawn below its box, the same command inside a box that clips,
/// and a round box over a square one.
fn overflow() -> impl View {
  let over = Signal::new(0_u32);
  let clipped = Signal::new(0_u32);
  let beneath = Signal::new(0_u32);
  let round = Signal::new(0_u32);

  probe("overflowing", over);
  probe("clipped", clipped);
  probe("beneath", beneath);
  probe("round", round);

  let command = |name: &str, count: Signal<u32>| {
    BasicButton::new(Activate::new().handle_activate(move || count.update(|count| *count += 1)))
      .set_height(20)
      .set_spacing(8)
      .add_part(
        Part::Mark,
        Container::new()
          .set_width(40)
          .set_height(20)
          .set_background(SystemColors::ACCENT)
          .set_offset(Offset::new(0, 20)),
      )
      .set_label(name)
  };

  Column::new()
    .set_padding(16)
    .set_spacing(48)
    .add_child(Row::new().add_child(command("Overflowing", over)))
    .add_child(
      Row::new().add_child(
        Container::new()
          .set_clip(true)
          .set_child(command("Clipped", clipped)),
      ),
    )
    .add_child(
      Row::new().add_child(
        Stack::new()
          .add_child(
            Container::new()
              .set_label("Beneath")
              .set_width(60)
              .set_height(60)
              .set_background(SystemColors::CONTROL)
              .handle_click(move || beneath.update(|count| *count += 1)),
          )
          .add_child(
            Container::new()
              .set_label("Round")
              .set_width(18)
              .set_height(18)
              .set_corner_radius(9)
              .set_background(SystemColors::ACCENT)
              .handle_click(move || round.update(|count| *count += 1)),
          ),
      ),
    )
}

/// A picker holding a fruit, or none, its options every fruit and no option for none.
fn unchosen() -> impl View {
  let fruit = Signal::new(None::<Fruit>);

  probe_debug("fruit", fruit);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Text::new("Pick one"))
    .add_child(
      Row::new().add_child(Picker::new("Fruit").bind_chosen(fruit).derive_options(
        || Fruit::ALL.map(Some).to_vec(),
        |fruit| format!("{:?}", fruit.expect("every option carries a fruit")),
      )),
    )
}

/// A picker whose options are derived from a list that Control+R reverses: "Reverse", a button
/// the shortcut clicks wherever focus is.
fn reorder() -> impl View {
  let order = Signal::new(Fruit::ALL.to_vec());
  let fruit = Signal::new(Fruit::Banana);

  publish("order", move || format!("{:?}", order.get_untracked()));
  probe_debug("fruit", fruit);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Button::new("Reverse")
        .set_shortcut(Shortcut::primary('r'))
        .handle_activate(move || order.update(|order| order.reverse())),
    )
    .add_child(
      Row::new().add_child(
        Picker::new("Fruit")
          .bind_chosen(fruit)
          .derive_options(move || order.get(), |fruit| format!("{fruit:?}")),
      ),
    )
}

/// A list box choosing one fruit or none, and one choosing several.
fn listbox() -> impl View {
  let favorite = Signal::new(None::<Fruit>);
  let basket = Signal::new(std::collections::BTreeSet::<Fruit>::new());

  probe_debug("favorite", favorite);
  probe_debug("basket", basket);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Listbox::new("Favorite")
        .bind_chosen(favorite)
        .set_options(Fruit::options()),
    )
    .add_child(
      Listbox::new("Basket")
        .bind_chosen(basket)
        .set_options(Fruit::options()),
    )
}

/// A lazy list of ten thousand rows 40 pixels tall, 300 pixels of them in view, each holding a
/// button "Open {n}", between the buttons "Before" and "After".
fn lazy() -> impl View {
  Column::new()
    .set_padding(16)
    .add_child(Button::new("Before"))
    .add_child(
      LazyColumn::new()
        .set_height(300)
        .set_label("Actions")
        .set_item_extent(ItemExtent::Estimated(40.))
        .derive_items(
          || (0..10_000).collect::<Vec<usize>>(),
          |index| *index,
          |index: usize| {
            Container::new()
              .set_height(40)
              .set_child(Button::new(format!("Open {index}")))
          },
        ),
    )
    .add_child(Button::new("After"))
}

/// A list box choosing several of five thousand numbered options, "Number 0" onwards, how
/// many it has chosen published as "chosen".
fn long() -> impl View {
  let chosen = Signal::new(std::collections::HashSet::<u32>::new());

  publish("chosen", move || {
    chosen.with(|chosen| chosen.len().to_string())
  });

  Column::new().set_padding(16).add_child(
    Listbox::new("Numbers").bind_chosen(chosen).derive_options(
      || (0..5000).collect::<Vec<u32>>(),
      |value: &u32| format!("Number {value}"),
    ),
  )
}
