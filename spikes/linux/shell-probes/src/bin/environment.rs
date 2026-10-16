//! The reader's environment as the realization reports it: the scheme, the contrast, reduced
//! motion and the accent, as texts, and a swatch whose fill answers the scheme and the
//! contrast.

use skene_gui::prelude::*;

#[skene_gui::main]
fn app() -> impl View {
  let scheme = media::scheme();
  let contrast = media::high_contrast();
  let motion = media::reduced_motion();
  let accent = Environment::current().accent;

  Container::new()
    .set_padding(16)
    .set_background(
      Color::rgb(0xffffff)
        .when(Media::Dark, Color::rgb(0x1c1c1e))
        .when(Media::HighContrast, Color::rgb(0xffff00)),
    )
    .set_alignment(Alignment::new(AxisAlignment::Stretch, AxisAlignment::Start))
    .set_child(
      Column::new()
        .set_spacing(8)
        .add_child(Text::derive(move || format!("Scheme: {:?}", scheme.get())))
        .add_child(Text::derive(move || {
          format!("High contrast: {}", contrast.get())
        }))
        .add_child(Text::derive(move || {
          format!("Reduced motion: {}", motion.get())
        }))
        .add_child(Text::derive(move || match accent.get() {
          Some(color) => format!(
            "Accent: #{:02x}{:02x}{:02x}",
            color.red, color.green, color.blue
          ),
          None => "Accent: none".to_owned(),
        }))
        .add_child(Toggle::new("Accent switch").set_on(true))
        .add_child(
          Container::new()
            .set_height(40)
            .set_corner_radius(8)
            .set_background(
              Color::rgb(0x34c759)
                .when(Media::Dark, Color::rgb(0x30d158))
                .when(Media::HighContrast, Color::rgb(0x000000)),
            ),
        ),
    )
}
