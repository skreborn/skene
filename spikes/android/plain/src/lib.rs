//! Parley as an app ships it: the line starts of a text at a width, through fontique's own
//! Android collection, with or without the complex-script dictionaries (the `complex` feature).
//! Built without features it holds the JNI entry points alone, the baseline its size is weighed
//! against.

use jni::{
  objects::{JClass, JString},
  sys::{jboolean, jfloat, jintArray},
  JNIEnv,
};

/// Whether the complex-script dictionaries are compiled in.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_PlainParley_complex<'local>(
  _env: JNIEnv<'local>,
  _class: JClass<'local>,
) -> jboolean {
  u8::from(cfg!(feature = "complex"))
}

/// The UTF-16 start of every line of `text` in `locale` at `size`, broken at `width`; null where
/// Parley is not compiled in.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_PlainParley_lines<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  text: JString<'local>,
  locale: JString<'local>,
  size: jfloat,
  width: jfloat,
) -> jintArray {
  let text: String = env.get_string(&text).map(Into::into).unwrap_or_default();
  let locale: String = env.get_string(&locale).map(Into::into).unwrap_or_default();
  let Some(starts) = lines::starts(&text, &locale, size, width)
  else {
    return std::ptr::null_mut();
  };
  let Ok(array) = env.new_int_array(starts.len() as i32)
  else {
    return std::ptr::null_mut();
  };

  if env.set_int_array_region(&array, 0, &starts).is_err() {
    return std::ptr::null_mut();
  }

  array.into_raw()
}

#[cfg(feature = "parley")]
mod lines {
  use std::{borrow::Cow, sync::Mutex};

  use parley::{
    fontique::Language,
    FontContext,
    FontFamily,
    FontFamilyName,
    GenericFamily,
    LayoutContext,
    LineHeight,
    OverflowWrap,
    StyleProperty,
  };

  static CONTEXTS: Mutex<Option<(FontContext, LayoutContext<()>)>> = Mutex::new(None);

  /// The UTF-16 start of each line.
  pub fn starts(text: &str, locale: &str, size: f32, width: f32) -> Option<Vec<i32>> {
    let mut contexts = CONTEXTS.lock().ok()?;
    let (fonts, layouts) =
      contexts.get_or_insert_with(|| (FontContext::new(), LayoutContext::new()));
    let families = [
      FontFamilyName::Generic(GenericFamily::SystemUi),
      FontFamilyName::Generic(GenericFamily::SansSerif),
    ];
    let mut builder = layouts.ranged_builder(fonts, text, 1., false);

    builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(
      &families,
    ))));
    builder.push_default(StyleProperty::FontSize(size));
    builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(1.)));
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));
    builder.push_default(StyleProperty::Locale(Language::parse(locale).ok()));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(width));

    Some(
      layout
        .lines()
        .map(|line| text[..line.text_range().start].encode_utf16().count() as i32)
        .collect(),
    )
  }
}

#[cfg(not(feature = "parley"))]
mod lines {
  /// Nothing: Parley is not compiled in.
  pub fn starts(_text: &str, _locale: &str, _size: f32, _width: f32) -> Option<Vec<i32>> {
    None
  }
}
