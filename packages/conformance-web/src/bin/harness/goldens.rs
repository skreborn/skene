//! The screenshot goldens: every page of the suite as the pinned Firefox draws it in the
//! pinned font, compared pixel for pixel against the PNG kept for it.

use std::{fs, path::Path};

use skene_conformance::Driver;
use skene_foundation::Size;

use crate::browser::Browser;

/// The viewport a golden is taken at, in CSS pixels.
const VIEWPORT: Size<f32> = Size::new(640., 400.);

/// The variable that, set to `overwrite`, writes each golden from what the browser draws.
const OVERWRITE: &str = "SKENE_GOLDENS";

/// Takes the golden of the page `page` in `browser` and compares it against
/// `<goldens>/<page>.png`, or writes it there where [`OVERWRITE`] says so.
///
/// # Errors
///
/// Returns how the page draws otherwise, what it draws left beside the golden as
/// `<page>.new.png`.
pub(crate) fn check(browser: &mut Browser<'_>, goldens: &Path, page: &str) -> Result<(), String> {
  browser.open(page);
  browser.resize(VIEWPORT);
  browser.await_fonts();

  let drawn = browser.screenshot();
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
  let (kept, drawn) = match (Image::decode(kept), Image::decode(drawn)) {
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

/// A decoded PNG: its size and its pixels, four bytes of RGBA each.
struct Image {
  width: u32,
  height: u32,
  pixels: Vec<u8>,
}

impl Image {
  /// The image the PNG `bytes` holds.
  fn decode(bytes: &[u8]) -> Result<Self, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));

    decoder.set_transformations(png::Transformations::ALPHA | png::Transformations::EXPAND);

    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let frame = reader
      .next_frame(&mut pixels)
      .map_err(|error| error.to_string())?;

    pixels.truncate(frame.buffer_size());

    Ok(Self {
      width: frame.width,
      height: frame.height,
      pixels,
    })
  }
}
