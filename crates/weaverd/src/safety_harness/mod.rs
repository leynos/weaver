//! Double-Lock safety harness for safe code modifications.
//!
//! The safety harness wraps every actuation command in a verifiable transaction.
//! All proposed changes are validated against a two-phase verification process
//! before being committed to the filesystem:
//!
//! 1. **Syntactic Lock**: Modified files are parsed to ensure they produce valid
//!    syntax trees. This catches structural errors such as unbalanced braces or
//!    broken statements.
//!
//! 2. **Semantic Lock**: Changes are submitted to the configured LSP server,
//!    which verifies that no new errors or high-severity warnings are introduced
//!    compared to the pre-edit state.
//!
//! Changes that fail either lock are rejected, leaving the filesystem untouched
//! and returning a structured error describing the failure.
//!
//! The harness operates in-memory by applying proposed diffs to virtual file
//! buffers. Only when both locks pass are the changes atomically committed to
//! the real filesystem.
//!
//! # Design
//!
//! The harness follows the broker process pattern described in the design
//! document. Plugin outputs are captured as diffs (or text edits), validated
//! through both locks, and only then written to disk. This zero-trust approach
//! treats all external tool output as potentially problematic until proven safe.

mod edit;
mod error;
mod locks;
mod transaction;
mod verification;

pub use edit::{FileEdit, Position, ReplacementText, TextEdit, TextRange};
pub use error::{LockPhase, SafetyHarnessError, VerificationFailure};
pub use locks::{SemanticLockResult, SyntacticLockResult};
pub use transaction::{EditTransaction, TransactionOutcome};
pub use verification::{
    ConfigurableSemanticLock, ConfigurableSyntacticLock, PlaceholderSemanticLock,
    PlaceholderSyntacticLock, SemanticLock, SyntacticLock, VerificationContext,
};
