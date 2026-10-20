//! The GPU path on the real platform: every page of the conformance suite painted on the GPU
//! and then, the device lost, on the CPU, alike within a stated tolerance; a window whose
//! device is lost painting on, its motion included, on the CPU; glyphs that outgrow the atlas,
//! together or alone, or are drawn as it starts over, painting on as on the CPU; a change
//! drawing its part of the window alone, as the whole window drew it; and a popup painting on
//! the window's device, and on the CPU once the device is lost under it. Built where the
//! realization builds its GPU path (skene-gui-linux's `gpu` feature).

use skene_gui::{
  AccessRequest,
  prelude::*,
  testing::{self, Drawn, Driver as _, PaintPath, PathChoice, Role, Screenshot},
};

/// The most a channel of a pixel may differ between the paths.
const MOST: u8 = 4;

/// The most pixels, in a million, whose channels may differ by more than [`NEAR`].
const FEW: usize = 100;

/// A difference at most this is the paths' own rounding.
const NEAR: u8 = 2;

/// How two shots of one page differ.
struct Difference {
  /// The largest difference of a channel.
  most: u8,
  /// The pixels a channel of which differs by more than [`NEAR`].
  apart: usize,
  /// The pixels compared.
  pixels: usize,
}

impl Difference {
  /// How `cpu` differs from `gpu`, which are the same size.
  fn between(gpu: &Screenshot, cpu: &Screenshot) -> Self {
    assert_eq!(
      (gpu.width, gpu.height),
      (cpu.width, cpu.height),
      "both paths paint the content at one size"
    );

    let mut difference = Self {
      most: 0,
      apart: 0,
      pixels: gpu.rgba.len() / 4,
    };

    for (one, other) in gpu.rgba.chunks(4).zip(cpu.rgba.chunks(4)) {
      let most = one
        .iter()
        .zip(other)
        .map(|(one, other)| one.abs_diff(*other))
        .max()
        .unwrap_or(0);

      difference.most = difference.most.max(most);

      if most > NEAR {
        difference.apart += 1;
      }
    }

    difference
  }

  /// Whether the paths paint alike: no channel further apart than [`MOST`], and at most
  /// [`FEW`] pixels in a million further apart than [`NEAR`].
  const fn is_alike(&self) -> bool {
    self.most <= MOST && self.apart * 1_000_000 <= FEW * self.pixels
  }
}

/// Opens `page` on the GPU, any adapter admitted.
fn on_the_gpu(page: &str) -> testing::Screen {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let screen = testing::open(page);

  assert_eq!(
    screen.driver().paint_path(),
    PaintPath::Gpu,
    "{page}: an adapter presents to the window (SKENE_TRACE=1 says why not)"
  );

  screen
}

#[skene_gui::test]
fn every_page_paints_alike_on_the_gpu_and_on_the_cpu() {
  skene_conformance::pages::register();

  let mut unlike = Vec::new();

  for page in testing::pages() {
    let screen = on_the_gpu(page);
    let gpu = screen.render_to_image();

    assert!(
      screen.driver().trigger_device_loss(),
      "{page}: the window painted on the GPU"
    );
    assert_eq!(
      screen.driver().paint_path(),
      PaintPath::Cpu,
      "{page}: the CPU took over"
    );

    let cpu = screen.render_to_image();
    let difference = Difference::between(&gpu, &cpu);

    eprintln!(
      "{page}: most {} apart, {} of {} pixels beyond {NEAR}",
      difference.most, difference.apart, difference.pixels
    );

    if !difference.is_alike() {
      if let Some(directory) = std::env::var_os("SKENE_SHOTS") {
        let directory = std::path::Path::new(&directory);

        screen.render_to_png(directory.join(format!("{page}-cpu.png")));
        write_png(&gpu, &directory.join(format!("{page}-gpu.png")));
      }

      unlike.push(format!(
        "{page}: a channel {} apart, {} of {} pixels beyond {NEAR}",
        difference.most, difference.apart, difference.pixels
      ));
    }
  }

  assert!(
    unlike.is_empty(),
    "pages the paths paint differently (beyond {MOST} a channel, or more than {FEW} pixels in a million beyond {NEAR}):\n{}",
    unlike.join("\n")
  );
}

#[skene_gui::test]
fn a_window_whose_device_is_lost_paints_on_the_cpu_motion_included() {
  skene_conformance::pages::register();

  let screen = on_the_gpu("motion");

  assert!(screen.driver().trigger_device_loss());
  assert_eq!(screen.driver().paint_path(), PaintPath::Cpu);

  let bounds = screen.find_by_role(Role::Region, "Derived").read().bounds;
  let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);

  assert_eq!(screen.pixel(x, y), skene_gui::Color::WHITE);

  screen.driver().set_recording(true);
  screen.find_by_role(Role::Button, "Warm").click();

  let frames = screen.driver().take_frames();

  screen.driver().set_recording(false);

  assert_eq!(screen.pixel(x, y), skene_gui::Color::rgb(0xff0000));

  let between = frames
    .iter()
    .filter(|frame| {
      let [red, green, blue, _] = frame.shot.rgba_at(x.trunc(), y.trunc());

      // Neither white nor red, but on the way: red stays high as green and blue fall.
      red > 0xe0 && (0x10..0xf0).contains(&green) && (0x10..0xf0).contains(&blue)
    })
    .count();

  assert!(
    between >= 3,
    "the transition ran on the CPU, frame by frame: {between} of {} frames between white and red",
    frames.len()
  );
}

/// A button that flips a long text's size between 200 and 400 pixels under a linear transition
/// of a second and a half: glyph images enough to fill the atlas many times over.
fn growing() -> impl skene_gui::View {
  use skene_gui::{Curve, Transition, keys};

  let big = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Grow").handle_activate(move || big.update(|big| *big = !*big)))
    .add_child(
      Container::new()
        .derive_font_size(move || if big.get() { 400. } else { 200. })
        .add_transition(Transition::new(keys::FONT_SIZE, 1500).set_curve(Curve::Linear))
        .set_child(Text::new(
          "The quick brown fox jumps over the lazy dog 0123456789 QWERTYUIOPASDFGHJKLZXCVBNM",
        )),
    )
}

#[skene_gui::test]
fn glyphs_that_outgrow_the_atlas_keep_painting_on_the_gpu() {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let screen = testing::render(growing);

  assert_eq!(screen.driver().paint_path(), PaintPath::Gpu);

  let grow = screen.find_by_role(Role::Button, "Grow");

  for _ in 0..4 {
    grow.click();
    screen.driver().wait(std::time::Duration::from_millis(1600));
  }

  assert_eq!(
    screen.driver().paint_path(),
    PaintPath::Gpu,
    "the atlas starts over rather than running out, and the device was not lost"
  );
}

/// Whether `shot`'s pixel at `x`, `y` is dark ink.
fn is_dark(shot: &Screenshot, x: u32, y: u32) -> bool {
  #[expect(clippy::cast_precision_loss, reason = "window pixels")]
  let [red, green, blue, _] = shot.rgba_at(x as f32, y as f32);

  red < 0x40 && green < 0x40 && blue < 0x40
}

/// The dark pixels of `shot`, and how many of its pixels are dark where `other`'s are not or
/// the other way round.
fn dark(shot: &Screenshot, other: &Screenshot) -> (usize, usize) {
  let (mut counted, mut differing) = (0, 0);

  for y in 0..shot.height {
    for x in 0..shot.width {
      let dark = is_dark(shot, x, y);

      counted += usize::from(dark);
      differing += usize::from(dark != is_dark(other, x, y));
    }
  }

  (counted, differing)
}

/// A caption, then `letter` at 5000 pixels, a mask some 4700 by 3600 pixels for a W, wider
/// than an atlas: the window shows where a W's tiles meet, and the caption's glyphs, in the
/// atlas first, leave the W's parts to grow it while the frame draws.
fn huge(letter: Signal<&'static str>) -> impl View {
  Stack::new().add_child(Text::new("Wa")).add_child(
    Pinned::new()
      .set_edges(Edges {
        top: Some(-2728.),
        trailing: None,
        bottom: None,
        leading: Some(-1858.),
      })
      .set_child(Text::derive(move || letter.get()).set_font_size(5000)),
  )
}

/// Asserts that `screen` paints on the GPU as it does on the CPU once its device is lost, the
/// letter it shows in ink.
fn paints_alike(screen: &testing::Screen, what: &str) {
  assert_eq!(screen.driver().paint_path(), PaintPath::Gpu);

  let gpu = screen.render_to_image();

  assert!(screen.driver().trigger_device_loss());

  let (cpu, differing) = dark(&screen.render_to_image(), &gpu);

  assert!(
    cpu > 1000 && differing * 1000 <= cpu,
    "{what} is drawn on the GPU as on the CPU: {cpu} dark pixels on the CPU, {differing} of them differing on the GPU"
  );
}

#[skene_gui::test]
fn a_glyph_larger_than_an_atlas_paints_on_the_gpu_as_on_the_cpu() {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let letter = Signal::new("W");
  let screen = testing::render(move || huge(letter));

  paints_alike(
    &screen,
    "the glyph wider than an atlas, drawn in parts as they grow it",
  );
}

#[skene_gui::test]
fn a_glyph_drawn_as_the_atlas_starts_over_paints_on_the_gpu_as_on_the_cpu() {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let letter = Signal::new("W");
  let screen = testing::render(move || huge(letter));

  // Each letter takes a quarter of the atlas or more: the W again, after the third, finds the
  // atlas past what it keeps and starts it over.
  for next in ["M", "X", "W"] {
    letter.set(next);
    screen.settle();
  }

  paints_alike(
    &screen,
    "the glyph drawn in the frame that starts the atlas over",
  );
}

#[skene_gui::test]
fn a_change_draws_its_part_alone_on_the_gpu_as_the_whole_window_drew_it() {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let screen = testing::render(skene_conformance_linux::plate);
  let switch = screen.find_by_role(Role::Switch, "Lit");

  assert_eq!(screen.driver().paint_path(), PaintPath::Gpu);

  // The window's first frame, drawn whole.
  let whole = screen.render_to_image();

  // A reader's flips, which move no pointer: each draws the plate and the switch again.
  switch.trigger_access(AccessRequest::Toggle);

  let ((left, top, width, height), (frame_width, frame_height)) = screen.driver().last_drawn();

  assert!(
    height * 2 <= frame_height,
    "the flip drew a band of {width}x{height} at {left},{top} of the {frame_width}x{frame_height} frame, not the whole"
  );

  switch.trigger_access(AccessRequest::Toggle);
  assert!(!switch.is_on());
  assert_eq!(screen.driver().paint_path(), PaintPath::Gpu);
  skene_conformance_linux::assert_drawn_alike(&whole, &screen.render_to_image());
}

/// A picker of three sizes in a column.
fn sizes() -> impl View {
  Column::new().set_padding(16).add_child(
    Picker::new("Size").set_options(["Small", "Medium", "Large"].map(|size| (size, size))),
  )
}

#[skene_gui::test]
fn a_popup_paints_on_the_gpu_and_on_the_cpu_once_the_device_is_lost() {
  Drawn::set_path_choice(Some(PathChoice::Any));

  let screen = testing::render(sizes);

  assert_eq!(screen.driver().paint_path(), PaintPath::Gpu);
  screen.find_by_role(Role::ComboBox, "Size").click();
  screen.find_by_role(Role::Option, "Medium");
  assert_eq!(
    screen.driver().popup_paint_path(),
    Some(PaintPath::Gpu),
    "the popup paints on the window's device"
  );

  let (gpu, at) = screen
    .driver()
    .popup_content()
    .expect("the list is open on its own surface");

  // Lost under the open popup, unknown to the window: the loss wakes it, and the window and
  // the popup paint again on the CPU with nothing else changing.
  assert!(screen.driver().trigger_device_loss());
  assert_eq!(screen.driver().paint_path(), PaintPath::Cpu);
  assert_eq!(screen.driver().popup_paint_path(), Some(PaintPath::Cpu));

  let (cpu, again) = screen
    .driver()
    .popup_content()
    .expect("the list stays open");
  let difference = Difference::between(&gpu, &cpu);

  assert_eq!(at, again, "the list stays where it was");
  assert!(
    difference.is_alike(),
    "the list paints alike on both paths: a channel {} apart, {} of {} pixels beyond {NEAR}",
    difference.most,
    difference.apart,
    difference.pixels
  );
}

/// Writes `shot` as a PNG at `path`.
fn write_png(shot: &Screenshot, path: &std::path::Path) {
  let file = std::fs::File::create(path).expect("the shot's file");
  let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.width, shot.height);

  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  encoder
    .write_header()
    .and_then(|mut writer| writer.write_image_data(&shot.rgba))
    .expect("the shot is written");
}
