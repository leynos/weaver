//! Socket listener for daemon transport endpoints.
//!
//! The transport module binds to configured socket endpoints and accepts
//! connections in a background thread.

mod errors;
mod handler;
mod listener;
#[cfg(test)]
mod listener_tests;
#[cfg(unix)]
mod listener_unix;
#[cfg(test)]
mod test_utils;

#[doc(hidden)]
pub use self::handler::{ConnectionHandler, ConnectionStream};
#[cfg(test)]
pub(crate) use self::listener::ListenerHandle;
#[cfg(test)]
pub(crate) use self::test_utils::CountingHandler;
pub(crate) use self::{errors::ListenerError, listener::SocketListener};

const LISTENER_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::transport");
