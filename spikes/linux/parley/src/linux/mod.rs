//! Linux's own layers under Parley: fontconfig's per-character fallback and rendering
//! settings, FreeType's rasterization under them, and the desktop's settings beneath both.

mod fallback;
pub mod fontconfig;
pub mod freetype;
pub mod xsettings;

pub use self::{
  fallback::{Fallback, Presentation},
  fontconfig::{DesktopSettings, Fontconfig, RenderSettings, Request},
  freetype::{GlyphMask, MaskPixels, Rasterizer},
};
