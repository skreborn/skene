//! The per-run fallback resolver B and C would ship: DirectWrite's system font fallback asked,
//! as `IDWriteTextLayout` asks it, with the run's own text, the requested family as the base,
//! its weight and style, and the text's locale (fontique asks once per script with a sample
//! string, no base family and regular weight).

use windows::Win32::Graphics::DirectWrite::*;
use windows_core::{Interface, OutRef, PCWSTR, Result, implement};

use crate::{
  engine::wide,
  native::{family_name, localized},
  paint::face_path,
};

/// One range the fallback mapped to a font.
#[derive(Clone, Debug)]
pub struct Mapped {
  // UTF-16 start and length.
  pub start: u32,
  pub len: u32,
  // The font's family (WWS, as the system collection names it), `None` where nothing maps.
  pub family: Option<String>,
  pub path: Option<String>,
  pub index: u32,
  pub weight: i32,
  pub simulations: i32,
  pub scale: f32,
}

#[implement(IDWriteTextAnalysisSource)]
struct Source {
  text: Vec<u16>,
  locale: Vec<u16>,
}

impl IDWriteTextAnalysisSource_Impl for Source_Impl {
  fn GetLocaleName(&self, position: u32, length: *mut u32, locale: *mut *mut u16) -> Result<()> {
    unsafe {
      *length = (self.text.len() as u32).saturating_sub(position);
      *locale = self.locale.as_ptr() as *mut u16;
    }

    Ok(())
  }

  fn GetNumberSubstitution(
    &self,
    position: u32,
    length: *mut u32,
    substitution: OutRef<'_, IDWriteNumberSubstitution>,
  ) -> Result<()> {
    substitution.write(None)?;
    unsafe { *length = (self.text.len() as u32).saturating_sub(position) };
    Ok(())
  }

  fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
  }

  fn GetTextAtPosition(&self, position: u32, text: *mut *mut u16, length: *mut u32) -> Result<()> {
    unsafe {
      let rest = self.text.get(position as usize..).unwrap_or_default();
      *length = rest.len() as u32;
      *text = rest.as_ptr() as *mut u16;
    }

    Ok(())
  }

  fn GetTextBeforePosition(
    &self,
    position: u32,
    text: *mut *mut u16,
    length: *mut u32,
  ) -> Result<()> {
    unsafe {
      let before = self.text.get(..position as usize).unwrap_or_default();
      *length = before.len() as u32;
      *text = before.as_ptr() as *mut u16;
    }

    Ok(())
  }
}

/// The system fallback over the system collection.
pub struct Resolver {
  fallback: IDWriteFontFallback,
  collection: IDWriteFontCollection,
}

impl Resolver {
  pub fn new(factory: &IDWriteFactory4) -> Result<Self> {
    let fallback = unsafe { factory.cast::<IDWriteFactory2>()?.GetSystemFontFallback()? };
    let mut collection = None;
    unsafe { IDWriteFactory::GetSystemFontCollection(factory, &mut collection, false)? };

    Ok(Self {
      fallback,
      collection: collection.ok_or_else(windows_core::Error::empty)?,
    })
  }

  /// Every range of `text` mapped to the font the fallback picks, with `base` as the base family
  /// (none where `None`).
  pub fn map(
    &self,
    text: &str,
    locale: &str,
    base: Option<&str>,
    weight: i32,
    italic: bool,
  ) -> Result<Vec<Mapped>> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let len = utf16.len() as u32;
    let source: IDWriteTextAnalysisSource = Source {
      text: utf16,
      locale: wide(locale),
    }
    .into();
    let base = base.map(wide);
    let mut out = Vec::new();
    let mut at = 0u32;

    while at < len {
      let mut mapped_len = 0u32;
      let mut font: Option<IDWriteFont> = None;
      let mut scale = 1.0f32;

      unsafe {
        self.fallback.MapCharacters(
          &source,
          at,
          len - at,
          &self.collection,
          base.as_ref().map_or(PCWSTR::null(), |b| PCWSTR(b.as_ptr())),
          DWRITE_FONT_WEIGHT(weight),
          if italic {
            DWRITE_FONT_STYLE_ITALIC
          }
          else {
            DWRITE_FONT_STYLE_NORMAL
          },
          DWRITE_FONT_STRETCH_NORMAL,
          &mut mapped_len,
          &mut font,
          &mut scale,
        )?;
      }

      let mapped_len = mapped_len.max(1);
      let mut mapped = Mapped {
        start: at,
        len: mapped_len,
        family: None,
        path: None,
        index: 0,
        weight: 0,
        simulations: 0,
        scale,
      };

      if let Some(font) = font {
        let face = unsafe { font.CreateFontFace()? };
        mapped.family = unsafe { font.GetFontFamily()?.GetFamilyNames() }
          .ok()
          .and_then(|names| localized(&names).ok())
          .or_else(|| {
            face
              .cast::<IDWriteFontFace3>()
              .ok()
              .and_then(|f| family_name(&f).ok())
          });
        mapped.path = face_path(&face).ok();
        mapped.index = unsafe { face.GetIndex() };
        mapped.weight = unsafe { font.GetWeight() }.0;
        mapped.simulations = unsafe { font.GetSimulations() }.0;
      }

      out.push(mapped);
      at += mapped_len;
    }

    Ok(out)
  }
}
