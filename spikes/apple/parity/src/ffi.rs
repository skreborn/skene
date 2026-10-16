//! The C entry the iOS test links: the suite configured by JSON, answering its summary.

use std::ffi::{CStr, CString, c_char};

/// Runs the suite configured by `config` (JSON, every field optional) and answers its results
/// summary, or `error: ...`; free it with [`apple_parity_free`].
///
/// # Safety
///
/// `config` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apple_parity_run(config: *const c_char) -> *mut c_char {
  let config = unsafe { CStr::from_ptr(config) }
    .to_string_lossy()
    .into_owned();
  let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    crate::suite::run_json(&config)
  }));
  let text = match result {
    Ok(Ok(text)) => text,
    Ok(Err(error)) => format!("error: {error}"),
    Err(panic) => format!(
      "error: panicked: {}",
      panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
        .unwrap_or_default()
    ),
  };

  CString::new(text.replace('\0', " "))
    .unwrap_or_default()
    .into_raw()
}

/// Runs the language line-height probe alone, answering its JSON.
#[unsafe(no_mangle)]
pub extern "C" fn apple_parity_language() -> *mut c_char {
  #[allow(unused_mut)]
  let mut report = crate::probes::Report::default();

  #[cfg(target_os = "ios")]
  crate::ios::language(&mut report);

  let json = serde_json::to_string(&report.json).unwrap_or_default();

  CString::new(json).unwrap_or_default().into_raw()
}

/// Runs the font start-up and memory probe ([`crate::fontmem`]) with fonts stored as `storage`
/// (`copy`, `layout`, `map`, `map-all`), answering its JSON, or `error: ...`.
///
/// # Safety
///
/// `storage` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apple_parity_fontmem(storage: *const c_char) -> *mut c_char {
  let started = std::time::Instant::now();
  let name = unsafe { CStr::from_ptr(storage) }
    .to_string_lossy()
    .into_owned();
  let text = match crate::tuned::Storage::parse(&name) {
    Some(storage) => {
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::fontmem::run(storage, started).to_string()
      }))
      .unwrap_or_else(|_| "error: panicked".to_owned())
    }
    None => format!("error: no storage {name:?}"),
  };

  CString::new(text.replace('\0', " "))
    .unwrap_or_default()
    .into_raw()
}

/// Frees a string [`apple_parity_run`] answered.
///
/// # Safety
///
/// `text` must be a string [`apple_parity_run`] answered, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apple_parity_free(text: *mut c_char) {
  if !text.is_null() {
    drop(unsafe { CString::from_raw(text) });
  }
}
