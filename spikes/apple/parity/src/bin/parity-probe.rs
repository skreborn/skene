//! The suite from a command line: `parity-probe suite '{"out": "DIR"}'` runs everything,
//! `parity-probe language` prints the language line-height probe (iOS, under
//! `xcrun simctl spawn ... -AppleLanguages "(th)"`).

fn main() {
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
    _ => {
      eprintln!("usage: parity-probe suite [CONFIG.json] | language");
      std::process::exit(2);
    }
  }
}
