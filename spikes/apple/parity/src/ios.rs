//! iOS only: Dynamic Type (each text style's preferred size against `UIFontMetrics` scaling a
//! base size, and A against B at every category), Bold Text, and the language-dependent line
//! height of iOS 17.
#![allow(unused_unsafe)]

use objc2::rc::Retained;
use objc2_foundation::NSArray;
use objc2_ui_kit::{
  UIContentSizeCategory,
  UIContentSizeCategoryAccessibilityExtraExtraExtraLarge,
  UIContentSizeCategoryAccessibilityExtraExtraLarge,
  UIContentSizeCategoryAccessibilityExtraLarge,
  UIContentSizeCategoryAccessibilityLarge,
  UIContentSizeCategoryAccessibilityMedium,
  UIContentSizeCategoryExtraExtraExtraLarge,
  UIContentSizeCategoryExtraExtraLarge,
  UIContentSizeCategoryExtraLarge,
  UIContentSizeCategoryExtraSmall,
  UIContentSizeCategoryLarge,
  UIContentSizeCategoryMedium,
  UIContentSizeCategorySmall,
  UIFont,
  UIFontMetrics,
  UILegibilityWeight,
  UITraitCollection,
};
use serde_json::json;
use text_parity::{
  bench::plain_case,
  corpus::{Corpus, Variant},
  fonts::Fonts,
  layout::{Options as LayoutOptions, ParleyEngine},
};

use crate::{
  coretext,
  platform::{STYLE_NAMES, as_ct, ui_styles},
  probes::Report,
  raster::Raster,
  sets::Context,
  subset,
  textkit::{self, Setup},
  tuned::TunedEngine,
};

/// The content size categories, smallest first.
#[must_use]
pub fn categories() -> Vec<(&'static str, &'static UIContentSizeCategory)> {
  unsafe {
    vec![
      ("xs", UIContentSizeCategoryExtraSmall),
      ("s", UIContentSizeCategorySmall),
      ("m", UIContentSizeCategoryMedium),
      ("l", UIContentSizeCategoryLarge),
      ("xl", UIContentSizeCategoryExtraLarge),
      ("xxl", UIContentSizeCategoryExtraExtraLarge),
      ("xxxl", UIContentSizeCategoryExtraExtraExtraLarge),
      ("ax1", UIContentSizeCategoryAccessibilityMedium),
      ("ax2", UIContentSizeCategoryAccessibilityLarge),
      ("ax3", UIContentSizeCategoryAccessibilityExtraLarge),
      ("ax4", UIContentSizeCategoryAccessibilityExtraExtraLarge),
      (
        "ax5",
        UIContentSizeCategoryAccessibilityExtraExtraExtraLarge,
      ),
    ]
  }
}

fn traits(category: &UIContentSizeCategory, bold: bool) -> Retained<UITraitCollection> {
  let size = UITraitCollection::traitCollectionWithPreferredContentSizeCategory(category);

  if !bold {
    return size;
  }

  let weight = UITraitCollection::traitCollectionWithLegibilityWeight(UILegibilityWeight::Bold);

  #[allow(deprecated)]
  UITraitCollection::traitCollectionWithTraitsFromCollections(&NSArray::from_retained_slice(&[
    size, weight,
  ]))
}

fn preferred(
  style: &objc2_ui_kit::UIFontTextStyle,
  category: &UIContentSizeCategory,
  bold: bool,
) -> Retained<UIFont> {
  UIFont::preferredFontForTextStyle_compatibleWithTraitCollection(
    style,
    Some(&traits(category, bold)),
  )
}

const SUBSET: [(&str, Option<f32>); 7] = [
  ("latin-paragraph", Some(320.)),
  ("han-zh-hans", Some(120.)),
  ("ko-paragraph", Some(200.)),
  ("thai", Some(160.)),
  ("arabic", Some(120.)),
  ("emoji-zwj", None),
  ("mixed-scripts", Some(160.)),
];

/// Dynamic Type: the size table, then A against B at every category for `body`.
pub fn dynamic_type(
  report: &mut Report,
  context: &Context,
  tuned: &mut TunedEngine,
  raster: &mut Raster,
) {
  report.line("\n[Dynamic Type: preferredFont against UIFontMetrics scaling the Large size]");

  let large = unsafe { UIContentSizeCategoryLarge };
  let mut table = Vec::new();
  let mut mismatches = 0usize;

  for (name, style) in STYLE_NAMES.iter().zip(ui_styles()) {
    let base = preferred(style, large, false);
    let base_size = unsafe { base.pointSize() };
    let metrics = UIFontMetrics::metricsForTextStyle(style);
    let mut cells = Vec::new();

    for (category, value) in categories() {
      let font = preferred(style, value, false);
      let size = unsafe { font.pointSize() };
      let scaled = metrics
        .scaledValueForValue_compatibleWithTraitCollection(base_size, Some(&traits(value, false)));
      let scaled_font =
        metrics.scaledFontForFont_compatibleWithTraitCollection(&base, Some(&traits(value, false)));
      let scaled_font_size = unsafe { scaled_font.pointSize() };
      let info = coretext::info(as_ct(&font));

      if (size - scaled).abs() > 0.01 || (size - scaled_font_size).abs() > 0.01 {
        mismatches += 1;
      }

      cells.push(format!(
        "{category} {size:.1}/{scaled:.2}/{scaled_font_size:.1}"
      ));
      table.push(json!({
        "style": name, "category": category, "preferred": size, "scaled_value": scaled, "scaled_font": scaled_font_size,
        "postscript": info.postscript, "leading": unsafe { font.leading() }, "line_height": unsafe { font.lineHeight() },
        "variation": info.variation,
      }));
    }

    report.line(format!("  {name}: {}", cells.join(", ")));
  }

  report.line(format!("  (preferred / scaledValue(for: Large size) / scaledFont(for: Large font)) mismatches over 0.01pt: {mismatches}"));

  let mut stock = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let mut per_category = Vec::new();
  let body = ui_styles()[6];

  report.line("  A (TextKit, preferredFont(.body) at the category) against B stock, B tuned, C tuned at the same size:");

  for (category, value) in categories() {
    let font = preferred(body, value, false);
    let size = unsafe { font.pointSize() } as f32;
    let corpus = Corpus {
      version: 1,
      cases: subset::cases(&SUBSET, category, &[size]),
    };
    let dir = context.out.join("dynamic-type").join(category);
    let setup = |_: &Variant<'_>| Setup {
      font: Some(font.clone()),
      ..Setup::default()
    };
    let sets = subset::run(
      context,
      &corpus,
      &dir,
      &setup,
      &|case| case.weight,
      tuned,
      raster,
      &mut stock,
    );
    let summary = subset::summarize(
      report,
      &format!("{category} ({size}pt)"),
      &corpus,
      &dir,
      &sets,
    );

    per_category.push(json!({"category": category, "size": size, "summary": summary}));
  }

  report.put(
    "dynamic_type",
    json!({"table": table, "mismatches": mismatches, "categories": per_category}),
  );
}

/// Bold Text: what the preferred fonts and the fallback become, and A against tuned B with
/// the bold weight.
pub fn bold_text(
  report: &mut Report,
  context: &Context,
  tuned: &mut TunedEngine,
  raster: &mut Raster,
) {
  report.line("\n[Bold Text: legibilityWeight bold]");

  let large = unsafe { UIContentSizeCategoryLarge };
  let mut rows = Vec::new();

  for (name, style) in STYLE_NAMES.iter().zip(ui_styles()) {
    let regular = coretext::info(as_ct(&preferred(style, large, false)));
    let bold = coretext::info(as_ct(&preferred(style, large, true)));

    report.line(format!(
      "  {name}: {} {:?} -> {} {:?}",
      regular.postscript, regular.variation, bold.postscript, bold.variation
    ));
    rows.push(json!({"style": name, "regular": regular, "bold": bold}));
  }

  let body = ui_styles()[6];
  let bold_body = preferred(body, large, true);
  let string = objc2_core_foundation::CFString::from_str("中文");
  let fallback = unsafe {
    objc2_core_text::CTFont::for_string_with_language(
      as_ct(&bold_body),
      &string,
      objc2_core_foundation::CFRange {
        location: 0,
        length: 2,
      },
      Some(&objc2_core_foundation::CFString::from_str("zh-Hans")),
    )
  };
  let fallback_info = coretext::info(&fallback);
  let regular_fallback = unsafe {
    objc2_core_text::CTFont::for_string_with_language(
      as_ct(&preferred(body, large, false)),
      &string,
      objc2_core_foundation::CFRange {
        location: 0,
        length: 2,
      },
      Some(&objc2_core_foundation::CFString::from_str("zh-Hans")),
    )
  };

  report.line(format!(
    "  CJK fallback of body: regular -> {} ; bold -> {} (weight trait {:.2})",
    coretext::info(&regular_fallback).postscript,
    fallback_info.postscript,
    fallback_info.weight_trait
  ));

  let bold_weight = coretext::css_weight(&coretext::info(as_ct(&bold_body))).round() as u16;
  let size = unsafe { bold_body.pointSize() } as f32;
  let corpus = Corpus {
    version: 1,
    cases: subset::cases(&SUBSET, "bold", &[size]),
  };
  let dir = context.out.join("bold-text");
  let mut stock = ParleyEngine::new(Fonts::system(), LayoutOptions::default());
  let setup = |_: &Variant<'_>| Setup {
    font: Some(bold_body.clone()),
    ..Setup::default()
  };
  let sets = subset::run(
    context,
    &corpus,
    &dir,
    &setup,
    &|_| bold_weight,
    tuned,
    raster,
    &mut stock,
  );

  report.line(format!("  A (bold preferred body {size}pt) against B stock (weight 400), tuned B and C at weight {bold_weight}:"));

  let summary = subset::summarize(report, "bold", &corpus, &dir, &sets);

  report.put("bold_text", json!({"styles": rows, "fallback_bold": fallback_info, "weight": bold_weight, "summary": summary}));
}

/// The preferred languages and what the body text style's line comes to under them.
pub fn language(report: &mut Report) {
  let languages = objc2_foundation::NSLocale::preferredLanguages();
  let languages: Vec<String> = languages
    .iter()
    .map(|language| language.to_string())
    .collect();
  let large = unsafe { UIContentSizeCategoryLarge };
  let body = ui_styles()[6];
  let font = preferred(body, large, false);
  let ct = as_ct(&font);
  let mut lines = Vec::new();

  for (label, text) in [
    ("latin", "Handgloves"),
    ("thai", "ภาษาไทยที่มีวรรณยุกต์ซ้อนกัน"),
    ("arabic", "مرحبا بالعالم"),
    ("hindi", "हिन्दी भारत की राजभाषा"),
  ] {
    let case = plain_case(label, text);
    let variant = Variant {
      case: &case,
      size: unsafe { font.pointSize() } as f32,
      width: None,
    };
    let styled = textkit::variant(
      &variant,
      &Setup {
        font: Some(font.clone()),
        ..Setup::default()
      },
      0,
    );
    let plain = textkit::variant(&variant, &Setup::default(), 0);
    let line = styled.record.lines.first().cloned().unwrap_or_default();
    let plain_line = plain.record.lines.first().cloned().unwrap_or_default();

    lines.push(json!({
      "text": label, "body_line_height": line.height, "body_baseline": line.baseline,
      "body_usage": textkit::usage(&styled.laid), "system_line_height": plain_line.height,
      "label": crate::ios::label_size(&styled, &font, text),
    }));
  }

  let summary = json!({
    "languages": languages,
    "body": {
      "postscript": coretext::info(ct).postscript, "size": unsafe { font.pointSize() }, "line_height": unsafe { font.lineHeight() },
      "leading": unsafe { font.leading() }, "ascender": unsafe { font.ascender() }, "descender": unsafe { font.descender() },
      "ct": [unsafe { ct.ascent() }, unsafe { ct.descent() }, unsafe { ct.leading() }],
    },
    "lines": lines,
  });

  report.line(format!("[language line height] {summary}"));
  report.put("language", summary);
}

/// UILabel's `sizeThatFits` for one line of `text` in `font`, where a main thread is at hand.
#[must_use]
pub fn label_size(_: &textkit::TkVariant, font: &UIFont, text: &str) -> Option<(f64, f64)> {
  let mtm = objc2::MainThreadMarker::new()?;
  let label = objc2_ui_kit::UILabel::new(mtm);

  unsafe { label.setFont(Some(font)) };
  label.setText(Some(&objc2_foundation::NSString::from_str(text)));

  let size = label.sizeThatFits(objc2_core_foundation::CGSize::new(1.0e6, 1.0e6));

  Some((size.width, size.height))
}
