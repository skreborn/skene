//! The GPU path on the real platform: every page of the conformance suite painted on the GPU
//! and then, the device lost, on the CPU, alike within a stated tolerance; and a window whose
//! device is lost painting on, its motion included, on the CPU.

use skene_gui::testing::{self, Drawn, Driver as _, PaintPath, PathChoice, Role, Screenshot};

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
