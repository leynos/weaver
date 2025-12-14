//! Sandboxing utilities for Weaver processes.
//!
//! The `weaver-sandbox` crate wraps the [`birdcage`] library with policy
//! defaults aligned to Weaver's zero-trust design. Callers describe the
//! resources a subprocess is permitted to access using a [`SandboxProfile`],
//! then launch that subprocess through a [`Sandbox`]. Linux namespaces and
//! `seccomp-bpf` filters are applied automatically via `birdcage`.
//!
//! The sandbox is intentionally restrictive:
//! - Networking is disabled unless explicitly enabled.
//! - Environment variables are stripped unless whitelisted.
//! - Executables must be whitelisted and provided as absolute paths.
//! - Standard library locations on Linux are whitelisted by default to keep
//!   dynamically linked binaries functional without exposing the wider
//!   filesystem.
//!
//! ```rust,no_run
//! use weaver_sandbox::{
//!     Sandbox, SandboxCommand, SandboxProfile, process::Stdio,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let profile = SandboxProfile::new()
//!     .allow_executable("/bin/echo")
//!     .allow_networking();
//!
//! let mut command = SandboxCommand::new("/bin/echo");
//! command.arg("hello from the cage").stdout(Stdio::piped());
//!
//! let sandbox = Sandbox::new(profile);
//! let mut child = sandbox.spawn(command)?;
//! let output = child.wait_with_output()?;
//! assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from the cage\n");
//! # Ok(()) }
//! ```
//!
//! Callers should invoke [`Sandbox::spawn`] from a single-threaded context.
//! When multiple threads are active the sandbox returns a
//! [`SandboxError::MultiThreaded`] rather than panicking on the internal
//! assertion used by `birdcage`.

pub(crate) mod env_guard;
mod error;
mod profile;
mod runtime;
mod sandbox;

pub use birdcage::process;
pub use error::SandboxError;
pub use profile::{EnvironmentPolicy, NetworkPolicy, SandboxProfile};
pub use sandbox::{Sandbox, SandboxChild, SandboxCommand, SandboxOutput};
