//! What the shell's stand-ins share: the steps a scenario paces them by.

use std::time::Duration;

/// The steps a scenario paces a stand-in by: each `SIGUSR1` the stand-in is sent is one step.
///
/// The signal is blocked from [`Steps::block`] on, in the thread that blocks it and in every
/// thread that thread starts after, so a step asked for before the stand-in waits for it is
/// kept for [`Steps::next`] rather than ending the process.
pub struct Steps {
  signals: libc::sigset_t,
}

impl Steps {
  /// Blocks `SIGUSR1`: called before the stand-in starts any thread.
  pub fn block() -> Self {
    // SAFETY: the set is initialized by sigemptyset before it is read, and the mask is changed
    // for this thread alone.
    unsafe {
      let mut signals: libc::sigset_t = std::mem::zeroed();

      libc::sigemptyset(&raw mut signals);
      libc::sigaddset(&raw mut signals, libc::SIGUSR1);
      libc::pthread_sigmask(libc::SIG_BLOCK, &raw const signals, std::ptr::null_mut());

      Self { signals }
    }
  }

  /// Waits at most `within` for the next step: false where none came.
  pub fn next(&self, within: Duration) -> bool {
    let timeout = libc::timespec {
      tv_sec: libc::time_t::try_from(within.as_secs()).unwrap_or(libc::time_t::MAX),
      tv_nsec: libc::c_long::from(within.subsec_nanos()),
    };

    loop {
      // SAFETY: the set and the timeout outlive the call, and no signal information is asked
      // for.
      let taken = unsafe {
        libc::sigtimedwait(
          &raw const self.signals,
          std::ptr::null_mut(),
          &raw const timeout,
        )
      };

      if taken == libc::SIGUSR1 {
        return true;
      }

      if std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
        return false;
      }
    }
  }
}
