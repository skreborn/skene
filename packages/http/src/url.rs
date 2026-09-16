//! An absolute URL taken apart for a transport that sends its parts separately.

use crate::Error;

/// The parts of an absolute HTTP URL: whether it is secure, its authority, and its
/// path with the query, `/` at least.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parts {
  pub(crate) secure: bool,
  pub(crate) authority: String,
  pub(crate) path: String,
}

/// Takes `url` apart; a relative URL or another scheme is an error.
pub(crate) fn parts(url: &str) -> Result<Parts, Error> {
  let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
    (true, rest)
  }
  else if let Some(rest) = url.strip_prefix("http://") {
    (false, rest)
  }
  else {
    return Err(Error::Url(format!(
      "{url:?} is not an absolute http or https URL"
    )));
  };
  let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));

  if authority.is_empty() {
    return Err(Error::Url(format!("{url:?} names no host")));
  }

  Ok(Parts {
    secure,
    authority: authority.to_owned(),
    path: format!("/{path}"),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The scheme decides security, the authority keeps its port, the path keeps its
  /// query and is `/` when absent, and a relative URL is refused.
  #[test]
  fn urls_come_apart() {
    assert_eq!(
      parts("https://example.com:8443/a/b?c=1").unwrap(),
      Parts {
        secure: true,
        authority: "example.com:8443".to_owned(),
        path: "/a/b?c=1".to_owned(),
      }
    );
    assert_eq!(parts("http://localhost").unwrap().path, "/");
    assert!(!parts("http://localhost").unwrap().secure);
    assert!(matches!(parts("/relative"), Err(Error::Url(_))));
    assert!(matches!(parts("ftp://x/"), Err(Error::Url(_))));
    assert!(matches!(parts("http:///x"), Err(Error::Url(_))));
  }
}
