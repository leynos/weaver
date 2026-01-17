//! JSONL request dispatch for daemon command handling.
//!
//! This module implements the request dispatch loop that reads `CommandRequest`
//! messages from connected clients, routes them to domain handlers, and streams
//! `DaemonMessage` responses back. The dispatcher integrates with the transport
//! layer via the `ConnectionHandler` trait.
//!
//! ## Protocol
//!
//! Clients send a single JSONL request line containing a `CommandRequest`:
//!
//! ```json
//! {"command":{"domain":"observe","operation":"get-definition"},"arguments":[]}
//! ```
//!
//! The daemon responds with zero or more `Stream` messages followed by a
//! terminal `Exit` message:
//!
//! ```json
//! {"kind":"stream","stream":"stderr","data":"observe get-definition: not yet implemented\n"}
//! {"kind":"exit","status":1}
//! ```
//!
//! ## Domain Routing
//!
//! Requests are routed by domain (`observe`, `act`, `verify`) and then by
//! operation within each domain. Unknown domains or operations result in
//! structured error responses.

mod errors;
mod handler;
pub mod observe;
mod request;
mod response;
mod router;

pub(crate) use self::handler::DispatchConnectionHandler;
