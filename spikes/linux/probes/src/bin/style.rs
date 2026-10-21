//! Style on real GTK 4.14: content-addressed classes at priority 599, named color tokens at
//! 598, states as pseudo-classes, transitions as GTK CSS transitions, derived values in a
//! per-widget provider, media conditions answered by the shared half, and an app's own CSS
//! at 600 winning; every check reads the window's pixels.

use probes::{
  count, hex, key, near, near_within, result, run, settle, show, wait, widget, xdotool,
};
use skene_gui::{
  __private::{rules, sheet},
  Border, FontFamily, TextDecoration, TextDecorationStyle, Transition, keys,
  platform::linux::gtk::{self, prelude::*},
  prelude::*,
};

const RED: u32 = 0xff0000;
const BLUE: u32 = 0x0000ff;
const WHITE: u32 = 0xffffff;

/// A palette whose page is white in light and black in dark.
const PAGE: TokenRef<Color> = TokenRef::new("probe-page");
const GAP: TokenRef<f32> = TokenRef::new("probe-gap");

struct Palette;

impl TokenSet for Palette {
  fn define(&self, definitions: &mut Definitions) {
    definitions
      .add(&Token::new(PAGE, Color::WHITE).when(Media::Dark, Color::BLACK))
      .add(&Token::new(GAP, 24));
  }
}

fn page(child: impl View) -> Container {
  Container::new()
    .set_background(Color::WHITE)
    .set_padding(20)
    .set_alignment(Alignment::TOP_LEADING)
    .set_child(child)
}

fn body() {
  let settings = gtk::Settings::default().expect("settings");

  // Box decoration: background, border, corner radius.
  let card = NodeHandle::new();
  let shown = show(300, -1, || {
    page(
      Container::new()
        .set_width(100)
        .set_height(60)
        .set_background(Color::rgb(RED))
        .set_border(Border::new(4))
        .set_border_color(Color::rgb(BLUE))
        .set_corner_radius(20)
        .set_handle(&card),
    )
  });
  let (center, edge, corner) = (
    shown.pixel_in(&card, 50., 30.),
    shown.pixel_in(&card, 1., 30.),
    shown.pixel_in(&card, 1., 1.),
  );

  result(
    "background, a 4px border and a 20px corner radius",
    near(center, RED) && near(edge, BLUE) && near(corner, WHITE),
    format!(
      "center {} edge {} corner {}",
      hex(center),
      hex(edge),
      hex(corner)
    ),
  );

  shown.close();

  // A shadow below, and opacity.
  let [shadowed, faded] = [NodeHandle::new(), NodeHandle::new()];
  let shown = show(300, -1, || {
    page(
      Column::new()
        .set_spacing(30)
        .add_child(
          Container::new()
            .set_width(100)
            .set_height(30)
            .set_background(Color::rgb(RED))
            .set_shadow(Shadow::new(8, 0, Color::BLACK))
            .set_handle(&shadowed),
        )
        .add_child(
          Container::new()
            .set_width(100)
            .set_height(30)
            .set_background(Color::rgb(RED))
            .set_opacity(0.5)
            .set_handle(&faded),
        ),
    )
  });
  let (below, half) = (
    shown.pixel_in(&shadowed, 50., 34.),
    shown.pixel_in(&faded, 50., 15.),
  );

  result(
    "a shadow 8px below, and opacity 0.5",
    near(below, 0x000000) && near_within(half, 0xff8080, 4),
    format!("below {} faded {}", hex(below), hex(half)),
  );

  shown.close();

  // Run properties on a text.
  let texts: [NodeHandle; 8] = std::array::from_fn(|_| NodeHandle::new());
  let shown = show(400, -1, || {
    page(
      Column::new()
        .set_spacing(8)
        .add_child(
          Text::new("Red ink")
            .set_color(Color::rgb(RED))
            .set_handle(&texts[0]),
        )
        .add_child(Text::new("Small").set_font_size(12).set_handle(&texts[1]))
        .add_child(Text::new("Large").set_font_size(32).set_handle(&texts[2]))
        .add_child(
          Text::new("Spaced")
            .set_letter_spacing(10)
            .set_handle(&texts[3]),
        )
        .add_child(Text::new("Spaced").set_handle(&texts[4]))
        .add_child(
          Text::new("Underlined")
            .set_decoration(TextDecoration::UNDERLINE)
            .set_decoration_style(TextDecorationStyle::Double)
            .set_decoration_color(Color::rgb(BLUE))
            .set_handle(&texts[5]),
        )
        .add_child(
          Text::new("iiiiiiii")
            .set_font_family(FontFamily::Monospace)
            .set_handle(&texts[6]),
        )
        .add_child(Text::new("iiiiiiii").set_handle(&texts[7])),
    )
  });
  let red_ink = shown.count_in(&texts[0], RED);
  let (small, large) = (shown.rect(&texts[1]).3, shown.rect(&texts[2]).3);
  let (spaced, plain) = (
    widget(&texts[3])
      .measure(gtk::Orientation::Horizontal, -1)
      .1,
    widget(&texts[4])
      .measure(gtk::Orientation::Horizontal, -1)
      .1,
  );
  let underline = shown.count_in(&texts[5], BLUE);
  let (mono, sans) = (
    widget(&texts[6])
      .measure(gtk::Orientation::Horizontal, -1)
      .1,
    widget(&texts[7])
      .measure(gtk::Orientation::Horizontal, -1)
      .1,
  );

  result("text color", red_ink > 20, format!("{red_ink} red pixels"));
  result(
    "font size 12 and 32",
    large > small * 2.,
    format!("heights {small} and {large}"),
  );
  result(
    "letter spacing 10",
    spaced - plain >= 50,
    format!("natural widths {plain} and {spaced} (6 letters)"),
  );
  result(
    "a double underline in its own color",
    underline > 30,
    format!("{underline} blue pixels"),
  );
  result(
    "monospace",
    mono > sans * 2,
    format!("\"iiiiiiii\" {sans} wide in the system face, {mono} in monospace"),
  );

  shown.close();

  // A box's run properties are inherited by the texts beneath it.
  let [inherited, regular] = [NodeHandle::new(), NodeHandle::new()];
  let shown = show(300, -1, || {
    page(
      Container::new()
        .set_color(Color::rgb(0x00a000))
        .set_font_weight(FontWeight::BOLD)
        .set_child(
          Column::new()
            .add_child(Text::new("Green and bold").set_handle(&inherited))
            .add_child(
              Text::new("Green and bold")
                .set_font_weight(FontWeight::REGULAR)
                .set_handle(&regular),
            ),
        ),
    )
  });
  let green = shown.count_in(&inherited, 0x00a000);
  let bold = green;

  let plain = shown.count_in(&regular, 0x00a000);

  result(
    "a box's color is inherited by a text beneath",
    green > 20,
    format!("{green} green pixels"),
  );
  result(
    "a box's bold weight is inherited, a text's own regular overrides it",
    bold > plain + plain / 5,
    format!("green ink pixels: inherited bold {bold}, own regular {plain}"),
  );

  shown.close();

  // Tokens: a color token is a GTK named color, rewritten on a scheme switch with no node
  // touched; a length token is resolved.
  let [themed, first, second] = [NodeHandle::new(), NodeHandle::new(), NodeHandle::new()];
  let shown = show(300, -1, || {
    Theme::new(Palette).set_child(
      Container::new()
        .set_background(PAGE)
        .set_padding(10)
        .set_handle(&themed)
        .set_child(
          Column::new()
            .set_spacing(GAP)
            .add_child(Container::new().set_height(10).set_handle(&first))
            .add_child(Container::new().set_height(10).set_handle(&second)),
        ),
    )
  });
  let light = shown.pixel_in(&themed, 2., 2.);
  let before = sheet();

  media::set_scheme(Some(Scheme::Dark));
  settle();

  let dark = shown.pixel_in(&themed, 2., 2.);
  let after = sheet();
  let gap = shown.rect(&second).1 - shown.rect(&first).1;

  result(
    "a color token follows the scheme as a named color",
    near(light, WHITE) && near(dark, 0x000000),
    format!("light {} dark {}", hex(light), hex(dark)),
  );
  result(
    "a scheme switch restyles no node",
    after.applies == before.applies && after.token_reloads > before.token_reloads,
    format!(
      "style applies {} -> {}, token reloads {} -> {}",
      before.applies, after.applies, before.token_reloads, after.token_reloads
    ),
  );
  result(
    "a length token resolves",
    gap == 34.,
    format!("children 34 apart: {gap}"),
  );

  media::set_scheme(None);
  settle();

  let back = shown.pixel_in(&themed, 2., 2.);

  result(
    "following the reader again (light) restores it",
    near(back, WHITE),
    hex(back),
  );

  shown.close();

  // A theme rebuilt again and again withdraws its named colors each time it goes.
  let generation = Signal::new(0);
  let scopes = skene_gui::__private::token_scopes();
  let shown = show(200, -1, || {
    Derived::new(move || {
      let _ = generation.get();

      Theme::new(Palette).set_child(Container::new().set_height(10).set_background(PAGE))
    })
  });
  let during = skene_gui::__private::token_scopes();

  for step in 1..=10 {
    generation.set(step);
    settle();
  }

  result(
    "a theme rebuilt ten times leaves one scope of named colors",
    during == scopes + 1 && skene_gui::__private::token_scopes() == scopes + 1,
    format!(
      "scopes {scopes} before, {during} shown, {} after ten rebuilds",
      skene_gui::__private::token_scopes()
    ),
  );

  shown.close();

  // Media conditions on a property: width breakpoints, the scheme, reduced motion.
  let [padded, swatch, still] = [NodeHandle::new(), NodeHandle::new(), NodeHandle::new()];
  let shown = show(400, -1, || {
    Column::new()
      .add_child(
        Container::new()
          .set_padding(8.when(Media::min_width(600), 32))
          .set_child(Container::new().set_height(10).set_handle(&padded)),
      )
      .add_child(
        Container::new()
          .set_height(20)
          .set_background(
            Value::from(Color::rgb(RED))
              .when(Media::max_width(500), Color::rgb(BLUE))
              .when(Media::Dark, Color::rgb(0x00ff00)),
          )
          .set_handle(&swatch),
      )
      .add_child(
        Container::new()
          .set_height(20)
          .set_background(Value::from(Color::rgb(RED)).when(Media::ReducedMotion, Color::rgb(BLUE)))
          .set_handle(&still),
      )
  });
  let narrow = (shown.rect(&padded).0, shown.pixel_in(&swatch, 5., 5.));

  shown.resize(700, 200);

  let wide = (shown.rect(&padded).0, shown.pixel_in(&swatch, 5., 5.));

  result(
    "min_width(600) and max_width(500) follow the window",
    narrow.0 == 8. && near(narrow.1, BLUE) && wide.0 == 32. && near(wide.1, RED),
    format!(
      "at 400: padding {} swatch {}; at 700: padding {} swatch {}",
      narrow.0,
      hex(narrow.1),
      wide.0,
      hex(wide.1)
    ),
  );

  media::set_scheme(Some(Scheme::Dark));
  settle();

  let dark = shown.pixel_in(&swatch, 5., 5.);

  media::set_scheme(None);
  settle();
  result("Media::Dark on a property", near(dark, 0x00ff00), hex(dark));

  let moving = shown.pixel_in(&still, 5., 5.);

  settings.set_gtk_enable_animations(false);
  settle();

  let reduced = shown.pixel_in(&still, 5., 5.);

  settings.set_gtk_enable_animations(true);
  settle();

  result(
    "Media::ReducedMotion follows gtk-enable-animations",
    near(moving, RED) && near(reduced, BLUE),
    format!("{} then {}", hex(moving), hex(reduced)),
  );

  shown.close();

  // States as GTK pseudo-classes: hover, press, focus that shows, focus within, disabled.
  let [hovered, pressed, focusable, within, entry, disabled] =
    std::array::from_fn(|_| NodeHandle::new());
  let gray = Color::rgb(0xcccccc);
  let shown = show(400, -1, || {
    Column::new()
      .set_spacing(10)
      .add_child(
        Container::new()
          .set_height(30)
          .set_background(Value::from(gray).when(State::Hovered, Color::rgb(RED)))
          .set_handle(&hovered),
      )
      .add_child(
        Container::new()
          .set_height(30)
          .set_background(Value::from(gray).when(State::Pressed, Color::rgb(0x00ff00)))
          .set_handle(&pressed),
      )
      .add_child(
        Container::new()
          .set_height(30)
          .set_background(Value::from(gray).when(State::FocusVisible, Color::rgb(BLUE)))
          .handle_click(|| {})
          .set_handle(&focusable),
      )
      .add_child(
        Container::new()
          .set_padding(10)
          .set_background(Value::from(gray).when(State::FocusWithin, Color::rgb(0xffff00)))
          .set_handle(&within)
          .set_child(TextField::new("Name").set_handle(&entry)),
      )
      .add_child(
        Container::new().set_child(
          Container::new()
            .set_height(30)
            .set_background(Value::from(gray).when(State::Disabled, Color::rgb(0xff00ff)))
            .set_handle(&disabled),
        ),
      )
  });

  shown.hover(&widget(&hovered));

  let hover = shown.pixel_in(&hovered, 5., 5.);

  shown.hover(&widget(&pressed));

  let unpressed = shown.pixel_in(&pressed, 5., 5.);

  xdotool(&["mousedown", "1"]);

  let press = shown.pixel_in(&pressed, 5., 5.);

  xdotool(&["mouseup", "1"]);

  let away = shown.pixel_in(&hovered, 5., 5.);

  result(
    "State::Hovered as :hover from real pointer motion",
    near(hover, RED) && near(away, 0xcccccc),
    format!("over {} away {}", hex(hover), hex(away)),
  );
  result(
    "State::Pressed as :active while a real press holds",
    near(unpressed, 0xcccccc) && near(press, 0x00ff00),
    format!("before {} pressed {}", hex(unpressed), hex(press)),
  );

  let mut tabs = 0;

  loop {
    key("Tab");
    tabs += 1;

    if widget(&focusable).has_focus() || tabs >= 10 {
      break;
    }
  }

  let focus = shown.pixel_in(&focusable, 5., 5.);

  result(
    "State::FocusVisible as :focus-visible after Tab",
    near(focus, BLUE),
    format!("{} after {tabs} Tab presses", hex(focus)),
  );

  let before = shown.pixel_in(&within, 2., 2.);

  shown.click(&widget(&entry));

  let inside = shown.pixel_in(&within, 2., 2.);

  result(
    "State::FocusWithin as :focus-within",
    near(before, 0xcccccc) && near(inside, 0xffff00),
    format!(
      "{} then {} with the entry focused",
      hex(before),
      hex(inside)
    ),
  );

  if let Some(parent) = widget(&disabled).parent() {
    parent.set_sensitive(false);
  }

  let off = shown.pixel_in(&disabled, 5., 5.);

  result(
    "State::Disabled as :disabled under an insensitive ancestor",
    near(off, 0xff00ff),
    hex(off),
  );

  shown.close();

  // A transition: a GTK CSS transition, cut where animations are off.
  let color = Signal::new(Color::rgb(RED));
  let animated = NodeHandle::new();
  let shown = show(200, -1, || {
    Container::new()
      .set_height(40)
      .derive_background(move || color.get())
      .add_transition(Transition::new(keys::BACKGROUND, 600))
      .set_handle(&animated)
  });
  let start = shown.pixel_in(&animated, 5., 5.);

  color.set(Color::rgb(BLUE));
  wait(200);

  let middle = shown.pixel_now(&animated, 5., 5.);

  wait(700);

  let end = shown.pixel_now(&animated, 5., 5.);

  settings.set_gtk_enable_animations(false);
  color.set(Color::rgb(RED));
  wait(60);

  let cut = shown.pixel_now(&animated, 5., 5.);

  settings.set_gtk_enable_animations(true);

  result(
    "a 600 ms transition passes through between",
    near(start, RED) && !near(middle, RED) && !near(middle, BLUE) && near(end, BLUE),
    format!(
      "{} -> {} at 200 ms -> {} at 900 ms",
      hex(start),
      hex(middle),
      hex(end)
    ),
  );
  result(
    "gtk-enable-animations=false cuts it",
    near(cut, RED),
    format!("{} 60 ms after the change", hex(cut)),
  );

  shown.close();

  // A derived value goes through the widget's own provider: no rule, no reload, no apply.
  let level = Signal::new(0_u8);
  let derived = NodeHandle::new();
  let shown = show(200, -1, || {
    Container::new()
      .set_height(20)
      .derive_background(move || Color::rgb(u32::from(level.get()) << 16))
      .set_handle(&derived)
  });
  let (stats, rule_count) = (sheet(), rules());

  for step in 1..=5 {
    level.set(step * 50);
    settle();
  }

  let (after, after_rules) = (sheet(), rules());
  let last = shown.pixel_in(&derived, 5., 5.);

  result(
    "five derived values: five per-widget writes, no rule, no reload, no apply",
    after.slot_writes - stats.slot_writes == 5
      && after.applies == stats.applies
      && after_rules == rule_count
      && after.reloads == stats.reloads
      && near(last, 0xfa0000),
    format!(
      "slot writes +{}, applies +{}, rules {} -> {}, reloads +{}, shown {}",
      after.slot_writes - stats.slot_writes,
      after.applies - stats.applies,
      rule_count,
      after_rules,
      after.reloads - stats.reloads,
      hex(last)
    ),
  );

  shown.close();

  // An app's own CSS at GTK's application priority beats Skene's, and GTK's theme classes.
  let provider = gtk::CssProvider::new();

  provider.load_from_string(".app-ink { color: rgb(0, 128, 0); }");

  gtk::style_context_add_provider_for_display(
    &gtk::gdk::Display::default().expect("a display"),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
  );

  let [app, titled, plain] = [NodeHandle::new(), NodeHandle::new(), NodeHandle::new()];
  let shown = show(400, -1, || {
    page(
      Column::new()
        .add_child(
          Text::new("The app's green")
            .set_color(Color::rgb(RED))
            .set_linux_style_classes("app-ink")
            .set_handle(&app),
        )
        .add_child(
          Text::new("Title")
            .set_linux_style_classes("title-1")
            .set_handle(&titled),
        )
        .add_child(Text::new("Title").set_handle(&plain)),
    )
  });
  let (green, red) = (shown.count_in(&app, 0x008000), shown.count_in(&app, RED));
  let (title, body) = (shown.rect(&titled).3, shown.rect(&plain).3);

  result(
    "an app's CSS at priority 600 beats Skene's at 599",
    green > 20 && red == 0,
    format!("{green} green pixels, {red} red"),
  );
  result(
    "set_linux_style_classes: GTK's title-1",
    title > body * 1.5,
    format!("heights {body} plain, {title} title-1"),
  );

  shown.close();

  // Content addressing: equal styles share one class; a class nothing refers to is collected.
  let (one, two) = (NodeHandle::new(), NodeHandle::new());
  let first_color = Signal::new(true);
  let count_before = rules();
  let rebuilt = two;
  let shown = show(200, -1, || {
    Column::new()
      .add_child(
        Container::new()
          .set_height(10)
          .set_background(Color::rgb(0x123456))
          .set_handle(&one),
      )
      .add_child(Derived::new(move || {
        Container::new()
          .set_height(10)
          .set_background(Color::rgb(if first_color.get() {
            0x123456
          } else {
            0x654321
          }))
          .set_handle(&rebuilt)
      }))
      .add_child(Derived::new(move || {
        Container::new()
          .set_height(10)
          .set_background(Color::rgb(if first_color.get() {
            0x111111
          } else {
            0x222222
          }))
      }))
  });
  let classes = |handle: &NodeHandle| -> Vec<String> {
    widget(handle)
      .css_classes()
      .iter()
      .map(ToString::to_string)
      .filter(|class| class.starts_with("skene-"))
      .collect()
  };
  let (first, second) = (classes(&one), classes(&two));

  result(
    "equal styles share one content-addressed class",
    first == second
      && first.len() == 1
      && first[0].len() == "skene-".len() + 16
      && rules() == count_before + 2,
    format!(
      "{first:?} {second:?}; three boxes, two styles: rules {count_before} -> {}",
      rules()
    ),
  );

  let collected = sheet().collected;

  first_color.set(false);
  settle();

  result(
    "a class nothing refers to is collected at the next reload",
    sheet().collected == collected + 1,
    format!(
      "two derived boxes rebuilt in new colors, one of the old classes still referred to: collected {collected} -> {}",
      sheet().collected
    ),
  );

  shown.close();

  // Clipping: a rounded box cuts its child at its corners.
  let [clipped, open] = [NodeHandle::new(), NodeHandle::new()];
  let filler = || {
    Container::new()
      .set_width(80)
      .set_height(80)
      .set_background(Color::rgb(RED))
  };
  let shown = show(300, -1, || {
    page(
      Row::new()
        .set_spacing(20)
        .add_child(
          Container::new()
            .set_corner_radius(30)
            .set_clip(true)
            .set_handle(&clipped)
            .set_child(filler()),
        )
        .add_child(
          Container::new()
            .set_corner_radius(30)
            .set_handle(&open)
            .set_child(filler()),
        ),
    )
  });
  let (cut, uncut) = (
    shown.pixel_in(&clipped, 1., 1.),
    shown.pixel_in(&open, 1., 1.),
  );

  result(
    "set_clip cuts a child at the rounded corner",
    near(cut, WHITE) && near(uncut, RED),
    format!("clipped corner {} unclipped {}", hex(cut), hex(uncut)),
  );

  shown.screenshot("results/style-clip.png");
  shown.close();

  let _ = count;
}

fn main() -> std::process::ExitCode {
  run("style", body)
}
