//! The probes the Windows half of the text-engine revisit asks for, each printing `key: value`
//! lines: fallback per locale (A's layout, the per-run resolver and stock fontique side by side),
//! Windows's recommended UI font per language, Segoe UI Variable and its automatic optical size,
//! line metrics per UI family, shaping parity, break opportunities in Thai, Lao, Khmer and
//! Burmese (DirectWrite, Parley and the system ICU), fontique's start-up and memory, text scaling,
//! and a per-user font.

use std::{borrow::Cow, path::Path, time::Instant};

use parley::{
  FontFamily,
  FontFamilyName,
  FontVariation,
  FontVariations,
  FontWeight,
  Layout,
  LayoutContext,
  LineHeight,
  OverflowWrap,
  PositionedLayoutItem,
  StyleProperty,
  TextWrapMode,
  fontique::Language,
  setting::Tag,
};
use text_parity::{
  bench::plain_case,
  corpus::{Corpus, Variant},
  fonts::Fonts,
  layout::{Options as LayoutOptions, ParleyEngine},
  text::Offsets,
};
use windows::{
  Globalization::Fonts::LanguageFontGroup,
  UI::ViewManagement::UISettings,
  Win32::{
    Globalization::{UBRK_LINE, UErrorCode, ubrk_close, ubrk_next, ubrk_open},
    Graphics::DirectWrite::*,
    System::{
      ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
      Threading::GetCurrentProcess,
    },
  },
};
use windows_core::{HSTRING, Interface, PCSTR, PCWSTR};

use crate::{
  Report,
  engine::{collect, line_metrics, wide},
  faces::axis_tag,
  native::{SYSTEM_FAMILY, face_metrics, family_name, localized},
  paint::face_path,
  parity::{Msg, Res},
  resolver::Resolver,
};

fn ms(started: Instant) -> f64 {
  started.elapsed().as_secs_f64() * 1e3
}

/// The process's working set and private bytes, in MB.
pub fn memory() -> (f64, f64) {
  let mut c = PROCESS_MEMORY_COUNTERS {
    cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
    ..Default::default()
  };
  let _ = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };

  (
    c.WorkingSetSize as f64 / 1048576.0,
    c.PagefileUsage as f64 / 1048576.0,
  )
}

/// One line of text through DirectWrite, unwrapped.
fn dwrite_layout(
  factory: &IDWriteFactory4,
  text: &str,
  family: &str,
  size: f32,
  weight: i32,
  locale: &str,
  wrap: DWRITE_WORD_WRAPPING,
  width: f32,
) -> Res<IDWriteTextLayout> {
  let family = wide(family);
  let locale = wide(locale);
  let utf16: Vec<u16> = text.encode_utf16().collect();

  unsafe {
    let format = IDWriteFactory::CreateTextFormat(
      factory,
      PCWSTR(family.as_ptr()),
      None,
      DWRITE_FONT_WEIGHT(weight),
      DWRITE_FONT_STYLE_NORMAL,
      DWRITE_FONT_STRETCH_NORMAL,
      size,
      PCWSTR(locale.as_ptr()),
    )
    .msg("format")?;
    format.SetWordWrapping(wrap).msg("wrapping")?;
    IDWriteFactory::CreateTextLayout(factory, &utf16, &format, width, 1.0e6).msg("layout")
  }
}

/// A face named for a report: family, file name and face index.
fn face_label(face: &IDWriteFontFace) -> String {
  let family = face
    .cast::<IDWriteFontFace3>()
    .ok()
    .and_then(|f| family_name(&f).ok())
    .unwrap_or_default();
  let path = face_path(face).unwrap_or_default();
  let file = path.rsplit('\\').next().unwrap_or_default().to_owned();
  let sims = unsafe { face.GetSimulations() }.0;

  format!(
    "{family}({file}#{}{})",
    unsafe { face.GetIndex() },
    if sims != 0 {
      format!(" sim{sims}")
    }
    else {
      String::new()
    }
  )
}

/// DirectWrite's run fonts over `text`: UTF-16 start, length, face.
fn dwrite_fonts(layout: &IDWriteTextLayout, text: &str) -> Res<Vec<String>> {
  let c = collect(layout, 0.0, 0.0, 1.0, false).msg("collect")?;
  let utf16: Vec<u16> = text.encode_utf16().collect();

  Ok(
    c.runs
      .iter()
      .filter(|r| !r.inline)
      .map(|r| {
        let s = r.text_position as usize;
        let e = (s + r.cluster_map.len()).min(utf16.len());
        format!(
          "{:?}={}",
          String::from_utf16_lossy(&utf16[s..e]),
          face_label(&r.face)
        )
      })
      .collect(),
  )
}

/// A Parley layout of one paragraph, every knob explicit.
pub struct Direct {
  pub fonts: Fonts,
  layouts: LayoutContext<()>,
}

impl Direct {
  pub fn new(fonts: Fonts) -> Self {
    Self {
      fonts,
      layouts: LayoutContext::new(),
    }
  }

  #[expect(clippy::too_many_arguments)]
  pub fn lay_out(
    &mut self,
    text: &str,
    family: &str,
    size: f32,
    weight: f32,
    locale: &str,
    variations: &[(&str, f32)],
    wrap_normal: bool,
    width: Option<f32>,
  ) -> Layout<()> {
    let mut builder = self
      .layouts
      .ranged_builder(&mut self.fonts.context, text, 1.0, false);
    builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
      if family == "system-ui" {
        vec![
          FontFamilyName::Generic(parley::GenericFamily::SystemUi),
          FontFamilyName::Generic(parley::GenericFamily::SansSerif),
        ]
      }
      else {
        vec![FontFamilyName::Named(Cow::Owned(family.to_owned()))]
      },
    ))));
    builder.push_default(StyleProperty::FontSize(size));
    builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
    builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(1.0)));
    builder.push_default(StyleProperty::OverflowWrap(if wrap_normal {
      OverflowWrap::Normal
    }
    else {
      OverflowWrap::BreakWord
    }));
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::Wrap));

    if let Ok(language) = Language::parse(locale) {
      builder.push_default(StyleProperty::Locale(Some(language)));
    }

    if !variations.is_empty() {
      let list: Vec<FontVariation> = variations
        .iter()
        .filter_map(|(tag, value)| {
          Some(FontVariation::new(
            Tag::new(tag.as_bytes().try_into().ok()?),
            *value,
          ))
        })
        .collect();
      builder.push_default(StyleProperty::FontVariations(FontVariations::List(
        Cow::Owned(list),
      )));
    }

    let mut layout = builder.build(text);
    layout.break_all_lines(width);
    layout
  }

  /// `text` in `system-ui` with a zero line height on the root style and the font metrics' on a
  /// span over the whole text.
  pub fn strutless(&mut self, text: &str, size: f32, locale: &str) -> Layout<()> {
    // The tree builder nests the span inside the root, so the root's zero line height is the
    // strut's alone; a ranged builder's whole-text range would restyle the root itself.
    let root = parley::TextStyle {
      font_family: FontFamily::List(Cow::Owned(vec![
        FontFamilyName::Generic(parley::GenericFamily::SystemUi),
        FontFamilyName::Generic(parley::GenericFamily::SansSerif),
      ])),
      font_size: size,
      line_height: LineHeight::FontSizeRelative(0.0),
      locale: Language::parse(locale).ok(),
      ..parley::TextStyle::default()
    };
    let mut builder = self
      .layouts
      .tree_builder(&mut self.fonts.context, 1.0, false, &root);
    builder
      .push_style_modification_span(&[StyleProperty::LineHeight(LineHeight::MetricsRelative(1.0))]);
    builder.push_text(text);
    builder.pop_style_span();
    let (mut layout, _) = builder.build();
    layout.break_all_lines(None);
    layout
  }

  /// `text` with a zero line height on the root and, over each range the fallback mapped, a span
  /// naming the mapped family first at the font metrics' line height: a line sized by the fonts
  /// its runs use, as DirectWrite sizes it.
  pub fn per_run_spans(
    &mut self,
    text: &str,
    size: f32,
    locale: &str,
    ranges: &[(std::ops::Range<usize>, Option<String>)],
  ) -> Layout<()> {
    let root = parley::TextStyle {
      font_family: FontFamily::List(Cow::Owned(vec![
        FontFamilyName::Generic(parley::GenericFamily::SystemUi),
        FontFamilyName::Generic(parley::GenericFamily::SansSerif),
      ])),
      font_size: size,
      line_height: LineHeight::FontSizeRelative(0.0),
      locale: Language::parse(locale).ok(),
      ..parley::TextStyle::default()
    };
    let mut builder = self
      .layouts
      .tree_builder(&mut self.fonts.context, 1.0, false, &root);

    for (range, family) in ranges {
      let mut families = Vec::new();

      if let Some(family) = family {
        families.push(FontFamilyName::Named(Cow::Owned(family.clone())));
      }

      families.push(FontFamilyName::Generic(parley::GenericFamily::SystemUi));
      families.push(FontFamilyName::Generic(parley::GenericFamily::SansSerif));
      builder.push_style_modification_span(&[
        StyleProperty::FontFamily(FontFamily::List(Cow::Owned(families))),
        StyleProperty::LineHeight(LineHeight::MetricsRelative(1.0)),
      ]);
      builder.push_text(&text[range.clone()]);
      builder.pop_style_span();
    }

    let (mut layout, _) = builder.build();
    layout.break_all_lines(None);
    layout
  }

  /// Parley's run fonts over `text`: the run's text and its font's family and file.
  pub fn fonts_of(&mut self, layout: &Layout<()>, text: &str) -> Vec<String> {
    let mut out = Vec::new();

    for line in layout.lines() {
      for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(g) = item {
          let run = g.run();
          let id = self.fonts.identity(run.font());
          let range = run.text_range();
          let file = id
            .path
            .as_deref()
            .and_then(|p| p.rsplit(['\\', '/']).next())
            .unwrap_or("?")
            .to_owned();
          let syn = run.synthesis();
          out.push(format!(
            "{:?}={}({file}#{}{}{})",
            &text[range.start.min(text.len())..range.end.min(text.len())],
            id.family,
            id.index,
            if syn.embolden() { " embolden" } else { "" },
            if syn.skew().is_some() { " skew" } else { "" },
          ));
        }
      }
    }

    out
  }
}

/// Fallback per locale: which font draws Han, an Extension B character and bold CJK.
pub fn fallback(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let resolver = Resolver::new(factory).msg("resolver")?;
  let han = "直骨角海写次化刃誤曜述返令";
  let cases: Vec<(&str, &str, &str, i32)> = vec![
    ("han.ja-JP", han, "ja-JP", 400),
    ("han.zh-CN", han, "zh-CN", 400),
    ("han.zh-Hans-CN", han, "zh-Hans-CN", 400),
    ("han.zh-TW", han, "zh-TW", 400),
    ("han.zh-Hant-TW", han, "zh-Hant-TW", 400),
    ("han.zh-HK", han, "zh-HK", 400),
    ("han.ko-KR", han, "ko-KR", 400),
    ("han.en-US", han, "en-US", 400),
    ("han.none", han, "", 400),
    ("extb.ja-JP", "\u{20B9F}\u{20BB7}野家", "ja-JP", 400),
    ("extb.zh-CN", "\u{20B9F}\u{20BB7}野家", "zh-CN", 400),
    ("extb.none", "\u{20B9F}\u{20BB7}野家", "", 400),
    ("bold.ja-JP", "太字の日本語漢字", "ja-JP", 700),
    ("bold.zh-CN", "粗体中文汉字", "zh-CN", 700),
    ("bold.ko-KR", "굵은 한국어", "ko-KR", 700),
    ("semibold.ja-JP", "日本語漢字", "ja-JP", 600),
    ("symbols.en-US", "→ ★ ✓ ∑ ♫ ⌘ ☎ ⚠", "en-US", 400),
    (
      "mixed.en-US",
      "English, 日本語, 한국어, العربية, עברית, ไทย, हिन्दी",
      "en-US",
      400,
    ),
  ];

  for (label, text, locale, weight) in cases {
    let layout = dwrite_layout(
      factory,
      text,
      SYSTEM_FAMILY,
      17.0,
      weight,
      locale,
      DWRITE_WORD_WRAPPING_NO_WRAP,
      1.0e6,
    )?;
    let a = dwrite_fonts(&layout, text)?;
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let mapped = resolver
      .map(text, locale, Some(SYSTEM_FAMILY), weight, false)
      .msg("map")?;
    let resolved: Vec<String> = mapped
      .iter()
      .map(|m| {
        let s = m.start as usize;
        let e = (s + m.len as usize).min(utf16.len());
        format!(
          "{:?}={}({}#{} w{} sim{} scale{})",
          String::from_utf16_lossy(&utf16[s..e]),
          m.family.clone().unwrap_or_else(|| "none".to_owned()),
          m.path
            .as_deref()
            .and_then(|p| p.rsplit('\\').next())
            .unwrap_or("?"),
          m.index,
          m.weight,
          m.simulations,
          m.scale
        )
      })
      .collect();
    let unbased = resolver
      .map(text, locale, None, 400, false)
      .msg("map")?
      .iter()
      .map(|m| m.family.clone().unwrap_or_else(|| "none".to_owned()))
      .collect::<Vec<_>>();
    let layout_b = direct.lay_out(
      text,
      "system-ui",
      17.0,
      weight as f32,
      if locale.is_empty() { "und" } else { locale },
      &[],
      false,
      None,
    );
    let b = direct.fonts_of(&layout_b, text);

    r.line(format!("fallback.{label}.a: {a:?}"));
    r.line(format!("fallback.{label}.resolver: {resolved:?}"));
    r.line(format!("fallback.{label}.unbased_regular: {unbased:?}"));
    r.line(format!("fallback.{label}.fontique: {b:?}"));
  }

  Ok(())
}

/// Windows's recommended UI font per language (`LanguageFontGroup.UITextFont`).
pub fn language_fonts(r: &mut Report) {
  for tag in [
    "en", "ja", "zh-Hans", "zh-Hant", "zh-HK", "ko", "th", "hi", "km", "lo", "my", "ar", "he",
    "bn", "ta",
  ] {
    let found = LanguageFontGroup::CreateLanguageFontGroup(&HSTRING::from(tag)).and_then(|g| {
      let ui = g.UITextFont()?;
      let doc = g.ModernDocumentFont()?;
      Ok(format!(
        "UITextFont={} scale={:.2} ModernDocumentFont={}",
        ui.FontFamily()?,
        ui.ScaleFactor()?,
        doc.FontFamily()?
      ))
    });
    r.line(format!(
      "languagefont.{tag}: {}",
      found.unwrap_or_else(|e| format!("error {e}"))
    ));
  }
}

/// Segoe UI Variable: where it is, under which names, its axes, and DirectWrite's automatic optical
/// size against Parley given the same axis values.
pub fn segoe_variable(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let file = Path::new(r"C:\Windows\Fonts\SegUIVar.ttf");
  r.line(format!(
    "segoevar.file: {} {}",
    file.display(),
    std::fs::metadata(file).map_or("absent".to_owned(), |m| format!("{} bytes", m.len()))
  ));

  let mut wws = None;
  unsafe { IDWriteFactory::GetSystemFontCollection(factory, &mut wws, false) }.msg("collection")?;
  let wws = wws.ok_or("no collection")?;
  let names = [
    "Segoe UI Variable",
    "Segoe UI Variable Text",
    "Segoe UI Variable Display",
    "Segoe UI Variable Small",
    "Segoe UI",
  ];

  for name in names {
    let w = wide(name);
    let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
    let _ = unsafe { wws.FindFamilyName(PCWSTR(w.as_ptr()), &mut index, &mut exists) };
    let fontique = direct.fonts.context.collection.family_id(name).map(|id| {
      direct
        .fonts
        .context
        .collection
        .family(id)
        .map(|f| {
          f.fonts()
            .iter()
            .map(|font| {
              format!(
                "w{} axes[{}]",
                font.weight().value(),
                font
                  .axes()
                  .iter()
                  .map(|a| format!("{} {}..{} def {}", a.tag, a.min, a.max, a.default))
                  .collect::<Vec<_>>()
                  .join(", ")
              )
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    });
    r.line(format!(
      "segoevar.names {name}: dwrite_wws={} fontique={fontique:?}",
      exists.as_bool()
    ));
  }

  let f6: IDWriteFactory6 = factory.cast().msg("factory6")?;
  let typographic =
    unsafe { f6.GetSystemFontCollection(false, DWRITE_FONT_FAMILY_MODEL_TYPOGRAPHIC) }
      .msg("typographic collection")?;
  let w = wide("Segoe UI Variable");
  let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
  let _ = unsafe { typographic.FindFamilyName(PCWSTR(w.as_ptr()), &mut index, &mut exists) };
  r.line(format!(
    "segoevar.typographic: 'Segoe UI Variable' found={}",
    exists.as_bool()
  ));

  if !exists.as_bool() {
    return Ok(());
  }

  let fontique_name = ["Segoe UI Variable", "Segoe UI Variable Text"]
    .into_iter()
    .find(|n| direct.fonts.context.collection.family_id(n).is_some());
  let text = "Hamburgefonstiv 0123";
  let utf16: Vec<u16> = text.encode_utf16().collect();
  let locale = wide("en-us");

  for size in [8.0f32, 10.5, 12.0, 14.0, 20.0, 28.0, 40.0] {
    let mut line = format!("segoevar.opsz@{size}:");

    for automatic in [false, true] {
      let format = unsafe {
        f6.CreateTextFormat(
          PCWSTR(w.as_ptr()),
          &typographic,
          &[DWRITE_FONT_AXIS_VALUE {
            axisTag: axis_tag("wght"),
            value: 400.0,
          }],
          size,
          PCWSTR(locale.as_ptr()),
        )
      }
      .msg("format3")?;

      if automatic {
        unsafe { format.SetAutomaticFontAxes(DWRITE_AUTOMATIC_FONT_AXES_OPTICAL_SIZE) }
          .msg("automatic axes")?;
      }

      let layout =
        unsafe { IDWriteFactory::CreateTextLayout(factory, &utf16, &format, 1.0e6, 1.0e6) }
          .msg("layout")?;
      let mut m = DWRITE_TEXT_METRICS::default();
      unsafe { layout.GetMetrics(&mut m) }.msg("metrics")?;
      let runs = collect(&layout, 0.0, 0.0, 1.0, false).msg("collect")?;
      let mut axes = String::new();

      for run in &runs.runs {
        if let Ok(face5) = run.face.cast::<IDWriteFontFace5>() {
          let n = unsafe { face5.GetFontAxisValueCount() };
          let mut values = vec![DWRITE_FONT_AXIS_VALUE::default(); n as usize];
          let _ = unsafe { face5.GetFontAxisValues(&mut values) };
          axes = values
            .iter()
            .map(|v| {
              format!(
                "{}={}",
                String::from_utf8_lossy(&v.axisTag.0.to_le_bytes()),
                v.value
              )
            })
            .collect::<Vec<_>>()
            .join(",");
        }
      }

      let opsz = axes
        .split(',')
        .find_map(|kv| kv.strip_prefix("opsz=").and_then(|v| v.parse::<f32>().ok()));
      line += &format!(
        " {}: dwrite width={:.3} axes[{axes}]",
        if automatic { "auto" } else { "default" },
        m.width
      );

      if let (Some(name), Some(opsz)) = (fontique_name, opsz) {
        let b = direct.lay_out(
          text,
          name,
          size,
          400.0,
          "en-US",
          &[("wght", 400.0), ("opsz", opsz)],
          false,
          None,
        );
        let plain = direct.lay_out(text, name, size, 400.0, "en-US", &[], false, None);
        line += &format!(
          " parley(opsz={opsz}) width={:.3} parley(no variations) width={:.3}",
          b.width(),
          plain.width()
        );
      }
    }

    r.line(line);
  }

  Ok(())
}

/// Line metrics per UI family: DirectWrite's default line box against Parley's.
pub fn metrics(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let families = [
    ("Segoe UI", "Ag", "en-US"),
    ("Segoe UI Variable Text", "Ag", "en-US"),
    ("Yu Gothic UI", "あ漢Ag", "ja-JP"),
    ("Microsoft YaHei UI", "汉字Ag", "zh-CN"),
    ("Malgun Gothic", "한국Ag", "ko-KR"),
    ("Leelawadee UI", "ไทยAg", "th-TH"),
    ("Nirmala UI", "हिन्दीAg", "hi-IN"),
    ("Segoe UI Emoji", "😀", "en-US"),
  ];

  for (family, text, locale) in families {
    for size in [14.0f32, 20.0] {
      let layout = dwrite_layout(
        factory,
        text,
        family,
        size,
        400,
        locale,
        DWRITE_WORD_WRAPPING_NO_WRAP,
        1.0e6,
      )?;
      let lines = line_metrics(&layout).msg("lines")?;
      let runs = collect(&layout, 0.0, 0.0, 1.0, false).msg("collect")?;
      let fm = runs
        .runs
        .first()
        .map(|run| face_metrics(&run.face, run.em))
        .unwrap_or_default();
      let b = direct.lay_out(text, family, size, 400.0, locale, &[], false, None);
      let bl = b.lines().next().map(|l| *l.metrics());
      let fonts = direct.fonts_of(&b, text);

      r.line(format!(
        "metrics.{family}@{size}: dwrite height={:.3} baseline={:.3} font(asc={:.3} desc={:.3} gap={:.3}) | parley height={:.3} baseline={:.3} ascent={:.3} descent={:.3} leading={:.3} fonts={fonts:?}",
        lines.first().map_or(0.0, |l| l.height),
        lines.first().map_or(0.0, |l| l.baseline),
        fm.0,
        fm.1,
        fm.2,
        bl.map_or(0.0, |m| m.block_max_coord - m.block_min_coord),
        bl.map_or(0.0, |m| m.baseline),
        bl.map_or(0.0, |m| m.baseline - m.content_block_min_coord),
        bl.map_or(0.0, |m| m.content_block_max_coord - m.baseline),
        bl.map_or(0.0, |m| (m.block_max_coord - m.block_min_coord)
          - (m.content_block_max_coord - m.content_block_min_coord)),
      ));
    }
  }

  Ok(())
}

/// Shaping parity: per cluster, DirectWrite's glyphs and advances against Parley's.
pub fn shaping(factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let corpus = Corpus::builtin();
  let mut engine = ParleyEngine::new(Fonts::system(), LayoutOptions::default());

  for id in [
    "latin-kerning",
    "latin-ligatures",
    "arabic",
    "devanagari",
    "bengali",
    "tamil",
    "thai",
    "khmer",
    "lao",
    "burmese",
    "han-ja",
  ] {
    let Some(case) = corpus.case(id)
    else {
      continue;
    };
    let offsets = Offsets::new(&case.text);

    for size in [14.0f32, 17.0] {
      let layout = dwrite_layout(
        factory,
        &case.text,
        SYSTEM_FAMILY,
        size,
        400,
        &case.locale,
        DWRITE_WORD_WRAPPING_NO_WRAP,
        1.0e6,
      )?;
      let collected = collect(&layout, 0.0, 0.0, 1.0, false).msg("collect")?;
      // UTF-16 cluster start -> (glyph ids, advance, file).
      let mut a: Vec<(usize, Vec<u32>, f32, String)> = Vec::new();

      for run in collected.runs.iter().filter(|r| !r.inline) {
        let file = face_path(&run.face).unwrap_or_default().to_lowercase();
        let n = run.cluster_map.len();
        let mut j = 0;

        while j < n {
          let g0 = run.cluster_map[j] as usize;
          let mut k = j + 1;

          while k < n && run.cluster_map[k] as usize == g0 {
            k += 1;
          }

          let g1 = if k < n {
            (run.cluster_map[k] as usize).max(g0)
          }
          else {
            run.glyphs.len()
          };
          a.push((
            run.text_position as usize + j,
            run.glyphs[g0..g1].iter().map(|g| u32::from(*g)).collect(),
            run.advances[g0..g1].iter().sum(),
            file.clone(),
          ));
          j = k;
        }
      }

      let variant = Variant {
        case,
        size,
        width: None,
      };
      let laid = engine.lay_out(&variant);
      let mut b: Vec<(usize, Vec<u32>, f32, String)> = Vec::new();

      for line in laid.layout.lines() {
        for item in line.items() {
          if let PositionedLayoutItem::GlyphRun(g) = item {
            let run = g.run();
            let file = engine
              .fonts
              .identity(run.font())
              .path
              .unwrap_or_default()
              .to_lowercase();

            for cluster in run.visual_clusters() {
              let start = offsets.utf16(cluster.text_range().start);
              let ids: Vec<u32> = cluster.glyphs().map(|g| g.id).collect();
              b.push((start, ids, cluster.advance(), file.clone()));
            }
          }
        }
      }

      b.sort_by_key(|c| c.0);
      a.sort_by_key(|c| c.0);
      let (mut matched, mut ids_equal, mut same_font, mut max_d) = (0, 0, 0, 0.0f32);
      let mut first_diff = None;

      for ca in &a {
        if let Some(cb) = b.iter().find(|cb| cb.0 == ca.0) {
          matched += 1;

          if ca.3 == cb.3 {
            same_font += 1;

            if ca.1 == cb.1 {
              ids_equal += 1;
            }
            else if first_diff.is_none() {
              first_diff = Some(format!("@{} a{:?} b{:?}", ca.0, ca.1, cb.1));
            }

            max_d = max_d.max((ca.2 - cb.2).abs());
          }
        }
      }

      let sum_a: f32 = a.iter().map(|c| c.2).sum();
      let sum_b: f32 = b.iter().map(|c| c.2).sum();
      r.line(format!(
        "shaping.{id}@{size}: clusters a={} b={} matched={matched} same_font={same_font} ids_equal={ids_equal} max_cluster_dadv={max_d:.3} width a={sum_a:.3} b={sum_b:.3} drift={:.3} first_diff={first_diff:?}",
        a.len(),
        b.len(),
        sum_b - sum_a
      ));
    }
  }

  Ok(())
}

/// The system ICU's line-break boundaries (`icu.dll`), in UTF-16.
fn icu_breaks(text: &str, locale: &str) -> Res<Vec<usize>> {
  let utf16: Vec<u16> = text.encode_utf16().collect();
  let locale = std::ffi::CString::new(locale.replace('-', "_")).map_err(|e| e.to_string())?;
  let mut status = UErrorCode(0);
  let it = unsafe {
    ubrk_open(
      UBRK_LINE,
      PCSTR(locale.as_ptr() as *const u8),
      utf16.as_ptr(),
      utf16.len() as i32,
      &mut status,
    )
  };

  if it.is_null() || status.0 > 0 {
    return Err(format!("ubrk_open: status {}", status.0));
  }

  let mut out = Vec::new();

  loop {
    let at = unsafe { ubrk_next(it) };

    if at < 0 {
      break;
    }

    out.push(at as usize);
  }

  unsafe { ubrk_close(it) };
  out.retain(|&b| b < utf16.len());
  Ok(out)
}

/// Break opportunities in scripts written without spaces: every line start of a layout at a
/// width no word fits, DirectWrite's against Parley's and the system ICU's.
pub fn breaks(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let corpus = Corpus::builtin();

  for id in [
    "thai",
    "khmer",
    "lao",
    "burmese",
    "ja-kinsoku",
    "zh-hans-paragraph",
    "ko-paragraph",
  ] {
    let Some(case) = corpus.case(id)
    else {
      continue;
    };
    let offsets = Offsets::new(&case.text);
    let layout = dwrite_layout(
      factory,
      &case.text,
      SYSTEM_FAMILY,
      17.0,
      400,
      &case.locale,
      // Whole words only: a line per break opportunity, no emergency break inside a word.
      DWRITE_WORD_WRAPPING_WHOLE_WORD,
      0.5,
    )?;
    let mut dw = Vec::new();
    let mut at = 0usize;

    for line in line_metrics(&layout).msg("lines")? {
      if at > 0 {
        dw.push(at);
      }

      at += line.length as usize;
    }

    let b = direct.lay_out(
      &case.text,
      "system-ui",
      17.0,
      400.0,
      &case.locale,
      &[],
      true,
      Some(0.5),
    );
    let parley: Vec<usize> = b
      .lines()
      .skip(1)
      .map(|l| offsets.utf16(l.text_range().start))
      .collect();
    let icu = icu_breaks(&case.text, &case.locale);
    let agree = |x: &[usize], y: &[usize]| x.iter().filter(|p| y.contains(p)).count();
    let icu_list = icu.clone().unwrap_or_default();

    r.line(format!(
      "breaks.{id}: utf16 len {} | dwrite {} {:?} | parley {} {:?} | icu {} {:?} | dwrite∩parley {} dwrite∩icu {} parley∩icu {}",
      case.text.encode_utf16().count(),
      dw.len(),
      dw,
      parley.len(),
      parley,
      icu_list.len(),
      icu.as_ref().map_or_else(|e| vec![e.clone()], |v| v.iter().map(|b| b.to_string()).collect()),
      agree(&dw, &parley),
      agree(&dw, &icu_list),
      agree(&parley, &icu_list),
    ));
  }

  Ok(())
}

/// fontique's start-up on DirectWrite, from a fresh process: the collection, every family
/// resolved, and the first layouts; with the process's memory around each.
pub fn fontique(r: &mut Report) -> Res<()> {
  let m0 = memory();
  let started = Instant::now();
  let mut fonts = Fonts::system();
  let made = ms(started);
  let m1 = memory();
  let started = Instant::now();
  let names: Vec<String> = fonts
    .context
    .collection
    .family_names()
    .map(str::to_owned)
    .collect();
  let listed = ms(started);
  let started = Instant::now();
  let mut faces = 0;

  for name in &names {
    if let Some(id) = fonts.context.collection.family_id(name)
      && let Some(family) = fonts.context.collection.family(id)
    {
      faces += family.fonts().len();
    }
  }

  let resolved = ms(started);
  let m2 = memory();
  let mut engine = ParleyEngine::new(fonts, LayoutOptions::default());
  let hello = plain_case("hello", "Hello");
  let started = Instant::now();
  let _ = engine.lay_out(&Variant {
    case: &hello,
    size: 17.0,
    width: None,
  });
  let first = ms(started);
  let mixed = plain_case(
    "mixed",
    "English, 日本語, 한국어, العربية, עברית, ไทย, हिन्दी, Ελληνικά, Русский 😀",
  );
  let started = Instant::now();
  let _ = engine.lay_out(&Variant {
    case: &mixed,
    size: 17.0,
    width: None,
  });
  let first_mixed = ms(started);
  let started = Instant::now();
  let _ = engine.lay_out(&Variant {
    case: &mixed,
    size: 17.0,
    width: None,
  });
  let again_mixed = ms(started);
  let m3 = memory();

  r.line(format!(
    "fontique.startup: collection {made:.2} ms ({} families named, listing {listed:.2} ms); every family resolved {resolved:.2} ms ({faces} faces); first layout 'Hello' {first:.2} ms; mixed scripts first {first_mixed:.2} ms, again {again_mixed:.2} ms",
    names.len()
  ));
  r.line(format!(
    "fontique.memory (working set MB, private MB): start {:.1}/{:.1}, after collection {:.1}/{:.1}, after every family {:.1}/{:.1}, after layouts {:.1}/{:.1}",
    m0.0, m0.1, m1.0, m1.1, m2.0, m2.1, m3.0, m3.1
  ));

  Ok(())
}

/// DirectWrite's own start-up from a fresh process, the same steps as [`fontique`].
pub fn dwrite_startup(r: &mut Report) -> Res<()> {
  let m0 = memory();
  let started = Instant::now();
  let factory: IDWriteFactory4 =
    unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.msg("factory")?;
  let mut collection = None;
  unsafe { IDWriteFactory::GetSystemFontCollection(&factory, &mut collection, false) }
    .msg("collection")?;
  let collection = collection.ok_or("no collection")?;
  let made = ms(started);
  let count = unsafe { collection.GetFontFamilyCount() };
  let m1 = memory();
  let started = Instant::now();
  let layout = dwrite_layout(
    &factory,
    "Hello",
    SYSTEM_FAMILY,
    17.0,
    400,
    "en-US",
    DWRITE_WORD_WRAPPING_NO_WRAP,
    1.0e6,
  )?;
  let _ = line_metrics(&layout).msg("lines")?;
  let first = ms(started);
  let mixed = "English, 日本語, 한국어, العربية, עברית, ไทย, हिन्दी, Ελληνικά, Русский 😀";
  let started = Instant::now();
  let layout = dwrite_layout(
    &factory,
    mixed,
    SYSTEM_FAMILY,
    17.0,
    400,
    "en-US",
    DWRITE_WORD_WRAPPING_NO_WRAP,
    1.0e6,
  )?;
  let _ = line_metrics(&layout).msg("lines")?;
  let first_mixed = ms(started);
  let m2 = memory();
  let names = (0..count.min(4))
    .filter_map(|i| unsafe { collection.GetFontFamily(i) }.ok())
    .filter_map(|f| unsafe { f.GetFamilyNames() }.ok())
    .filter_map(|n| localized(&n).ok())
    .collect::<Vec<_>>();

  r.line(format!(
    "dwrite.startup: shared factory + system collection {made:.2} ms ({count} WWS families, e.g. {names:?}); first layout 'Hello' {first:.2} ms; mixed scripts first {first_mixed:.2} ms"
  ));
  r.line(format!(
    "dwrite.memory (working set MB, private MB): start {:.1}/{:.1}, after collection {:.1}/{:.1}, after layouts {:.1}/{:.1}",
    m0.0, m0.1, m1.0, m1.1, m2.0, m2.1
  ));

  Ok(())
}

/// Windows's text scale (Settings > Accessibility > Text size), read and applied to A and B.
pub fn text_scale(factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let factor = UISettings::new()
    .and_then(|s| s.TextScaleFactor())
    .map_err(|e| format!("UISettings: {e}"))?;
  let corpus = Corpus::builtin();
  let case = corpus.case("latin-paragraph").ok_or("no case")?;
  let mut engine = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let native = crate::native::Native::new(factory);

  r.line(format!("textscale.factor: {factor}"));

  for scale in [1.0f32, factor as f32] {
    let variant = Variant {
      case,
      size: 13.0 * scale,
      width: Some(320.0),
    };
    let a = native.lay_out(&variant).msg("a")?;
    let a = native.record(&variant, &a).msg("record")?;
    let b = engine.lay_out(&variant);
    let b = b.record(&variant, &mut engine.fonts, None, false);

    r.line(format!(
      "textscale.applied size {:.2}: a lines={} height={:.2} | b lines={} height={:.2}",
      13.0 * scale,
      a.lines.len(),
      a.height,
      b.lines.len(),
      b.height
    ));
  }

  Ok(())
}

/// A font installed for the current user alone: whether DirectWrite and fontique see it.
pub fn user_font(factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let family = std::env::var("SPIKE_USER_FONT").unwrap_or_else(|_| "Atkinson Hyperlegible".into());
  let name = wide(&family);

  for (label, update) in [("cached", false), ("checked for updates", true)] {
    let isolated: IDWriteFactory =
      unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED) }.msg("factory")?;
    let mut collection = None;
    unsafe { isolated.GetSystemFontCollection(&mut collection, update) }.msg("collection")?;
    let collection = collection.ok_or("no collection")?;
    let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
    let _ = unsafe { collection.FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists) };
    r.line(format!(
      "userfont.dwrite ({label}): {family} found={}",
      exists.as_bool()
    ));
  }

  let set: Res<String> = (|| {
    let f3: IDWriteFactory3 = factory.cast().msg("factory3")?;
    let set = unsafe { f3.GetSystemFontSet() }.msg("font set")?;
    Ok(format!("{} fonts in the system font set", unsafe {
      set.GetFontCount()
    }))
  })();
  r.line(format!("userfont.fontset: {}", set.unwrap_or_else(|e| e)));

  let mut fonts = Fonts::system();
  let id = fonts.context.collection.family_id(&family);
  r.line(format!(
    "userfont.fontique: {family} found={}",
    id.is_some()
  ));

  // The shared factory's collection refreshed once, then fontique made again over it.
  let shared: IDWriteFactory =
    unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.msg("factory")?;
  let mut refreshed = None;
  unsafe { shared.GetSystemFontCollection(&mut refreshed, true) }.msg("collection")?;
  let mut again = Fonts::system();
  r.line(format!(
    "userfont.fontique after a shared GetSystemFontCollection(checkForUpdates): {family} found={}",
    again.context.collection.family_id(&family).is_some()
  ));
  drop(again);

  let layout = dwrite_layout(
    factory,
    "Hamburgefonstiv",
    &family,
    17.0,
    400,
    "en-US",
    DWRITE_WORD_WRAPPING_NO_WRAP,
    1.0e6,
  )?;
  r.line(format!(
    "userfont.a fonts: {:?}",
    dwrite_fonts(&layout, "Hamburgefonstiv")?
  ));
  let mut direct = Direct::new(fonts);
  let b = direct.lay_out(
    "Hamburgefonstiv",
    &family,
    17.0,
    400.0,
    "en-US",
    &[],
    false,
    None,
  );
  r.line(format!(
    "userfont.b fonts: {:?}",
    direct.fonts_of(&b, "Hamburgefonstiv")
  ));

  Ok(())
}

/// Recent emoji: whether Segoe UI Emoji maps each to a glyph, and the glyph DirectWrite's layout
/// draws for it.
pub fn emoji(factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let mut collection = None;
  unsafe { IDWriteFactory::GetSystemFontCollection(factory, &mut collection, false) }
    .msg("collection")?;
  let collection = collection.ok_or("no collection")?;
  let name = wide("Segoe UI Emoji");
  let (mut index, mut exists) = (0u32, windows_core::BOOL(0));
  unsafe { collection.FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists) }
    .msg("find")?;
  let font = unsafe {
    collection.GetFontFamily(index).and_then(|f| {
      f.GetFirstMatchingFont(
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        DWRITE_FONT_STYLE_NORMAL,
      )
    })
  }
  .msg("font")?;
  let face = unsafe { font.CreateFontFace() }.msg("face")?;
  let mut strings = None;
  let mut has = windows_core::BOOL(0);
  let _ = unsafe {
    font.GetInformationalStrings(
      DWRITE_INFORMATIONAL_STRING_VERSION_STRINGS,
      &mut strings,
      &mut has,
    )
  };
  let version = strings.and_then(|s| localized(&s).ok()).unwrap_or_default();
  r.line(format!(
    "emoji.font: Segoe UI Emoji {version:?} {}",
    face_label(&face)
  ));

  for (label, text) in [
    ("u14 melting face", "\u{1FAE0}"),
    ("u15 shaking face", "\u{1FAE8}"),
    ("u15 light blue heart", "\u{1FA75}"),
    ("u15 black bird", "\u{1F426}\u{200D}\u{2B1B}"),
    ("u15.1 head shaking", "\u{1F642}\u{200D}\u{2194}\u{FE0F}"),
    ("u15.1 phoenix", "\u{1F426}\u{200D}\u{1F525}"),
    ("u16 face with bags", "\u{1FAE9}"),
    ("u16 fingerprint", "\u{1FAC6}"),
    ("u16 harp", "\u{1FA89}"),
    ("u16 splatter", "\u{1FADF}"),
    ("u16 flag sark", "\u{1F1E8}\u{1F1F6}"),
  ] {
    let cps: Vec<u32> = text.chars().map(u32::from).collect();
    let mut cmap = vec![0u16; cps.len()];
    let _ = unsafe { face.GetGlyphIndices(cps.as_ptr(), cps.len() as u32, cmap.as_mut_ptr()) };
    let has_char: Vec<bool> = cps
      .iter()
      .map(|cp| {
        unsafe { font.HasCharacter(*cp) }
          .map(|b| b.as_bool())
          .unwrap_or(false)
      })
      .collect();
    let layout = dwrite_layout(
      factory,
      text,
      SYSTEM_FAMILY,
      17.0,
      400,
      "en-US",
      DWRITE_WORD_WRAPPING_NO_WRAP,
      1.0e6,
    )?;
    let runs = collect(&layout, 0.0, 0.0, 1.0, false).msg("collect")?;
    let drawn: Vec<String> = runs
      .runs
      .iter()
      .map(|run| format!("{}:{:?}", face_label(&run.face), run.glyphs))
      .collect();
    r.line(format!(
      "emoji.{label}: cmap {cmap:?} HasCharacter {has_char:?} | layout {drawn:?}"
    ));
  }

  Ok(())
}

/// Parley's strut (the paragraph font's line box, CSS 2 section 10.8) against DirectWrite, which
/// sizes a line by the fonts its runs use: one text laid out by DirectWrite, by Parley as the
/// harness does, and by Parley with a zero line height on the root and the metrics' on a span
/// over the whole text.
pub fn strut(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let resolver = Resolver::new(factory).msg("resolver")?;

  for (label, text, locale) in [
    ("han-zh-hant", "直骨角海写次化刃誤曜述返令", "zh-Hant-TW"),
    ("han-zh-hans", "直骨角海写次化刃誤曜述返令", "zh-Hans-CN"),
    ("han-ja", "直骨角海写次化刃誤曜述返令", "ja-JP"),
    ("latin", "Hamburgefonstiv", "en-US"),
    ("thai", "ภาษาไทยเป็นภาษาที่ไม่มี", "th-TH"),
    ("emoji", "Party 😀🎉", "en-US"),
    (
      "mixed",
      "English, 日本語, 한국어, العربية, ไทย, हिन्दी",
      "en-US",
    ),
  ] {
    let layout = dwrite_layout(
      factory,
      text,
      SYSTEM_FAMILY,
      17.0,
      400,
      locale,
      DWRITE_WORD_WRAPPING_NO_WRAP,
      1.0e6,
    )?;
    let lines = line_metrics(&layout).msg("lines")?;
    let with = direct.lay_out(text, "system-ui", 17.0, 400.0, locale, &[], false, None);
    let without = direct.strutless(text, 17.0, locale);
    let box_of = |l: &Layout<()>| {
      l.lines().next().map_or((0.0, 0.0), |line| {
        let m = line.metrics();
        (
          m.block_max_coord - m.block_min_coord,
          m.baseline - m.block_min_coord,
        )
      })
    };
    let (wh, wb) = box_of(&with);
    let (nh, nb) = box_of(&without);
    let offsets = Offsets::new(text);
    let ranges: Vec<(std::ops::Range<usize>, Option<String>)> = resolver
      .map(text, locale, Some(SYSTEM_FAMILY), 400, false)
      .msg("map")?
      .into_iter()
      .map(|m| {
        let [start, end] = offsets.range_utf8([m.start as usize, (m.start + m.len) as usize]);
        (start..end, m.family)
      })
      .collect();
    let spans = direct.per_run_spans(text, 17.0, locale, &ranges);
    let (sh, sb) = box_of(&spans);
    r.line(format!(
      "strut.{label}: dwrite height={:.3} baseline={:.3} | parley (strut) height={wh:.3} baseline={wb:.3} | parley (root line height 0) height={nh:.3} baseline={nb:.3} | parley (root line height 0, a span per fallback run naming its font) height={sh:.3} baseline={sb:.3}",
      lines.first().map_or(0.0, |l| l.height),
      lines.first().map_or(0.0, |l| l.baseline),
    ));
  }

  Ok(())
}

/// Segoe UI Variable at DirectWrite's automatic optical size and at Parley's given the same axis
/// values, both drawn by Direct2D: whether the outlines are the same instance.
pub fn opsz_pixels(factory: &IDWriteFactory4, direct: &mut Direct, r: &mut Report) -> Res<()> {
  let file = r"C:\Windows\Fonts\SegUIVar.ttf";

  if std::fs::metadata(file).is_err() {
    r.line("opszpx: Segoe UI Variable absent".to_owned());
    return Ok(());
  }

  let Some(name) = ["Segoe UI Variable Text", "Segoe UI Variable"]
    .into_iter()
    .find(|n| direct.fonts.context.collection.family_id(n).is_some())
  else {
    r.line("opszpx: fontique has no Segoe UI Variable".to_owned());
    return Ok(());
  };
  let f6: IDWriteFactory6 = factory.cast().msg("factory6")?;
  let typographic =
    unsafe { f6.GetSystemFontCollection(false, DWRITE_FONT_FAMILY_MODEL_TYPOGRAPHIC) }
      .msg("typographic")?;
  let family = wide("Segoe UI Variable");
  let locale = wide("en-us");
  let text = "Hamburgefonstiv 0123";
  let utf16: Vec<u16> = text.encode_utf16().collect();
  let reference = crate::paint::Reference::new().msg("reference")?;
  let mut faces = crate::faces::Faces::new(factory).msg("faces")?;

  for size in [8.0f32, 12.0, 20.0, 40.0] {
    let opsz = size * 0.75;
    let format = unsafe {
      f6.CreateTextFormat(
        PCWSTR(family.as_ptr()),
        &typographic,
        &[DWRITE_FONT_AXIS_VALUE {
          axisTag: axis_tag("wght"),
          value: 400.0,
        }],
        size,
        PCWSTR(locale.as_ptr()),
      )
    }
    .msg("format")?;
    unsafe { format.SetAutomaticFontAxes(DWRITE_AUTOMATIC_FONT_AXES_OPTICAL_SIZE) }
      .msg("automatic")?;
    let layout =
      unsafe { IDWriteFactory::CreateTextLayout(factory, &utf16, &format, 1.0e6, 1.0e6) }
        .msg("layout")?;
    let mut m = DWRITE_TEXT_METRICS::default();
    unsafe { layout.GetMetrics(&mut m) }.msg("metrics")?;
    let b = direct.lay_out(
      text,
      name,
      size,
      400.0,
      "en-US",
      &[("wght", 400.0), ("opsz", opsz)],
      false,
      None,
    );
    let Some(face) = faces.face(
      file,
      0,
      DWRITE_FONT_SIMULATIONS_NONE,
      &[("wght".to_owned(), 400.0), ("opsz".to_owned(), opsz)],
    )
    else {
      r.line(format!("opszpx@{size}: no face"));
      continue;
    };
    let mut glyphs = Vec::new();
    let mut offsets = Vec::new();
    let mut baseline = 0.0;

    for line in b.lines() {
      for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(g) = item {
          baseline = g.baseline();
          let mut x = g.offset();

          for cluster in g.run().visual_clusters() {
            for glyph in cluster.glyphs() {
              glyphs.push(glyph.id as u16);
              offsets.push(DWRITE_GLYPH_OFFSET {
                advanceOffset: x + glyph.x,
                ascenderOffset: glyph.y,
              });
              x += glyph.advance;
            }
          }
        }
      }
    }

    let runs = crate::engine::Collected {
      runs: vec![crate::engine::GlyphRunRec {
        face,
        em: size,
        advances: vec![0.0; glyphs.len()],
        glyphs,
        offsets,
        origin: (0.0, baseline),
        bidi: 0,
        sideways: false,
        inline: false,
        text_position: 0,
        cluster_map: Vec::new(),
      }],
      lines: Vec::new(),
    };
    let width = m.width.max(b.width()).ceil() + 16.0;
    let height = m.height.ceil() + 16.0;
    let to_rgba = |img: &crate::image::Img| text_parity::image::Rgba {
      width: img.w as u32,
      height: img.h as u32,
      data: img.px.iter().flatten().copied().collect(),
    };

    for scale in [1.0f32, 2.0] {
      let (w, h) = (
        (width * scale).round() as u32,
        (height * scale).round() as u32,
      );
      let a = reference
        .render_at(
          &[crate::paint::Item::Layout {
            layout: layout.clone(),
            x: 8.0,
            y: 8.0,
            color: [0.0, 0.0, 0.0, 1.0],
          }],
          w,
          h,
          scale,
        )
        .msg("a")?;
      let c = reference
        .render_at(
          &[crate::paint::Item::Glyphs {
            runs: std::rc::Rc::new(runs.scaled(1.0)),
            x: 8.0,
            y: 8.0,
            color: [0.0, 0.0, 0.0, 1.0],
          }],
          w,
          h,
          scale,
        )
        .msg("c")?;
      let metrics = text_parity::image::compare(&to_rgba(&a), &to_rgba(&c), 64);
      r.line(format!(
        "opszpx@{size} x{scale}: dwrite auto opsz vs parley opsz={opsz} drawn by Direct2D: width {:.3} vs {:.3}; mad={:.3} max={} ssim={:.4} iou={:.4}",
        m.width,
        b.width(),
        metrics.mean_abs_diff,
        metrics.max_diff,
        metrics.ssim,
        metrics.ink_iou
      ));
    }
  }

  Ok(())
}

fn noted(r: &mut Report, name: &str, result: Res<()>) {
  if let Err(e) = result {
    r.line(format!("error: probe {name}: {e}"));
  }
}

/// Every probe of the `probes` part.
pub fn run(_out: &Path, factory: &IDWriteFactory4, r: &mut Report) -> Res<()> {
  let mut direct = Direct::new(Fonts::system());
  let result = fallback(factory, &mut direct, r);
  noted(r, "fallback", result);
  language_fonts(r);
  let result = segoe_variable(factory, &mut direct, r);
  noted(r, "segoevar", result);
  let result = metrics(factory, &mut direct, r);
  noted(r, "metrics", result);
  let result = shaping(factory, r);
  noted(r, "shaping", result);
  let result = breaks(factory, &mut direct, r);
  noted(r, "breaks", result);
  let result = text_scale(factory, r);
  noted(r, "textscale", result);
  let result = emoji(factory, r);
  noted(r, "emoji", result);
  let result = strut(factory, &mut direct, r);
  noted(r, "strut", result);
  let result = opsz_pixels(factory, &mut direct, r);
  noted(r, "opszpx", result);
  Ok(())
}
