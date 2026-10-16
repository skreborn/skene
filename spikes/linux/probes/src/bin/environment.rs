//! The environment on real GTK, as `skene_gui::media` reports it.
//!
//! `environment portal`: run beside `portal_stub 1 0 contrast=1@2500 color-scheme=2@3500`, the
//! reader's scheme and contrast come from the XDG settings portal and follow its
//! `SettingChanged`. `environment settings`: no portal, `gtk-application-prefer-dark-theme=1`
//! in settings.ini, so they come from `GtkSettings`. `environment scale`: under `GDK_SCALE=2`.

use std::time::{Duration, Instant};

use probes::{hex, near, result, settle, show, wait};
use skene_gui::{
  platform::linux::gtk::{self, prelude::*},
  prelude::*,
};

/// Runs the loop until `holds` or `timeout` passes, returning whether it held.
fn wait_for(timeout: Duration, holds: impl Fn() -> bool) -> bool {
  let start = Instant::now();

  while !holds() {
    if start.elapsed() > timeout {
      return false;
    }

    wait(20);
  }

  true
}

fn prefers_dark() -> bool {
  gtk::Settings::default().is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme())
}

fn portal() {
  let (scheme, contrast) = (media::scheme(), media::high_contrast());

  result(
    "the portal's color-scheme 1 is the reader's dark",
    scheme.get() == Scheme::Dark && prefers_dark(),
    format!(
      "scheme {:?}, gtk-application-prefer-dark-theme {}",
      scheme.get(),
      prefers_dark()
    ),
  );

  media::set_scheme(Some(Scheme::Light));
  settle();

  let chosen = (scheme.get(), prefers_dark());

  media::set_scheme(None);
  settle();

  result(
    "set_scheme(Some(Light)), then None follows the reader's dark again",
    chosen == (Scheme::Light, false) && scheme.get() == Scheme::Dark && prefers_dark(),
    format!("chosen {chosen:?}, then {:?}", scheme.get()),
  );

  let swatch = NodeHandle::new();
  let shown = show(200, -1, || {
    Container::new()
      .set_height(30)
      .set_background(Value::from(Color::WHITE).when(Media::HighContrast, Color::rgb(0x0000ff)))
      .set_handle(&swatch)
  });
  let before = (contrast.get(), shown.pixel_in(&swatch, 5., 5.));
  let changed = wait_for(Duration::from_secs(5), move || contrast.get());

  settle();

  let after = shown.pixel_in(&swatch, 5., 5.);

  result(
    "the portal's live contrast change reaches Media::HighContrast",
    !before.0 && near(before.1, 0xffffff) && changed && near(after, 0x0000ff),
    format!(
      "{} then {} after SettingChanged contrast=1",
      hex(before.1),
      hex(after)
    ),
  );

  let light = wait_for(Duration::from_secs(5), move || {
    scheme.get() == Scheme::Light
  });

  result(
    "the portal's live color-scheme 2 is the reader's light",
    light && !prefers_dark(),
    format!(
      "scheme {:?}, gtk-application-prefer-dark-theme {}",
      scheme.get(),
      prefers_dark()
    ),
  );

  let dark = Signal::new(true);

  media::derive_scheme(move || dark.get().then_some(Scheme::Dark));
  settle();

  let derived = scheme.get();

  dark.set(false);
  settle();

  result(
    "derive_scheme chooses and lets go as its signal says",
    derived == Scheme::Dark && scheme.get() == Scheme::Light,
    format!("{derived:?}, then {:?}", scheme.get()),
  );

  shown.close();
}

fn settings() {
  let scheme = media::scheme();
  let settings = gtk::Settings::default().expect("settings");

  result(
    "no portal: GtkSettings' prefer-dark is the reader's dark",
    scheme.get() == Scheme::Dark,
    format!("{:?}", scheme.get()),
  );

  media::set_scheme(Some(Scheme::Light));
  settle();

  let chosen = (scheme.get(), prefers_dark());

  media::set_scheme(None);
  settle();

  result(
    "set_scheme(None) follows the reader's dark again, not light",
    chosen == (Scheme::Light, false) && scheme.get() == Scheme::Dark && prefers_dark(),
    format!("chosen {chosen:?}, then {:?}", scheme.get()),
  );

  settings.set_gtk_application_prefer_dark_theme(false);
  settle();

  result(
    "the reader's own change of GtkSettings is followed",
    scheme.get() == Scheme::Light,
    format!("{:?}", scheme.get()),
  );

  let reduced = media::reduced_motion();
  let before = reduced.get();

  settings.set_gtk_enable_animations(false);
  settle();

  let after = reduced.get();

  settings.set_gtk_enable_animations(true);
  settle();

  result(
    "reduced motion follows gtk-enable-animations",
    !before && after && !reduced.get(),
    format!("{before} -> {after} -> {}", reduced.get()),
  );

  let (width, height, wide) = (
    media::width(),
    media::height(),
    media::watch(Media::MinWidth(500)),
  );
  let shown = show(400, 300, || Text::new("Size"));
  let small = (width.get(), height.get(), wide.get());

  shown.resize(640, 480);

  result(
    "width, height and a watched condition follow the window",
    small == (400., 300., false) && (width.get(), height.get(), wide.get()) == (640., 480., true),
    format!(
      "{small:?} then {:?}",
      (width.get(), height.get(), wide.get())
    ),
  );
  result(
    "the scale is 1 without GDK_SCALE",
    media::scale().get() == 1.,
    media::scale().get(),
  );

  shown.close();
}

fn scale() {
  let scale = media::scale();
  let shown = show(200, 100, || Text::new("Twice"));

  result(
    "GDK_SCALE=2 reaches media::scale",
    scale.get() == 2.,
    format!(
      "{} (window {}x{})",
      scale.get(),
      shown.window.width(),
      shown.window.height()
    ),
  );

  shown.close();
}

fn body() {
  match std::env::args().nth(1).as_deref() {
    Some("portal") => portal(),
    Some("scale") => scale(),
    _ => settings(),
  }
}

fn main() -> std::process::ExitCode {
  probes::run("environment", body)
}
