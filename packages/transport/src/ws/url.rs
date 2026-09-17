//! An absolute URL taken apart for a transport that reaches its host itself.

use super::Error;

/// The parts of an absolute WebSocket URL: whether it is secure, its host, and its
/// port, the scheme's own when the URL names none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parts {
  pub(crate) secure: bool,
  pub(crate) host: String,
  pub(crate) port: u16,
}

/// Takes `url` apart; a relative URL or another scheme is an error.
pub(crate) fn parts(url: &str) -> Result<Parts, Error> {
  let (secure, rest) = if let Some(rest) = url.strip_prefix("wss://") {
    (true, rest)
  }
  else if let Some(rest) = url.strip_prefix("ws://") {
    (false, rest)
  }
  else {
    return Err(Error::Url(format!(
      "{url:?} is not an absolute ws or wss URL"
    )));
  };
  let (authority, _path) = rest.split_once('/').unwrap_or((rest, ""));

  if authority.is_empty() {
    return Err(Error::Url(format!("{url:?} names no host")));
  }

  let (host, port) = match authority.strip_prefix('[') {
    Some(bracketed) => {
      let (host, after) = bracketed
        .split_once(']')
        .ok_or_else(|| Error::Url(format!("{url:?} leaves its host open")))?;

      (host, after.strip_prefix(':'))
    }
    None => match authority.rsplit_once(':') {
      Some((host, port)) => (host, Some(port)),
      None => (authority, None),
    },
  };
  let port = match port {
    Some(port) => port
      .parse()
      .map_err(|_| Error::Url(format!("{url:?} names no port in {port:?}")))?,
    None if secure => 443,
    None => 80,
  };

  if host.is_empty() {
    return Err(Error::Url(format!("{url:?} names no host")));
  }

  Ok(Parts {
    secure,
    host: host.to_owned(),
    port,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The scheme decides security and the default port, an explicit port and a
  /// bracketed IPv6 host are kept, and a relative URL or another scheme is refused.
  #[test]
  fn urls_come_apart() {
    assert_eq!(
      parts("wss://example.com:8443/a/b?c=1").unwrap(),
      Parts {
        secure: true,
        host: "example.com".to_owned(),
        port: 8443,
      }
    );
    assert_eq!(
      parts("ws://[::1]:9001/").unwrap(),
      Parts {
        secure: false,
        host: "::1".to_owned(),
        port: 9001,
      }
    );
    assert_eq!(parts("ws://localhost").unwrap().port, 80);
    assert_eq!(parts("wss://localhost").unwrap().port, 443);
    assert!(matches!(parts("/relative"), Err(Error::Url(_))));
    assert!(matches!(parts("http://x/"), Err(Error::Url(_))));
    assert!(matches!(parts("ws:///x"), Err(Error::Url(_))));
    assert!(matches!(parts("ws://x:port/"), Err(Error::Url(_))));
  }
}
