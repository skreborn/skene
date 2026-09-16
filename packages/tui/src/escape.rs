//! The control sequences the loop writes: DEC private modes, cursor controls,
//! graphic renditions, the kitty keyboard flags, and the OSC strings for the title
//! and the clipboard.
//!
//! Every value writes through [`Display`], so a frame is written with no allocation.

use std::fmt::{self, Display};

/// The string terminator, `ESC \`.
pub(crate) const ST: &str = "\x1b\\";

/// DECSET 25: whether the cursor shows.
pub(crate) const SHOW_CURSOR: u16 = 25;

/// DECSET 1000: mouse presses and releases reported.
pub(crate) const MOUSE_TRACKING: u16 = 1000;

/// DECSET 1002: mouse motion with a button held reported.
pub(crate) const BUTTON_EVENT_MOUSE: u16 = 1002;

/// DECSET 1003: every mouse motion reported.
pub(crate) const ANY_EVENT_MOUSE: u16 = 1003;

/// DECSET 1006: mouse reports in the SGR encoding.
pub(crate) const SGR_MOUSE: u16 = 1006;

/// DECSET 1016: mouse reports in pixels.
pub(crate) const SGR_PIXELS_MOUSE: u16 = 1016;

/// DECSET 1049: the alternate screen, cleared, with the cursor saved.
pub(crate) const ALTERNATE_SCREEN: u16 = 1049;

/// DECSET 2004: pastes bracketed.
pub(crate) const BRACKETED_PASTE: u16 = 2004;

/// DECSET 2026: synchronized output.
pub(crate) const SYNCHRONIZED_OUTPUT: u16 = 2026;

/// DECSET 2027: grapheme clustering.
pub(crate) const GRAPHEME_CLUSTERING: u16 = 2027;

/// DECSET 2031: color scheme notifications.
pub(crate) const THEME: u16 = 2031;

/// DECSET 2048: in-band resize notifications.
pub(crate) const IN_BAND_RESIZE: u16 = 2048;

/// Mode 5522: the clipboard that answers (kitty's protocol), queried but never set.
pub(crate) const ANSWERING_CLIPBOARD: u16 = 5522;

/// The request for a cursor position report, `CSI 6 n`.
pub(crate) const REQUEST_CURSOR_POSITION: &str = "\x1b[6n";

/// The query for the color scheme, answered by a theme report.
pub(crate) const QUERY_THEME: &str = "\x1b[?996n";

/// The query for the kitty keyboard flags in force.
pub(crate) const QUERY_KEYBOARD_FLAGS: &str = "\x1b[?u";

/// The query for the foreground color, OSC 10.
pub(crate) const QUERY_FOREGROUND: &str = "\x1b]10;?\x1b\\";

/// The query for the background color, OSC 11.
pub(crate) const QUERY_BACKGROUND: &str = "\x1b]11;?\x1b\\";

/// The query for the sixteen base colors, OSC 4.
pub(crate) const QUERY_PALETTE: &str =
  "\x1b]4;0;?;1;?;2;?;3;?;4;?;5;?;6;?;7;?;8;?;9;?;10;?;11;?;12;?;13;?;14;?;15;?\x1b\\";

/// The query for the clipboard's content, OSC 52.
pub(crate) const QUERY_CLIPBOARD: &str = "\x1b]52;c;?\x1b\\";

/// The request for the clipboard's plain text through the answering clipboard, OSC
/// 5522; the payload names the type, `text/plain`, in base64.
pub(crate) const READ_CLIPBOARD: &str = "\x1b]5522;type=read;dGV4dC9wbGFpbg==\x1b\\";

/// The plain text type as the answering clipboard names it, in base64.
const PLAIN_TEXT: &str = "dGV4dC9wbGFpbg==";

/// The largest chunk of clipboard text one packet carries, before encoding.
const CLIPBOARD_CHUNK: usize = 4096;

/// Writes `text` to the answering clipboard, OSC 5522: the write opened, the text in
/// chunks as plain text, and the write closed.
pub(crate) fn write_clipboard(out: &mut impl std::io::Write, text: &str) -> std::io::Result<()> {
  write!(out, "\x1b]5522;type=write{ST}")?;

  for chunk in text.as_bytes().chunks(CLIPBOARD_CHUNK) {
    write!(
      out,
      "\x1b]5522;type=wdata:mime={PLAIN_TEXT};{}{ST}",
      base64::encode(chunk)
    )?;
  }

  write!(out, "\x1b]5522;type=wdata{ST}")
}

/// The DECRQSS request for the graphic rendition in force.
pub(crate) const REQUEST_GRAPHIC_RENDITION: &str = "\x1bP$qm\x1b\\";

/// The kitty keyboard flag for disambiguated escape codes.
pub(crate) const DISAMBIGUATE_ESCAPE_CODES: u8 = 1;

/// The kitty keyboard flag for event types: a release and a repeat.
pub(crate) const REPORT_EVENT_TYPES: u8 = 2;

/// The kitty keyboard flag for alternate keys: the shifted and base-layout keys.
pub(crate) const REPORT_ALTERNATE_KEYS: u8 = 4;

/// The kitty keyboard flag for every key as an escape code.
pub(crate) const REPORT_ALL_KEYS_AS_ESCAPE_CODES: u8 = 8;

/// A DEC private mode set or reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecMode {
  pub(crate) code: u16,
  pub(crate) on: bool,
}

impl Display for DecMode {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      out,
      "\x1b[?{}{}",
      self.code,
      if self.on { 'h' } else { 'l' }
    )
  }
}

/// A DECRQM query for a DEC private mode, answered by a mode report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryMode(pub(crate) u16);

impl Display for QueryMode {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b[?{}$p", self.0)
  }
}

/// The cursor moved to a zero-based cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MoveTo {
  pub(crate) column: u16,
  pub(crate) row: u16,
}

impl Display for MoveTo {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      out,
      "\x1b[{};{}H",
      u32::from(self.row) + 1,
      u32::from(self.column) + 1
    )
  }
}

/// The cursor's shape, DECSCUSR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CursorStyle {
  /// The terminal's own.
  Default,
  /// A blinking bar.
  BlinkingBar,
}

impl Display for CursorStyle {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    let code = match self {
      Self::Default => 0,
      Self::BlinkingBar => 5,
    };

    write!(out, "\x1b[{code} q")
  }
}

/// The kitty keyboard flags assigned wholesale, the stackless form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyboardFlags(pub(crate) u8);

impl Display for KeyboardFlags {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b[={};1u", self.0)
  }
}

/// The window title, OSC 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Title<'a>(pub(crate) &'a str);

impl Display for Title<'_> {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b]2;{}{ST}", self.0)
  }
}

/// Text written to the clipboard, OSC 52.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Copy<'a>(pub(crate) &'a str);

impl Display for Copy<'_> {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b]52;c;{}{ST}", base64::encode(self.0.as_bytes()))
  }
}

/// The pointer's shape by its name, OSC 22.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointerShape<'a>(pub(crate) &'a str);

impl Display for PointerShape<'_> {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b]22;{}{ST}", self.0)
  }
}

/// The largest payload one graphics command carries, in base64 characters.
const GRAPHICS_CHUNK: usize = 4096;

/// The graphics protocol's query: a one-pixel image the terminal never keeps, answered
/// `OK` under the same id where the protocol is spoken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphicsQuery(pub(crate) u32);

impl Display for GraphicsQuery {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b_Gi={},s=1,v=1,a=q,t=d,f=24;AAAA{ST}", self.0)
  }
}

/// Transmits `pixels`, unpremultiplied RGBA8 of `width` by `height`, as the image
/// `id`, replacing any the terminal holds under it; in chunks, every reply
/// suppressed.
pub(crate) fn write_transmit(
  out: &mut impl std::io::Write,
  id: u32,
  width: u16,
  height: u16,
  pixels: &[u8],
) -> std::io::Result<()> {
  let encoded = base64::encode(pixels);
  let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(GRAPHICS_CHUNK).collect();
  let last = chunks.len().saturating_sub(1);

  for (index, chunk) in chunks.iter().enumerate() {
    let more = u8::from(index < last);

    if index == 0 {
      write!(
        out,
        "\x1b_Ga=t,f=32,s={width},v={height},i={id},q=2,m={more};"
      )?;
    }
    else {
      write!(out, "\x1b_Gm={more};")?;
    }

    out.write_all(chunk)?;
    out.write_all(ST.as_bytes())?;
  }

  Ok(())
}

/// Places the transmitted image `id` at the cursor, scaled over `columns` by `rows`
/// cells, showing the source rectangle `crop` (left, top, width, height, in pixels)
/// on `layer`; the cursor stays, every reply suppressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
  pub(crate) id: u32,
  pub(crate) columns: u16,
  pub(crate) rows: u16,
  pub(crate) crop: (u16, u16, u16, u16),
  pub(crate) layer: i32,
}

impl Display for Place {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    let (left, top, width, height) = self.crop;

    write!(
      out,
      "\x1b_Ga=p,i={},p=1,c={},r={},x={left},y={top},w={width},h={height},z={},C=1,q=2{ST}",
      self.id, self.columns, self.rows, self.layer
    )
  }
}

/// Removes the placement of the image `id`, keeping its pixels for a later placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unplace(pub(crate) u32);

impl Display for Unplace {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b_Ga=d,d=i,i={},q=2{ST}", self.0)
  }
}

/// Removes the image `id` with its placements, freeing its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Free(pub(crate) u32);

impl Display for Free {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(out, "\x1b_Ga=d,d=I,i={},q=2{ST}", self.0)
  }
}

/// Removes every image with its placements, freeing their pixels.
pub(crate) const FREE_ALL: &str = "\x1b_Ga=d,d=A,q=2\x1b\\";

/// A color as SGR names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Color {
  /// The terminal's default.
  Reset,
  /// An entry of the 256-color palette.
  Palette(u8),
  /// 24-bit RGB.
  Rgb(u8, u8, u8),
}

/// An underline's style as SGR names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Underline {
  None,
  Single,
  Double,
  Curly,
  Dotted,
  Dashed,
}

/// One graphic rendition attribute, written as a whole SGR control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sgr {
  Reset,
  Bold(bool),
  Underline(Underline),
  UnderlineColor(Color),
  Foreground(Color),
  Background(Color),
  Reverse(bool),
  StrikeThrough(bool),
  Overline(bool),
}

impl Display for Sgr {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str("\x1b[")?;

    match self {
      Self::Reset => {}
      Self::Bold(true) => out.write_str("1")?,
      Self::Bold(false) => out.write_str("22")?,
      Self::Underline(Underline::None) => out.write_str("24")?,
      Self::Underline(Underline::Single) => out.write_str("4")?,
      Self::Underline(Underline::Double) => out.write_str("4:2")?,
      Self::Underline(Underline::Curly) => out.write_str("4:3")?,
      Self::Underline(Underline::Dotted) => out.write_str("4:4")?,
      Self::Underline(Underline::Dashed) => out.write_str("4:5")?,
      Self::UnderlineColor(Color::Reset) => out.write_str("59")?,
      Self::UnderlineColor(Color::Palette(index)) => write!(out, "58:5:{index}")?,
      Self::UnderlineColor(Color::Rgb(red, green, blue)) => {
        write!(out, "58:2::{red}:{green}:{blue}")?;
      }
      Self::Foreground(color) => write_color(out, 38, *color)?,
      Self::Background(color) => write_color(out, 48, *color)?,
      Self::Reverse(true) => out.write_str("7")?,
      Self::Reverse(false) => out.write_str("27")?,
      Self::StrikeThrough(true) => out.write_str("9")?,
      Self::StrikeThrough(false) => out.write_str("29")?,
      Self::Overline(true) => out.write_str("53")?,
      Self::Overline(false) => out.write_str("55")?,
    }

    out.write_str("m")
  }
}

/// Writes a foreground (`38`) or background (`48`) color: the base sixteen by their
/// own codes, the palette by `5`, RGB by `2` with semicolons, as every terminal reads.
fn write_color(out: &mut fmt::Formatter<'_>, code: u8, color: Color) -> fmt::Result {
  match color {
    Color::Reset => write!(out, "{}", code + 1),
    Color::Palette(index @ 0..=7) => write!(out, "{}", code - 8 + index),
    Color::Palette(index @ 8..=15) => write!(out, "{}", code + 44 + index),
    Color::Palette(index) => write!(out, "{code};5;{index}"),
    Color::Rgb(red, green, blue) => write!(out, "{code};2;{red};{green};{blue}"),
  }
}

/// Base64 in the standard alphabet, as OSC 52 carries the clipboard: encoded padded,
/// decoded with or without padding.
pub(crate) mod base64 {
  use base64::{
    Engine as _,
    alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
  };

  /// The engine: the standard alphabet, padding written and not required.
  const ENGINE: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
  );

  /// `bytes` encoded.
  pub(crate) fn encode(bytes: &[u8]) -> String {
    ENGINE.encode(bytes)
  }

  /// `text` decoded; `None` where it is not base64.
  pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
    ENGINE.decode(text).ok()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The modes, cursor controls and strings write as the terminal reads them.
  #[test]
  fn controls_write_their_sequences() {
    assert_eq!(
      DecMode {
        code: SYNCHRONIZED_OUTPUT,
        on: true
      }
      .to_string(),
      "\x1b[?2026h"
    );
    assert_eq!(QueryMode(IN_BAND_RESIZE).to_string(), "\x1b[?2048$p");
    assert_eq!(MoveTo { column: 2, row: 1 }.to_string(), "\x1b[2;3H");
    assert_eq!(CursorStyle::BlinkingBar.to_string(), "\x1b[5 q");
    assert_eq!(KeyboardFlags(15).to_string(), "\x1b[=15;1u");
    assert_eq!(Title("demo").to_string(), "\x1b]2;demo\x1b\\");
    assert_eq!(
      Copy("copied text").to_string(),
      "\x1b]52;c;Y29waWVkIHRleHQ=\x1b\\"
    );
    assert_eq!(PointerShape("text").to_string(), "\x1b]22;text\x1b\\");
  }

  /// Renditions write their codes: the base colors by their own, the palette and RGB
  /// by their introducers, an underline's style and color in the colon form.
  #[test]
  fn renditions_write_their_codes() {
    assert_eq!(Sgr::Reset.to_string(), "\x1b[m");
    assert_eq!(Sgr::Bold(true).to_string(), "\x1b[1m");
    assert_eq!(Sgr::Foreground(Color::Reset).to_string(), "\x1b[39m");
    assert_eq!(Sgr::Foreground(Color::Palette(1)).to_string(), "\x1b[31m");
    assert_eq!(Sgr::Background(Color::Palette(9)).to_string(), "\x1b[101m");
    assert_eq!(
      Sgr::Background(Color::Palette(200)).to_string(),
      "\x1b[48;5;200m"
    );
    assert_eq!(
      Sgr::Foreground(Color::Rgb(1, 2, 3)).to_string(),
      "\x1b[38;2;1;2;3m"
    );
    assert_eq!(Sgr::Underline(Underline::Curly).to_string(), "\x1b[4:3m");
    assert_eq!(
      Sgr::UnderlineColor(Color::Rgb(255, 0, 0)).to_string(),
      "\x1b[58:2::255:0:0m"
    );
    assert_eq!(Sgr::UnderlineColor(Color::Reset).to_string(), "\x1b[59m");
    assert_eq!(Sgr::Overline(false).to_string(), "\x1b[55m");
  }

  /// The graphics commands write their keys: the query, a transmission in chunks of
  /// at most four thousand and ninety-six characters with the last chunk marked, a
  /// placement with its crop, and the deletions.
  #[test]
  fn graphics_commands_write_their_keys() {
    assert_eq!(
      GraphicsQuery(31).to_string(),
      "\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\"
    );

    let mut out = Vec::new();

    write_transmit(&mut out, 5, 2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]).unwrap();

    assert_eq!(
      String::from_utf8(out).unwrap(),
      "\x1b_Ga=t,f=32,s=2,v=1,i=5,q=2,m=0;/wAA/wAA//8=\x1b\\"
    );

    let mut out = Vec::new();
    let pixels = vec![7; 3 * 4096];

    write_transmit(&mut out, 5, 64, 48, &pixels).unwrap();

    let out = String::from_utf8(out).unwrap();

    assert_eq!(out.matches("\x1b_G").count(), 4);
    assert!(out.starts_with("\x1b_Ga=t,f=32,s=64,v=48,i=5,q=2,m=1;"));
    assert!(out.contains("\x1b\\\x1b_Gm=1;"));
    assert!(out.contains("\x1b\\\x1b_Gm=0;"));
    assert_eq!(
      Place {
        id: 5,
        columns: 20,
        rows: 4,
        crop: (10, 0, 200, 80),
        layer: -7,
      }
      .to_string(),
      "\x1b_Ga=p,i=5,p=1,c=20,r=4,x=10,y=0,w=200,h=80,z=-7,C=1,q=2\x1b\\"
    );
    assert_eq!(Unplace(5).to_string(), "\x1b_Ga=d,d=i,i=5,q=2\x1b\\");
    assert_eq!(Free(5).to_string(), "\x1b_Ga=d,d=I,i=5,q=2\x1b\\");
  }

  /// A clipboard write opens, carries the text as plain text and closes.
  #[test]
  fn a_clipboard_write_is_framed() {
    let mut out = Vec::new();

    write_clipboard(&mut out, "hi").unwrap();

    assert_eq!(
      String::from_utf8(out).unwrap(),
      "\x1b]5522;type=write\x1b\\\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\\x1b]5522;type=wdata\x1b\\"
    );
  }

  /// Base64 round-trips, and decodes without padding.
  #[test]
  fn base64_round_trips() {
    assert_eq!(base64::encode(b"hello"), "aGVsbG8=");
    assert_eq!(base64::encode(b"hi"), "aGk=");
    assert_eq!(base64::encode(b"hey"), "aGV5");
    assert_eq!(base64::encode(b""), "");
    assert_eq!(base64::decode("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
    assert_eq!(base64::decode("aGk").as_deref(), Some(&b"hi"[..]));
    assert_eq!(base64::decode("a*"), None);
  }
}
