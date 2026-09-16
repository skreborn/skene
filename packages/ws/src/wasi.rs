//! No transport on a server render: a socket outlives a render, and a page carries
//! none.

use crate::{Error, Message, Receiver};

/// The link of a socket that never opens.
pub(crate) struct Link;

#[expect(
  clippy::unused_self,
  reason = "the link's surface is the same on every platform"
)]
impl Link {
  pub(crate) fn send(&self, _message: Message) -> Result<(), Error> {
    Err(Error::Unsupported)
  }

  pub(crate) fn close(&self) {}
}

/// Refuses every connection, the receiver closed from the start.
pub(crate) async fn connect(_url: String, receiver: Receiver) -> Result<Link, Error> {
  receiver.close();

  Err(Error::Unsupported)
}
