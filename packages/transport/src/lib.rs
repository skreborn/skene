//! # skene-transport
//!
//! The clients a skene app reaches the network with, built on the framework's public
//! seams alone: [`http`] sends a request and awaits its response, [`ws`] opens a
//! WebSocket and awaits its messages. Each runs inside any task the facade runs—a
//! `Resource`'s load, a `spawn`ed future—over the platform's own transport: the
//! browser's `fetch` and `WebSocket` on the web, `wasi:http` and `wasi:sockets`
//! through the server leaf's reactor on a server render, and a thread around a plain
//! client on every other host, where the future wakes the platform's executor.
//! Nothing in the framework knows this crate exists, so a third-party client sits on
//! exactly the same footing.

pub mod http;
pub mod ws;
