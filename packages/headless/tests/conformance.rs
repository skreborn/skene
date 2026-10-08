//! The conformance suite on the headless platform, and every page of it drawn against
//! the golden kept for it.

use std::{fs, path::Path, time::Duration};

use skene_conformance::{Driver, Probes, Reading};
use skene_foundation::{Container, HasChild, KeyEvent, NodeId, Role, Scene, Size, View};
use skene_headless::{
  Headless,
  Image,
  access_tree,
  find_by_role,
  find_by_text,
  focused_node,
  render_to_image,
  reset,
  trigger_focus,
  trigger_frame,
  trigger_resize,
  user,
};
use skene_reactive::Owner;

/// The headless platform driven as a user drives it, the page mounted under a root box as
/// every platform mounts it.
struct HeadlessDriver {
  /// The owner of what is mounted.
  owner: Option<Owner>,
  /// What the open page publishes.
  probes: Probes,
}

impl HeadlessDriver {
  /// A driver over a platform reset for it.
  fn new() -> Self {
    reset();

    Self {
      owner: None,
      probes: Probes::default(),
    }
  }
}

impl Drop for HeadlessDriver {
  fn drop(&mut self) {
    if let Some(owner) = self.owner.take() {
      owner.dispose();
    }
  }
}

impl Driver for HeadlessDriver {
  type Node = NodeId;
  type Scalar = f32;

  fn open(&mut self, page: &str) {
    if let Some(owner) = self.owner.take() {
      owner.dispose();
    }

    let page =
      skene_conformance::page::<Headless>(page).unwrap_or_else(|| panic!("no page named {page:?}"));
    let owner = Owner::new();
    let mut probes = Probes::default();
    let _root = owner.enter(|| {
      Container::<Headless>::new()
        .set_child((page.build)(&mut probes))
        .build()
    });

    self.owner = Some(owner);
    self.probes = probes;
  }

  fn probe(&mut self, name: &str) -> String {
    self
      .probes
      .read(name)
      .unwrap_or_else(|| panic!("the page publishes no probe {name:?}"))
  }

  fn find(&mut self, role: Role, name: &str) -> Option<NodeId> {
    find_by_role(role, name)
  }

  fn find_text(&mut self, text: &str) -> Option<NodeId> {
    find_by_text(text)
  }

  fn read(&mut self, node: &NodeId) -> Reading {
    Reading::of_tree(&access_tree(), *node)
  }

  fn hover(&mut self, node: &NodeId) {
    user::hover(*node);
  }

  fn click(&mut self, node: &NodeId) {
    user::click(*node);
  }

  fn hold(&mut self, node: &NodeId) {
    user::hold(*node);
  }

  fn release(&mut self, node: &NodeId) {
    user::release(*node);
  }

  fn press(&mut self, key: &KeyEvent) {
    user::press_key(key);
  }

  fn type_text(&mut self, node: &NodeId, text: &str) {
    user::type_text(*node, text);
  }

  fn focus(&mut self, node: &NodeId) {
    trigger_focus(*node);
  }

  fn resize(&mut self, size: Size<f32>) {
    trigger_resize(size);
  }

  fn advance(&mut self, duration: Duration) {
    trigger_frame(duration);
  }

  fn focused(&mut self) -> Option<NodeId> {
    focused_node()
  }

  fn focus_shows(&mut self) -> bool {
    Scene::<Headless>::read(|scene| {
      scene
        .focused()
        .is_some_and(|focused| scene.focus_shows(focused))
    })
  }

  fn is_pressed(&mut self, node: &NodeId) -> bool {
    Scene::<Headless>::read(|scene| scene.is_pressed(*node))
  }
}

#[test]
fn the_headless_platform_passes_the_suite() {
  skene_conformance::run(HeadlessDriver::new);
}

/// The viewport a golden is drawn at, in logical pixels: the browser goldens'.
const VIEWPORT: Size<f32> = Size::new(640., 400.);

/// The variable that, set to `overwrite`, writes each golden from what headless draws.
const OVERWRITE: &str = "SKENE_GOLDENS";

/// Every page of the suite draws as its golden: headless's picture of it in the pinned
/// face, compared pixel for pixel against `goldens/<page>.png`.
#[test]
fn every_page_draws_as_its_golden() {
  let goldens = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
  let failed: Vec<String> = skene_conformance::pages::<Headless>()
    .iter()
    .filter_map(|page| {
      check(&goldens, page.name)
        .err()
        .map(|failure| format!("{}: {failure}", page.name))
    })
    .collect();

  assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Draws the page `page` and compares it against `<goldens>/<page>.png`, or writes it
/// there where [`OVERWRITE`] says so.
///
/// # Errors
///
/// Returns how the page draws otherwise, what it draws left beside the golden as
/// `<page>.new.png`.
fn check(goldens: &Path, page: &str) -> Result<(), String> {
  let mut driver = HeadlessDriver::new();

  driver.resize(VIEWPORT);
  driver.open(page);

  let drawn = encode(&render_to_image());
  let golden = goldens.join(format!("{page}.png"));
  let fresh = goldens.join(format!("{page}.new.png"));
  let _ = fs::remove_file(&fresh);

  if std::env::var(OVERWRITE).as_deref() == Ok("overwrite") {
    return fs::write(&golden, &drawn).map_err(|error| format!("{}: {error}", golden.display()));
  }

  let difference = match fs::read(&golden) {
    Ok(kept) => difference(&kept, &drawn),
    Err(error) => Some(format!("{}: {error}", golden.display())),
  };

  match difference {
    None => Ok(()),
    Some(difference) => {
      let _ = fs::write(&fresh, &drawn);

      Err(format!(
        "{difference}; what it draws is in {}, and `{OVERWRITE}=overwrite` rewrites the \
         goldens",
        fresh.display()
      ))
    }
  }
}

/// How the PNG `drawn` differs from the PNG `kept`; `None` where their pixels are equal.
fn difference(kept: &[u8], drawn: &[u8]) -> Option<String> {
  let (kept, drawn) = match (decode(kept), decode(drawn)) {
    (Ok(kept), Ok(drawn)) => (kept, drawn),
    (Err(error), _) | (_, Err(error)) => return Some(error),
  };

  if (kept.width, kept.height) != (drawn.width, drawn.height) {
    return Some(format!(
      "it is {}×{} where the golden is {}×{}",
      drawn.width, drawn.height, kept.width, kept.height
    ));
  }

  let differing = kept
    .pixels
    .as_chunks::<4>()
    .0
    .iter()
    .zip(drawn.pixels.as_chunks::<4>().0)
    .filter(|(kept, drawn)| kept != drawn)
    .count();

  (differing > 0).then(|| format!("{differing} pixels differ from the golden"))
}

/// `image` as a PNG.
fn encode(image: &Image) -> Vec<u8> {
  let mut bytes = Vec::new();
  let mut encoder = png::Encoder::new(&mut bytes, u32::from(image.width), u32::from(image.height));

  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);

  let mut writer = encoder.write_header().expect("a PNG header");

  writer
    .write_image_data(&image.pixels)
    .expect("a PNG's pixels");
  drop(writer);

  bytes
}

/// The PNG `bytes` as an image of RGBA8 pixels.
///
/// # Errors
///
/// Returns what is wrong with a PNG that does not decode to RGBA8 pixels.
fn decode(bytes: &[u8]) -> Result<Image, String> {
  let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
  let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
  let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
  let info = reader
    .next_frame(&mut pixels)
    .map_err(|error| error.to_string())?;

  if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
    return Err(format!("a {:?} {:?} PNG", info.color_type, info.bit_depth));
  }

  pixels.truncate(info.buffer_size());

  Ok(Image {
    width: u16::try_from(info.width).map_err(|error| error.to_string())?,
    height: u16::try_from(info.height).map_err(|error| error.to_string())?,
    pixels,
  })
}
