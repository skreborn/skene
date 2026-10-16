//! What a process under the App Sandbox can read of the fonts Parley needs: the system font's
//! file, the Chinese UI font's file, and a font installed in `~/Library/Fonts`, opened
//! directly and through `CTFontCopyTable`.
//!
//! `sandbox-probe names FILE` prints a font file's PostScript name (run unsandboxed);
//! `sandbox-probe check POSTSCRIPT FILE` runs the probe (signed with the sandbox entitlement).

use apple_parity::{
  coretext,
  platform::{as_ct, system_font},
};
use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::{CTFont, CTFontTableOptions};

fn read(path: &str) -> String {
  match std::fs::read(path) {
    Ok(bytes) => format!("read {} bytes", bytes.len()),
    Err(error) => format!("read failed: {error}"),
  }
}

fn tables(font: &CTFont) -> String {
  let tags = coretext::tables(font);
  let head = unsafe { font.table(coretext::tag_of("head"), CTFontTableOptions::NoOptions) }
    .map_or(0, |data| data.to_vec().len());
  let sfnt = coretext::sfnt(font).map_or(0, |bytes| bytes.len());

  format!(
    "{} tables, head {head} bytes, whole sfnt {sfnt} bytes",
    tags.len()
  )
}

fn main() {
  let arguments: Vec<String> = std::env::args().collect();

  match arguments.get(1).map(String::as_str) {
    Some("names") => {
      let path = arguments.get(2).cloned().unwrap_or_default();
      let names = std::fs::read(&path)
        .ok()
        .and_then(|data| text_parity::fonts::names_of(&data, 0));

      println!(
        "{}",
        names.map(|(_, postscript)| postscript).unwrap_or_default()
      );
    }
    Some("check") => {
      println!(
        "APP_SANDBOX_CONTAINER_ID={:?}",
        std::env::var("APP_SANDBOX_CONTAINER_ID").ok()
      );
      println!("HOME={:?}", std::env::var("HOME").ok());

      let system = system_font(17., 400, false);
      let system = as_ct(&system);
      let info = coretext::info(system);

      println!(
        "system font {} at {:?}: {}; {}",
        info.postscript,
        info.path,
        info.path.as_deref().map(read).unwrap_or_default(),
        tables(system)
      );

      let string = CFString::from_str("中文");
      let chinese = unsafe {
        CTFont::for_string_with_language(
          system,
          &string,
          CFRange {
            location: 0,
            length: 2,
          },
          Some(&CFString::from_str("zh-Hans")),
        )
      };
      let info = coretext::info(&chinese);

      println!(
        "Chinese font {} at {:?}: {}; {}",
        info.postscript,
        info.path,
        info.path.as_deref().map(read).unwrap_or_default(),
        tables(&chinese)
      );

      if let (Some(postscript), Some(path)) = (arguments.get(2), arguments.get(3)) {
        let font =
          unsafe { CTFont::with_name(&CFString::from_str(postscript), 17., std::ptr::null()) };
        let info = coretext::info(&font);

        println!(
          "user font {postscript} (installed at {path}): file {}; Core Text resolved {} at {:?}; {}",
          read(path),
          info.postscript,
          info.path,
          tables(&font)
        );
      }

      let fontique = std::time::Instant::now();
      let fonts = text_parity::fonts::Fonts::system();
      let mut collection = fonts.context.collection;
      let families = collection.family_names().count();

      println!(
        "fontique system collection: {families} families in {:.1} ms",
        fontique.elapsed().as_secs_f64() * 1e3
      );
    }
    _ => {
      eprintln!("usage: sandbox-probe names FILE | check [POSTSCRIPT FILE]");
      std::process::exit(2);
    }
  }
}
