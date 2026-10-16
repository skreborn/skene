//! Hello: the first app on skene-gui's Linux realization.
//!
//! Nothing here names a platform's build: one `match` on `Platform::CURRENT` picks the footer,
//! and every view is a shared one.

use skene_gui::prelude::*;

/// The accent color.
pub const ACCENT: TokenRef<Color> = TokenRef::new("hello-accent");
/// The card's fill.
pub const CARD: TokenRef<Color> = TokenRef::new("hello-card");
/// The page behind the card.
pub const PAGE: TokenRef<Color> = TokenRef::new("hello-page");
/// The text's color.
pub const INK: TokenRef<Color> = TokenRef::new("hello-ink");
/// The space inside the card.
pub const GAP: TokenRef<f32> = TokenRef::new("hello-gap");

/// Hello's palette, light and dark in each definition.
pub struct Palette {
  accent: Token<Color>,
  card: Token<Color>,
  page: Token<Color>,
  ink: Token<Color>,
  gap: Token<f32>,
}

impl Default for Palette {
  fn default() -> Self {
    Self {
      accent: Token::new(ACCENT, Color::rgb(0x34c759)).when(Media::Dark, Color::rgb(0x30d158)),
      card: Token::new(CARD, Color::WHITE).when(Media::Dark, Color::rgb(0x1c1c1e)),
      page: Token::new(PAGE, Color::rgb(0xf2f2f7)).when(Media::Dark, Color::BLACK),
      ink: Token::new(INK, Color::rgb(0x1d1d1f)).when(Media::Dark, Color::rgb(0xf5f5f7)),
      gap: Token::new(GAP, 12),
    }
  }
}

impl TokenSet for Palette {
  fn define(&self, definitions: &mut Definitions) {
    definitions
      .add(&self.accent)
      .add(&self.card)
      .add(&self.page)
      .add(&self.ink)
      .add(&self.gap);
  }
}

/// What the greeting says for `name`.
#[must_use]
pub fn greeting(name: &str) -> String {
  match name.trim() {
    "" => "Who's there?".to_owned(),
    name => format!("Hello, {name}!"),
  }
}

/// The app.
pub fn app() -> impl View {
  let name = Signal::new(String::new());
  let greeted = Signal::new(Vec::<String>::new());
  let sync = Signal::new(true);
  let volume = Signal::new(0.5);
  let size = Signal::new(Some(1));
  let footer = match Platform::CURRENT {
    Platform::Web => "Served as markup, hydrated in place.",
    _ => "Drawn by Skene.",
  };

  Theme::new(Palette::default()).set_child(
    Container::new()
      .set_background(PAGE)
      .set_padding(16.when(Media::MinWidth(900), 48))
      .set_alignment(Alignment::new(AxisAlignment::Stretch, AxisAlignment::Start))
      .set_child(
        Container::new()
          .set_padding(24.when(Media::MinWidth(900), 48))
          .set_background(CARD)
          .set_corner_radius(16)
          .set_shadow(Shadow::new(2, 12, Color::rgba(0x0000_0024)))
          .set_child(
            Column::new()
              .set_spacing(GAP)
              .add_child(
                Row::new()
                  .set_spacing(12)
                  .set_item_alignment(AxisAlignment::Center)
                  .add_child(
                    Container::new()
                      .set_width(32)
                      .set_height(32)
                      .set_corner_radius(16)
                      .set_background(ACCENT),
                  )
                  .add_child(
                    Text::new("Hello, Skene")
                      .set_font_size(28)
                      .set_font_weight(FontWeight::BOLD)
                      .set_color(INK),
                  ),
              )
              .add_child(Text::derive(move || greeting(&name.get())).set_color(INK))
              .add_child(
                TextField::new("Name")
                  .set_placeholder("Ada")
                  .bind_text(name),
              )
              .add_child(Toggle::new("Sync").bind_on(sync))
              .add_child(
                Text::derive(move || {
                  if sync.get() {
                    "Sync is on"
                  } else {
                    "Sync is off"
                  }
                })
                .set_color(ACCENT),
              )
              .add_child(
                Toggle::new("Dark")
                  .derive_on(|| Environment::current().dark.get())
                  .handle_change(|dark| {
                    media::set_scheme(Some(if dark { Scheme::Dark } else { Scheme::Light }));
                  }),
              )
              .add_child(Slider::new("Volume").set_step(0.1).bind_value(volume))
              .add_child(
                Picker::new("Size")
                  .set_options(["Small", "Medium", "Large"])
                  .bind_chosen(size),
              )
              .add_child(
                Row::new()
                  .set_spacing(8)
                  .add_child(
                    Flexible::new().set_flexibility(1).set_child(
                      Button::new("Greet")
                        .set_prominent(true)
                        .handle_activate(move || {
                          let who = name.get();

                          if !who.trim().is_empty() && !greeted.get().contains(&who) {
                            greeted.update(|greeted| greeted.push(who));
                          }
                        }),
                    ),
                  )
                  .add_child(Flexible::new().set_flexibility(1).set_child(
                    Button::new("Reset").handle_activate(move || {
                      name.set(String::new());
                      greeted.set(Vec::new());
                      volume.set(0.5);
                    }),
                  )),
              )
              .add_child(Text::new(footer).set_font_size(13).set_color(INK))
              .derive_children(
                move || greeted.get(),
                Clone::clone,
                |who| {
                  Text::new(format!("Greeted {who}"))
                    .set_font_weight(FontWeight::SEMIBOLD)
                    .set_color(INK)
                },
              ),
          ),
      ),
  )
}
