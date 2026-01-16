//! Socket listener for daemon transport endpoints.
//!
//! The transport module binds to configured socket endpoints and accepts
//! connections in a background thread.

mod errors;
mod handler;
mod listener;
#[cfg(test)]
mod listener_tests;
#[cfg(test)]
mod test_utils;

pub(crate) use self::errors::ListenerError;
pub(crate) use self::handler::{ConnectionHandler, ConnectionStream};
#[cfg(test)]
pub(crate) use self::listener::ListenerHandle;
pub(crate) use self::listener::SocketListener;
#[cfg(test)]
pub(crate) use self::test_utils::CountingHandler;

const LISTENER_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::transport");
