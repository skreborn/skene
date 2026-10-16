//! The suite from a command line: `parity-probe suite '{"out": "DIR"}'` runs everything,
//! `parity-probe language` prints the language line-height probe (iOS, under
//! `xcrun simctl spawn ... -AppleLanguages "(th)"`), `parity-probe fontmem STORAGE [OUT.json]`
//! measures the tuned engine's start-up and memory with its fonts stored as `copy`, `layout`,
//! `map` or `map-all`, and `parity-probe fonttables` lists the first labels' fonts' tables.

fn main() {
  let main = std::time::Instant::now();
  let arguments: Vec<String> = std::env::args().collect();

  match arguments.get(1).map(String::as_str) {
    Some("suite") => {
      let config = arguments.get(2).cloned().unwrap_or_else(|| "{}".to_owned());

      match apple_parity::suite::run_json(&config) {
        Ok(text) => println!("=== RESULTS ===\n{text}=== END ==="),
        Err(error) => {
          eprintln!("error: {error}");
          std::process::exit(1);
        }
      }
    }
    Some("language") => {
      let text = apple_parity::ffi::apple_parity_language();
      let json = unsafe { std::ffi::CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();

      unsafe { apple_parity::ffi::apple_parity_free(text) };
      println!("LANGUAGE {json}");
    }
    Some("fontmem") => {
      let Some(storage) = arguments
        .get(2)
        .and_then(|name| apple_parity::tuned::Storage::parse(name))
      else {
        eprintln!("usage: parity-probe fontmem copy|layout|map|map-all [OUT.json]");
        std::process::exit(2);
      };
      let json = apple_parity::fontmem::run(storage, main);
      let text = serde_json::to_string(&json).unwrap_or_default();

      match arguments.get(3) {
        Some(path) => {
          if let Err(error) = std::fs::write(path, &text) {
            eprintln!("{path}: {error}");
            std::process::exit(1);
          }
        }
        None => println!("{text}"),
      }

      eprintln!("FONTMEM {}", apple_parity::fontmem::summary(&json));
    }
    Some("fonttables") => println!("{}", apple_parity::fontmem::tables()),
    _ => {
      eprintln!(
        "usage: parity-probe suite [CONFIG.json] | language | fontmem STORAGE [OUT.json] | fonttables"
      );
      std::process::exit(2);
    }
  }
}
