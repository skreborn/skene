//! A font file mapped into memory, read-only, as fontique maps the files it scans: its pages are
//! the file's own, clean and shared with every other process mapping it, so they count toward no
//! process's footprint.
//!
//! A face of a collection is registered alone by giving the mapping a private copy of the
//! collection's header that lists that face only: one page written, every other page the file's.

use std::{ffi::CString, os::raw::c_void};

/// What opening and mapping a file answered.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Access {
  /// The file's size in bytes, where `fstat` answered.
  pub size: Option<u64>,
  /// `errno` where `open` failed.
  pub open_errno: Option<i32>,
  /// `errno` where `mmap` failed.
  pub map_errno: Option<i32>,
  /// Microseconds `open`, `fstat` and `mmap` took.
  pub micros: u128,
}

/// A read-only mapping of a font file.
pub struct Mapped {
  pointer: *mut c_void,
  length: usize,
}

// SAFETY: the mapping is read-only once made and owned by this value alone.
unsafe impl Send for Mapped {}
// SAFETY: as above.
unsafe impl Sync for Mapped {}

fn errno() -> i32 {
  std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

impl Mapped {
  /// Maps the file at `path`; with `face`, a collection's header lists that face alone.
  ///
  /// # Errors
  ///
  /// What opening or mapping the file answered.
  pub fn open(path: &str, face: Option<u32>) -> Result<(Self, Access), Access> {
    let started = std::time::Instant::now();
    let mut access = Access::default();
    let Ok(name) = CString::new(path)
    else {
      return Err(access);
    };
    // SAFETY: a valid NUL-terminated path.
    let descriptor = unsafe { libc::open(name.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };

    if descriptor < 0 {
      access.open_errno = Some(errno());

      return Err(access);
    }

    // SAFETY: a zeroed stat is valid, the descriptor is open.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let length = if unsafe { libc::fstat(descriptor, &raw mut stat) } == 0 {
      stat.st_size as usize
    }
    else {
      0
    };

    access.size = Some(length as u64);

    // A private mapping, writable only while a collection's header is rewritten.
    let pointer = if length > 0 {
      // SAFETY: the descriptor is open and `length` its size.
      unsafe {
        libc::mmap(
          std::ptr::null_mut(),
          length,
          libc::PROT_READ | libc::PROT_WRITE,
          libc::MAP_PRIVATE,
          descriptor,
          0,
        )
      }
    }
    else {
      libc::MAP_FAILED
    };

    if pointer == libc::MAP_FAILED {
      access.map_errno = Some(errno());
      // SAFETY: the descriptor is open.
      unsafe { libc::close(descriptor) };

      return Err(access);
    }

    // SAFETY: the descriptor is open; the mapping holds the file.
    unsafe { libc::close(descriptor) };

    let mapped = Self { pointer, length };

    if let Some(face) = face {
      mapped.select(face);
    }

    // SAFETY: the mapping is `length` bytes long.
    unsafe { libc::mprotect(pointer, length, libc::PROT_READ) };
    access.micros = started.elapsed().as_micros();

    Ok((mapped, access))
  }

  /// Rewrites a collection's header (`ttcf`) to list face `face` alone.
  fn select(&self, face: u32) {
    // SAFETY: the mapping is writable here and `length` bytes long; every access is checked.
    let bytes = unsafe { std::slice::from_raw_parts_mut(self.pointer.cast::<u8>(), self.length) };

    if bytes.len() < 12 || &bytes[..4] != b"ttcf" {
      return;
    }

    let count = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let at = 12 + 4 * face as usize;

    if face >= count || bytes.len() < at + 4 {
      return;
    }

    let offset = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];

    bytes[8..12].copy_from_slice(&1u32.to_be_bytes());
    bytes[12..16].copy_from_slice(&offset);
  }

  /// The mapped length in bytes.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.length
  }

  /// Whether the mapping is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.length == 0
  }
}

impl AsRef<[u8]> for Mapped {
  fn as_ref(&self) -> &[u8] {
    // SAFETY: the mapping is `length` readable bytes for this value's life.
    unsafe { std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length) }
  }
}

impl Drop for Mapped {
  fn drop(&mut self) {
    // SAFETY: the mapping was made by `open` and is unmapped once.
    unsafe { libc::munmap(self.pointer, self.length) };
  }
}

/// The face of the font file `data` whose PostScript name is `postscript`: its index, and how
/// many faces the file holds.
#[must_use]
pub fn face_of(data: &[u8], postscript: &str) -> Option<(u32, u32)> {
  let count = match skrifa::raw::FileRef::new(data).ok()? {
    skrifa::raw::FileRef::Collection(collection) => collection.len(),
    skrifa::raw::FileRef::Font(_) => 1,
  };

  (0..count)
    .find(|index| {
      text_parity::fonts::names_of(data, *index).is_some_and(|(_, name)| name == postscript)
    })
    .map(|index| (index, count))
}
