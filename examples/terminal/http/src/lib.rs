//! HTTP from a terminal view: a task fetches a URL through `skene_transport::http` on a thread
//! of its own, and the loop's executor wakes the view when the answer lands.
//!
//! `spawn` runs the request as a task bound to the view's scope; the signal it writes
//! drives the text below. The status, the content type and the body's first lines
//! are shown; a failure is shown in its place.
//!
//! `terminal-http [url]` fetches the URL (`https://example.com/` without one) and shows
//! what came back. `Ctrl-Q` quits.

#![cfg(not(target_arch = "wasm32"))]

use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

/// The URL fetched when none is given.
const DEFAULT_URL: &str = "https://example.com/";

/// What the fetch has come to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Fetch {
  Pending,
  Failed(String),
  Landed {
    status: u16,
    kind: Option<String>,
    lines: Vec<String>,
  },
}

/// The page: the URL, and what fetching it came to.
#[must_use]
pub fn app(url: Option<String>) -> impl View {
  let url = url.unwrap_or_else(|| DEFAULT_URL.to_owned());
  let fetch = Signal::new(Fetch::Pending);

  spawn({
    let url = url.clone();

    async move {
      let outcome = match skene_transport::http::get(url).await {
        Err(error) => Fetch::Failed(error.to_string()),
        Ok(response) => Fetch::Landed {
          status: response.status(),
          kind: response.header("content-type").map(str::to_owned),
          lines: response
            .text()
            .unwrap_or_else(|_| "(not text)".to_owned())
            .lines()
            .take(6)
            .map(str::to_owned)
            .collect(),
        },
      };

      fetch.set(outcome);
    }
  });

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_overflow(Overflow::ScrollIfNeeded)
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Container::new().set_max_width(72).set_child(
          Column::new()
            .set_spacing(1)
            .set_padding(Inset::symmetric(2, 1))
            .set_children((
              Text::new("HTTP")
                .set_bold(true)
                .set_color(SimplePalette::MUTED_FOREGROUND),
              Text::new(format!("GET {url}")).set_color(SimplePalette::FOREGROUND),
              derived(move || match fetch.get() {
                Fetch::Pending => Text::new("Fetching…")
                  .set_color(SimplePalette::MUTED_FOREGROUND)
                  .build(),
                Fetch::Failed(error) => Text::new(format!("Failed: {error}"))
                  .set_color(SimplePalette::FOREGROUND)
                  .build(),
                Fetch::Landed {
                  status,
                  kind,
                  lines,
                } => Column::new()
                  .set_spacing(1)
                  .set_children((
                    Text::new(format!(
                      "{status} {}",
                      kind.unwrap_or_else(|| "(no content type)".to_owned())
                    ))
                    .set_color(SimplePalette::FOREGROUND),
                    Column::new().set_children(
                      lines
                        .into_iter()
                        .map(|line| Text::new(line).set_color(SimplePalette::MUTED_FOREGROUND))
                        .collect::<Vec<_>>(),
                    ),
                  ))
                  .build(),
              }),
            )),
        ),
      )
  })
}

/// The page over the URL the first argument names.
#[main]
fn from_arguments() -> impl View {
  app(std::env::args().nth(1))
}
