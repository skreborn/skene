//! Terminal capability detection—color depth, background darkness, and clipboard
//! support—and the clipboard read that shares its reply channel.
//!
//! Every detection degrades quietly: an unresponsive or unknown terminal gets 16
//! colors, a dark background, a working clipboard write and no clipboard read. The
//! background and clipboard queries run on their own thread and report through a
//! channel, so the loop starts before they answer; a clipboard read runs the same way.

use std::{env, sync::mpsc, time::Duration};
#[cfg(unix)]
use std::{io::Write, time::Instant};

use crossterm::style::Color as TerminalColor;

/// How many colors the terminal renders; painted RGB quantizes down to this depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorDepth {
  /// 24-bit RGB.
  TrueColor,
  /// The xterm 256-color palette.
  Ansi256,
  /// The 16 base colors.
  Ansi16,
}

impl ColorDepth {
  /// Detects the terminal's color depth from `COLORTERM` and `TERM`.
  pub(crate) fn detect() -> Self {
    Self::from_variables(
      env::var("COLORTERM").ok().as_deref(),
      env::var("TERM").ok().as_deref(),
    )
  }

  /// The depth `COLORTERM` and `TERM` values name; 16 colors when neither says more.
  fn from_variables(colorterm: Option<&str>, term: Option<&str>) -> Self {
    let truecolor = colorterm
      .is_some_and(|value| value.contains("truecolor") || value.contains("24bit"))
      || term.is_some_and(|value| value.contains("direct"));

    if truecolor {
      return Self::TrueColor;
    }

    if term.is_some_and(|value| value.contains("256")) {
      return Self::Ansi256;
    }

    Self::Ansi16
  }

  /// The nearest color the terminal renders at this depth.
  pub(crate) fn terminal_color(self, (red, green, blue): (u8, u8, u8)) -> TerminalColor {
    match self {
      Self::TrueColor => TerminalColor::Rgb {
        r: red,
        g: green,
        b: blue,
      },
      Self::Ansi256 => TerminalColor::AnsiValue(nearest_256(red, green, blue)),
      Self::Ansi16 => TerminalColor::AnsiValue(nearest_16(red, green, blue)),
    }
  }
}

/// What the asynchronous detection found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Detected {
  /// Whether the terminal background is dark.
  pub(crate) dark: bool,
  /// Whether OSC 52 clipboard writes reach the system clipboard.
  pub(crate) clipboard: bool,
  /// How long an OSC 52 clipboard read waits for the terminal's reply, `None` when
  /// the terminal answers none.
  pub(crate) clipboard_read: Option<Duration>,
}

/// How long a detection query waits for its reply.
const QUERY_WAIT: Duration = Duration::from_millis(100);

/// How long a clipboard read waits for a terminal that answers at once.
const READ_WAIT: Duration = Duration::from_millis(300);

/// How long a clipboard read waits for a terminal that asks the user first.
const ASKED_READ_WAIT: Duration = Duration::from_secs(10);

/// The longest clipboard reply read, in bytes.
const READ_LIMIT: usize = 4 * 1024 * 1024;

/// Starts the background and clipboard detection on its own thread; the answer arrives
/// on the returned channel once, then the channel closes.
///
/// Call with raw mode enabled and before the loop reads input: the replies arrive on
/// stdin and must not echo or be read by anything else.
pub(crate) fn start() -> mpsc::Receiver<Detected> {
  let (sender, receiver) = mpsc::channel();

  std::thread::spawn(move || {
    let terminal = terminal_version();

    let _ = sender.send(Detected {
      dark: background_is_dark(),
      clipboard: clipboard_writes_work(terminal.as_deref()),
      clipboard_read: clipboard_read_wait(terminal.as_deref()),
    });
  });

  receiver
}

/// Starts an OSC 52 clipboard read on its own thread, waiting `wait` for the reply;
/// the text arrives on the returned channel once—`None` when the terminal answered
/// nothing—then the channel closes.
///
/// Call while the loop leaves stdin alone: the reply arrives there.
pub(crate) fn read_clipboard(wait: Duration) -> mpsc::Receiver<Option<String>> {
  let (sender, receiver) = mpsc::channel();

  std::thread::spawn(move || {
    let _ = sender.send(query_clipboard(wait));
  });

  receiver
}

/// The squared distance between two RGB colors.
const fn distance(
  (red, green, blue): (u8, u8, u8),
  (other_red, other_green, other_blue): (u8, u8, u8),
) -> i32 {
  let delta_red = red as i32 - other_red as i32;
  let delta_green = green as i32 - other_green as i32;
  let delta_blue = blue as i32 - other_blue as i32;

  delta_red * delta_red + delta_green * delta_green + delta_blue * delta_blue
}

/// The xterm 256-palette index nearest to an RGB color.
fn nearest_256(red: u8, green: u8, blue: u8) -> u8 {
  const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

  let level = |component: u8| {
    LEVELS
      .iter()
      .enumerate()
      .min_by_key(|(_, level)| (i32::from(component) - i32::from(**level)).abs())
      .map_or(0, |(index, _)| index)
  };

  let (red_index, green_index, blue_index) = (level(red), level(green), level(blue));
  let cube_rgb = (LEVELS[red_index], LEVELS[green_index], LEVELS[blue_index]);
  #[expect(clippy::cast_possible_truncation)]
  let cube = 16 + 36 * red_index as u8 + 6 * green_index as u8 + blue_index as u8;

  let average = (u16::from(red) + u16::from(green) + u16::from(blue)) / 3;
  let step = ((i32::from(average) - 8 + 5) / 10).clamp(0, 23);
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let gray_level = (8 + 10 * step) as u8;
  #[expect(clippy::cast_sign_loss)]
  let gray = 232 + step as u8;

  let target = (red, green, blue);

  if distance(target, (gray_level, gray_level, gray_level)) < distance(target, cube_rgb) {
    gray
  }
  else {
    cube
  }
}

/// The 16-color palette index nearest to an RGB color, against the xterm defaults.
fn nearest_16(red: u8, green: u8, blue: u8) -> u8 {
  const PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
  ];

  let target = (red, green, blue);

  #[expect(clippy::cast_possible_truncation)]
  PALETTE
    .iter()
    .enumerate()
    .min_by_key(|(_, candidate)| distance(target, **candidate))
    .map_or(0, |(index, _)| index as u8)
}

/// Whether the terminal background is dark: an OSC 11 query first, `COLORFGBG` when the
/// terminal stays silent, dark when neither answers.
fn background_is_dark() -> bool {
  if let Some((red, green, blue)) = query_background() {
    return is_dark(red, green, blue);
  }

  env::var("COLORFGBG")
    .ok()
    .and_then(|value| colorfgbg_is_dark(&value))
    .unwrap_or(true)
}

/// Whether an RGB background counts as dark, by Rec. 709 luminance.
fn is_dark(red: u8, green: u8, blue: u8) -> bool {
  let luminance = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);

  luminance < 127.5
}

/// Whether a `COLORFGBG` value (`"fg;bg"`, possibly with segments between) names a dark
/// background.
fn colorfgbg_is_dark(value: &str) -> Option<bool> {
  let background: u8 = value.rsplit(';').next()?.trim().parse().ok()?;

  Some(!matches!(background, 7 | 15))
}

/// Queries the terminal background via OSC 11, waiting briefly for the reply on stdin.
#[cfg(unix)]
fn query_background() -> Option<(u8, u8, u8)> {
  parse_osc11(&query("\x1b]11;?\x1b\\", b"\x1b]11;", QUERY_WAIT, 64)?)
}

/// No reply channel outside a Unix terminal.
#[cfg(not(unix))]
fn query_background() -> Option<(u8, u8, u8)> {
  None
}

/// Reads the system clipboard via an OSC 52 query, waiting `wait` for the reply on
/// stdin.
#[cfg(unix)]
fn query_clipboard(wait: Duration) -> Option<String> {
  parse_osc52(&query("\x1b]52;c;?\x1b\\", b"\x1b]52;", wait, READ_LIMIT)?)
}

/// No reply channel outside a Unix terminal.
#[cfg(not(unix))]
fn query_clipboard(_wait: Duration) -> Option<String> {
  None
}

/// Whether OSC 52 clipboard writes reach the system clipboard: every terminal is
/// assumed capable except one identifying as VTE, which discards them.
fn clipboard_writes_work(terminal: Option<&str>) -> bool {
  terminal.is_none_or(|terminal| !terminal.starts_with("VTE("))
}

/// How long an OSC 52 clipboard read waits for the terminal named by XTVERSION: the
/// terminals answering reads at once wait briefly, the ones asking the user first wait
/// for the answer, and every other terminal—answering nothing—reads no clipboard.
fn clipboard_read_wait(terminal: Option<&str>) -> Option<Duration> {
  let name = terminal?.to_ascii_lowercase();

  if ["kitty", "ghostty"]
    .iter()
    .any(|asks| name.starts_with(asks))
  {
    return Some(ASKED_READ_WAIT);
  }

  ["foot", "xterm", "tmux", "contour"]
    .iter()
    .any(|answers| name.starts_with(answers))
    .then_some(READ_WAIT)
}

/// The terminal's name and version via XTVERSION (`NAME(VERSION)`), when it answers.
#[cfg(unix)]
fn terminal_version() -> Option<String> {
  parse_xtversion(&query("\x1b[>0q", b"\x1bP>|", QUERY_WAIT, 64)?)
}

/// No reply channel outside a Unix terminal.
#[cfg(not(unix))]
fn terminal_version() -> Option<String> {
  None
}

/// Writes `request` and reads the reply that starts with `prefix` from stdin, waiting
/// `wait` for it and reading at most `limit` bytes; the reply ends at BEL or ST.
#[cfg(unix)]
fn query(request: &str, prefix: &[u8], wait: Duration, limit: usize) -> Option<Vec<u8>> {
  let mut out = std::io::stdout();

  write!(out, "{request}").ok()?;
  out.flush().ok()?;

  let deadline = Instant::now() + wait;
  let mut response = Vec::new();

  loop {
    let remaining = deadline.checked_duration_since(Instant::now())?;
    let mut descriptor = libc::pollfd {
      fd: 0,
      events: libc::POLLIN,
      revents: 0,
    };
    let timeout = i32::try_from(remaining.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `descriptor` is a valid, initialized `pollfd` living for the call.
    let ready = unsafe { libc::poll(&raw mut descriptor, 1, timeout) };

    if ready <= 0 {
      return None;
    }

    let mut byte = 0u8;
    // SAFETY: `byte` is a valid one-byte buffer living for the call.
    let count = unsafe { libc::read(0, std::ptr::from_mut(&mut byte).cast(), 1) };

    if count != 1 {
      return None;
    }

    response.push(byte);

    // Anything that stops being the reply's prefix is unrelated input: stop consuming
    // rather than eat the user's first keystrokes.
    if response.len() <= prefix.len() && response != prefix[..response.len()] {
      return None;
    }

    if byte == 0x07 || response.ends_with(b"\x1b\\") {
      return Some(response);
    }

    if response.len() > limit {
      return None;
    }
  }
}

/// Parses an OSC 52 reply (`ESC ] 52 ; selections ; base64 BEL|ST`) to its text; an
/// empty payload is a denied or empty clipboard.
#[cfg(any(unix, test))]
fn parse_osc52(response: &[u8]) -> Option<String> {
  let text = std::str::from_utf8(response).ok()?;
  let payload = text
    .strip_prefix("\x1b]52;")?
    .trim_end_matches('\x07')
    .trim_end_matches("\x1b\\")
    .split_once(';')
    .map_or("", |(_, payload)| payload);
  let bytes = base64_decode(payload)?;

  (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
}

/// Standard base64 decoding, padding optional, `None` on a foreign character.
#[cfg(any(unix, test))]
fn base64_decode(text: &str) -> Option<Vec<u8>> {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

  let mut out = Vec::with_capacity(text.len() / 4 * 3);
  let mut block = 0u32;
  let mut filled = 0u32;

  for byte in text.bytes() {
    if byte == b'=' || byte.is_ascii_whitespace() {
      continue;
    }

    let value = u32::try_from(ALPHABET.iter().position(|letter| *letter == byte)?).ok()?;

    block = (block << 6) | value;
    filled += 6;

    if filled >= 8 {
      filled -= 8;
      out.push(u8::try_from((block >> filled) & 0xff).ok()?);
      block &= (1 << filled) - 1;
    }
  }

  Some(out)
}

/// Parses an XTVERSION reply (`DCS > | NAME(VERSION) ST`) to its `NAME(VERSION)` text.
#[cfg(any(unix, test))]
fn parse_xtversion(response: &[u8]) -> Option<String> {
  let text = std::str::from_utf8(response).ok()?;
  let inner = text
    .strip_prefix("\x1bP>|")?
    .trim_end_matches('\x07')
    .trim_end_matches("\x1b\\");

  (!inner.is_empty()).then(|| inner.to_owned())
}

/// Parses an OSC 11 reply (`ESC ] 11 ; rgb : rrrr / gggg / bbbb BEL|ST`) to RGB.
#[cfg(any(unix, test))]
fn parse_osc11(response: &[u8]) -> Option<(u8, u8, u8)> {
  let text = std::str::from_utf8(response).ok()?;
  let spec = text
    .split("]11;")
    .nth(1)?
    .trim_start_matches("rgba:")
    .trim_start_matches("rgb:");
  let mut parts = spec.split('/');
  let mut component = || {
    let part = parts.next()?;
    let digits: String = part.chars().take_while(char::is_ascii_hexdigit).collect();

    if digits.is_empty() || digits.len() > 4 {
      return None;
    }

    let value = u32::from_str_radix(&digits, 16).ok()?;
    let maximum = (1u32 << (4 * digits.len())) - 1;

    #[expect(clippy::cast_possible_truncation)]
    Some((value * 255 / maximum) as u8)
  };

  Some((component()?, component()?, component()?))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn depth_reads_the_variables() {
    assert_eq!(
      ColorDepth::from_variables(Some("truecolor"), Some("xterm-256color")),
      ColorDepth::TrueColor
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("xterm-direct")),
      ColorDepth::TrueColor
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("xterm-256color")),
      ColorDepth::Ansi256
    );
    assert_eq!(
      ColorDepth::from_variables(None, Some("vt100")),
      ColorDepth::Ansi16
    );
    assert_eq!(ColorDepth::from_variables(None, None), ColorDepth::Ansi16);
  }

  #[test]
  fn quantization_finds_the_nearest_palette_entry() {
    assert_eq!(nearest_256(255, 0, 0), 196);
    assert_eq!(nearest_256(255, 255, 255), 231);
    assert_eq!(nearest_256(128, 128, 128), 244);
    assert_eq!(nearest_16(0, 0, 0), 0);
    assert_eq!(nearest_16(255, 0, 0), 9);
    assert_eq!(nearest_16(255, 255, 255), 15);
  }

  #[test]
  fn osc11_replies_parse_at_any_component_width() {
    assert_eq!(
      parse_osc11(b"\x1b]11;rgb:1e1e/2e2e/3e3e\x07"),
      Some((30, 46, 62))
    );
    assert_eq!(
      parse_osc11(b"\x1b]11;rgb:ff/80/00\x1b\\"),
      Some((255, 128, 0))
    );
    assert_eq!(
      parse_osc11(b"\x1b]11;rgba:ffff/0000/0000\x07"),
      Some((255, 0, 0))
    );
    assert_eq!(parse_osc11(b"garbage"), None);
  }

  #[test]
  fn xtversion_replies_parse_and_identify_vte() {
    assert_eq!(
      parse_xtversion(b"\x1bP>|VTE(7802)\x1b\\").as_deref(),
      Some("VTE(7802)")
    );
    assert_eq!(
      parse_xtversion(b"\x1bP>|kitty(0.32.1)\x1b\\").as_deref(),
      Some("kitty(0.32.1)")
    );
    assert_eq!(parse_xtversion(b"garbage"), None);
  }

  #[test]
  fn clipboard_reads_wait_on_the_terminals_answering_them() {
    assert_eq!(
      clipboard_read_wait(Some("kitty(0.32.1)")),
      Some(ASKED_READ_WAIT)
    );
    assert_eq!(clipboard_read_wait(Some("foot(1.16.2)")), Some(READ_WAIT));
    assert_eq!(clipboard_read_wait(Some("XTerm(390)")), Some(READ_WAIT));
    assert_eq!(clipboard_read_wait(Some("tmux 3.4")), Some(READ_WAIT));
    assert_eq!(clipboard_read_wait(Some("WezTerm 20240203")), None);
    assert_eq!(clipboard_read_wait(Some("VTE(7802)")), None);
    assert_eq!(clipboard_read_wait(None), None);
    assert!(clipboard_writes_work(None));
    assert!(!clipboard_writes_work(Some("VTE(7802)")));
  }

  #[test]
  fn osc52_replies_decode_their_payload() {
    assert_eq!(
      parse_osc52(b"\x1b]52;c;aGVsbG8=\x07").as_deref(),
      Some("hello")
    );
    assert_eq!(
      parse_osc52(b"\x1b]52;c;aGk\x1b\\").as_deref(),
      Some("hi"),
      "padding is optional"
    );
    assert_eq!(
      parse_osc52(b"\x1b]52;c;\x07"),
      None,
      "an empty payload is no text"
    );
    assert_eq!(
      parse_osc52(b"\x1b]52;c;!!\x07"),
      None,
      "foreign characters fail"
    );
    assert_eq!(parse_osc52(b"garbage"), None);
  }

  #[test]
  fn colorfgbg_names_the_background() {
    assert_eq!(colorfgbg_is_dark("15;0"), Some(true));
    assert_eq!(colorfgbg_is_dark("0;15"), Some(false));
    assert_eq!(colorfgbg_is_dark("15;default;7"), Some(false));
    assert_eq!(colorfgbg_is_dark("nonsense"), None);
  }

  #[test]
  fn luminance_splits_dark_from_light() {
    assert!(is_dark(30, 30, 46));
    assert!(!is_dark(255, 255, 255));
  }
}
