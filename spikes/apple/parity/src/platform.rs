//! What differs between AppKit and UIKit, behind one set of names: the font and colour classes,
//! TextKit 2's classes (the same names in both frameworks, generated into two crates), the
//! system font at a weight, and the drawing context TextKit draws into.

use objc2::rc::Retained;
#[cfg(target_os = "macos")]
pub use objc2_app_kit::{
  NSColor as Color,
  NSFont as Font,
  NSFontAttributeName,
  NSForegroundColorAttributeName,
  NSLineBreakMode,
  NSLineBreakStrategy,
  NSMutableParagraphStyle,
  NSParagraphStyleAttributeName,
  NSTextAlignment,
  NSTextContainer,
  NSTextContentStorage,
  NSTextElementProvider,
  NSTextLayoutFragment,
  NSTextLayoutFragmentEnumerationOptions,
  NSTextLayoutManager,
  NSTextLayoutManagerSegmentOptions,
  NSTextLayoutManagerSegmentType,
  NSTextLineFragment,
  NSTextLocation,
  NSTextRange,
  NSWritingDirection,
};
use objc2_core_foundation::CGFloat;
use objc2_core_graphics::CGContext;
use objc2_core_text::CTFont;
#[cfg(target_os = "ios")]
pub use objc2_ui_kit::{
  NSFontAttributeName,
  NSForegroundColorAttributeName,
  NSLineBreakMode,
  NSLineBreakStrategy,
  NSMutableParagraphStyle,
  NSParagraphStyleAttributeName,
  NSTextAlignment,
  NSTextContainer,
  NSTextContentStorage,
  NSTextElementProvider,
  NSTextLayoutFragment,
  NSTextLayoutFragmentEnumerationOptions,
  NSTextLayoutManager,
  NSTextLayoutManagerSegmentOptions,
  NSTextLayoutManagerSegmentType,
  NSTextLineFragment,
  NSTextLocation,
  NSTextRange,
  NSWritingDirection,
  UIColor as Color,
  UIFont as Font,
};

/// The platform's name: `macos` or `ios`.
pub const OS: &str = if cfg!(target_os = "ios") {
  "ios"
}
else {
  "macos"
};

/// The system font weight (`NSFontWeight`, `UIFontWeight`) for a CSS weight.
#[must_use]
pub fn ui_weight(weight: u16) -> CGFloat {
  match weight {
    0..=149 => -0.8,
    150..=249 => -0.6,
    250..=349 => -0.4,
    350..=449 => 0.0,
    450..=549 => 0.23,
    550..=649 => 0.3,
    650..=749 => 0.4,
    750..=849 => 0.56,
    _ => 0.62,
  }
}

/// The system UI font at `size` points, `weight` (CSS) and, where asked, italic.
#[must_use]
pub fn system_font(size: f64, weight: u16, italic: bool) -> Retained<Font> {
  let font = Font::systemFontOfSize_weight(size, ui_weight(weight));

  if !italic {
    return font;
  }

  italicize(&font, size).unwrap_or(font)
}

#[cfg(target_os = "macos")]
fn italicize(font: &Font, size: f64) -> Option<Retained<Font>> {
  use objc2_app_kit::NSFontDescriptorSymbolicTraits;

  let descriptor = font.fontDescriptor();
  let traits = descriptor.symbolicTraits() | NSFontDescriptorSymbolicTraits::TraitItalic;
  let italic = descriptor.fontDescriptorWithSymbolicTraits(traits);

  Font::fontWithDescriptor_size(&italic, size)
}

#[cfg(target_os = "ios")]
fn italicize(font: &Font, size: f64) -> Option<Retained<Font>> {
  use objc2_ui_kit::UIFontDescriptorSymbolicTraits;

  let descriptor = unsafe { font.fontDescriptor() };
  let traits = unsafe { descriptor.symbolicTraits() } | UIFontDescriptorSymbolicTraits::TraitItalic;
  let italic = descriptor.fontDescriptorWithSymbolicTraits(traits)?;

  Some(Font::fontWithDescriptor_size(&italic, size))
}

/// The platform font as the Core Text font it is (the two are toll-free bridged).
#[must_use]
pub fn as_ct(font: &Font) -> &CTFont {
  unsafe { &*std::ptr::from_ref(font).cast::<CTFont>() }
}

/// A Core Text font as the platform font it is.
///
/// # Safety
///
/// `font` must be an `NSFont`/`UIFont` instance (one the platform created), which is true of
/// every font TextKit hands back.
#[must_use]
pub unsafe fn from_ct(font: &CTFont) -> &Font {
  unsafe { &*std::ptr::from_ref(font).cast::<Font>() }
}

/// Black or white.
#[must_use]
pub fn gray(white: bool) -> Retained<Color> {
  if white {
    Color::whiteColor()
  }
  else {
    Color::blackColor()
  }
}

/// Runs `body` with `context` as the current drawing context, flipped (y down) as TextKit 2
/// expects.
#[cfg(target_os = "macos")]
pub fn with_context<R>(context: &CGContext, body: impl FnOnce() -> R) -> R {
  use objc2_app_kit::NSGraphicsContext;

  let previous = NSGraphicsContext::currentContext();
  let current = NSGraphicsContext::graphicsContextWithCGContext_flipped(context, true);

  NSGraphicsContext::setCurrentContext(Some(&current));

  let result = body();

  NSGraphicsContext::setCurrentContext(previous.as_deref());

  result
}

/// Runs `body` with `context` as the current drawing context, flipped (y down) as TextKit 2
/// expects.
#[cfg(target_os = "ios")]
pub fn with_context<R>(context: &CGContext, body: impl FnOnce() -> R) -> R {
  unsafe { objc2_ui_kit::UIGraphicsPushContext(context) };

  let result = body();

  unsafe { objc2_ui_kit::UIGraphicsPopContext() };

  result
}

/// The text styles' names (UIKit's, shared by AppKit), in order.
pub const STYLE_NAMES: [&str; 11] = [
  "largeTitle",
  "title1",
  "title2",
  "title3",
  "headline",
  "subheadline",
  "body",
  "callout",
  "footnote",
  "caption1",
  "caption2",
];

/// Every text style's preferred font, by name.
#[cfg(target_os = "macos")]
#[must_use]
pub fn text_styles() -> Vec<(String, Retained<Font>)> {
  use objc2_app_kit::{
    NSFontTextStyleBody,
    NSFontTextStyleCallout,
    NSFontTextStyleCaption1,
    NSFontTextStyleCaption2,
    NSFontTextStyleFootnote,
    NSFontTextStyleHeadline,
    NSFontTextStyleLargeTitle,
    NSFontTextStyleSubheadline,
    NSFontTextStyleTitle1,
    NSFontTextStyleTitle2,
    NSFontTextStyleTitle3,
  };
  use objc2_foundation::NSDictionary;

  let styles = unsafe {
    [
      NSFontTextStyleLargeTitle,
      NSFontTextStyleTitle1,
      NSFontTextStyleTitle2,
      NSFontTextStyleTitle3,
      NSFontTextStyleHeadline,
      NSFontTextStyleSubheadline,
      NSFontTextStyleBody,
      NSFontTextStyleCallout,
      NSFontTextStyleFootnote,
      NSFontTextStyleCaption1,
      NSFontTextStyleCaption2,
    ]
  };
  let options = NSDictionary::new();

  STYLE_NAMES
    .iter()
    .zip(styles)
    .map(|(name, style)| {
      ((*name).to_owned(), unsafe {
        Font::preferredFontForTextStyle_options(style, &options)
      })
    })
    .collect()
}

/// Every text style's preferred font, by name.
#[cfg(target_os = "ios")]
#[must_use]
pub fn text_styles() -> Vec<(String, Retained<Font>)> {
  STYLE_NAMES
    .iter()
    .zip(ui_styles())
    .map(|(name, style)| ((*name).to_owned(), Font::preferredFontForTextStyle(style)))
    .collect()
}

/// UIKit's text styles, in [`STYLE_NAMES`] order.
#[cfg(target_os = "ios")]
#[must_use]
pub fn ui_styles() -> [&'static objc2_ui_kit::UIFontTextStyle; 11] {
  use objc2_ui_kit::{
    UIFontTextStyleBody,
    UIFontTextStyleCallout,
    UIFontTextStyleCaption1,
    UIFontTextStyleCaption2,
    UIFontTextStyleFootnote,
    UIFontTextStyleHeadline,
    UIFontTextStyleLargeTitle,
    UIFontTextStyleSubheadline,
    UIFontTextStyleTitle1,
    UIFontTextStyleTitle2,
    UIFontTextStyleTitle3,
  };

  unsafe {
    [
      UIFontTextStyleLargeTitle,
      UIFontTextStyleTitle1,
      UIFontTextStyleTitle2,
      UIFontTextStyleTitle3,
      UIFontTextStyleHeadline,
      UIFontTextStyleSubheadline,
      UIFontTextStyleBody,
      UIFontTextStyleCallout,
      UIFontTextStyleFootnote,
      UIFontTextStyleCaption1,
      UIFontTextStyleCaption2,
    ]
  }
}

/// The platform font's own line metrics, as a report fragment.
#[cfg(target_os = "macos")]
#[must_use]
pub fn font_line_metrics(font: &Font) -> String {
  format!(
    "; NSFont ascender {:.2} descender {:.2} leading {:.2}",
    font.ascender(),
    font.descender(),
    font.leading()
  )
}

/// The platform font's own line metrics, as a report fragment.
#[cfg(target_os = "ios")]
#[must_use]
pub fn font_line_metrics(font: &Font) -> String {
  unsafe {
    format!(
      "; UIFont ascender {:.2} descender {:.2} leading {:.2} lineHeight {:.2}",
      font.ascender(),
      font.descender(),
      font.leading(),
      font.lineHeight()
    )
  }
}
