//! HTTP from a view: a resource that fetches the app's own asset through
//! `skene_transport::http`, shown while it loads, once it lands, and again on demand.
//!
//! The client is a crate on the framework's public seams, so the resource that
//! drives it is the ordinary one: `Resource::client` loads on the client alone, since
//! the server render carries the loading caption and the bundle fetches on arrival.
//! The quotes are a manifest-declared asset, served beside the page, so the example
//! needs no network beyond the dev server.

use skene_gui::{HasChild, Resource, prelude::*};
use skene_simple::gui::prelude::*;

/// Where the quotes live: the app's declared asset, served at its URI.
const QUOTES: &str = "/gui-http/assets/quotes.txt";

#[main]
fn app() -> impl IntoView {
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_role(Role::Main)
      .set_color(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .add_child(
            Container::new().set_max_width(460.).set_child(
              Column::new()
                .set_spacing(24.)
                .set_children((heading(), quotes_card())),
            ),
          ),
      )
  })
}

fn heading() -> impl IntoView {
  Column::new().set_spacing(6.).set_children((
    Text::new("HTTP")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A request is a future; a resource shows what it lands.").set_font_size(15.),
  ))
}

/// The quotes fetched into a resource, a line each, with a button fetching them again.
fn quotes_card() -> impl IntoView {
  let quotes: Resource<Result<Vec<String>, String>> = Resource::client(|| async {
    let response = skene_transport::http::get(QUOTES)
      .await
      .map_err(|error| error.to_string())?;

    if !response.is_success() {
      return Err(format!("the server answered {}", response.status()));
    }

    let text = response.text().map_err(|error| error.to_string())?;

    Ok(text.lines().map(str::to_owned).collect())
  });

  SimpleCard::new().set_spacing(12.).set_children((
    Text::new("Quotes").set_font_weight(600.),
    Text::new("Fetched from the app's own asset through skene_transport::http.")
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    derived(move || match quotes.get() {
      None => Text::new("Fetching…")
        .set_color(SimplePalette::MUTED_FOREGROUND)
        .build(),
      Some(Err(error)) => Text::new(format!("Failed: {error}")).build(),
      Some(Ok(lines)) => Column::new()
        .set_spacing(8.)
        .set_children(
          lines
            .into_iter()
            .map(|line| Text::new(line).set_font_size(14.))
            .collect::<Vec<_>>(),
        )
        .build(),
    }),
    SimpleButton::new("Fetch again").handle_click(move || quotes.reload()),
  ))
}
