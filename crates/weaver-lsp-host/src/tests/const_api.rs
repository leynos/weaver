//! Compile-time coverage for the `const fn` items only the crate can name.
//!
//! The promoted `const` surface in `weaver-lsp-host` includes items that are
//! not public: the [`LspHostError`] constructors are `pub(crate)`, and
//! [`CapabilitySummary`] has private fields. An integration test in `tests/`
//! cannot call either, so the witnesses live here rather than in
//! `tests/const_api.rs`, whose wrappers cover the public accessors.
//!
//! Rust rejects a call to a non-`const` function from inside a `const fn`
//! body; each wrapper therefore fails to compile if its target loses `const`.
//! The wrappers run from an ordinary test because the workspace's
//! warnings-as-errors gate rejects dead code.

use weaver_config::CapabilityMatrix;

use crate::{
    CapabilityKind,
    CapabilitySource,
    HostOperation,
    Language,
    LanguageServerError,
    LspHostError,
    ServerCapabilitySet,
    capability::{CapabilitySummary, resolve_capabilities},
};

/// Compiles only while [`CapabilitySummary::language`] is a `const fn`.
#[must_use]
const fn language_of(summary: &CapabilitySummary) -> Language { summary.language() }

/// Compiles only while [`CapabilitySummary::position_encoding`] is a
/// `const fn`.
#[must_use]
const fn advertises_position_encoding(summary: &CapabilitySummary) -> bool {
    summary.position_encoding().is_some()
}

/// Compiles only while [`LspHostError::unknown`] is a `const fn`.
#[must_use]
const fn unknown_error(language: Language) -> LspHostError { LspHostError::unknown(language) }

/// Compiles only while [`LspHostError::duplicate`] is a `const fn`.
#[must_use]
const fn duplicate_error(language: Language) -> LspHostError { LspHostError::duplicate(language) }

/// Compiles only while [`LspHostError::capability_unavailable`] is a
/// `const fn`.
#[must_use]
const fn unavailable_error(
    language: Language,
    capability: CapabilityKind,
    reason: CapabilitySource,
) -> LspHostError {
    LspHostError::capability_unavailable(language, capability, reason)
}

/// Compiles only while [`LspHostError::server`] is a `const fn`.
#[must_use]
const fn server_error(
    language: Language,
    operation: HostOperation,
    source: LanguageServerError,
) -> LspHostError {
    LspHostError::server(language, operation, source)
}

/// Checks that every wrapper builds or reads the value the direct call does.
#[test]
fn const_accessors_agree_with_direct_calls() {
    let advertised = ServerCapabilitySet::new(true, false, true);
    let summary = resolve_capabilities(Language::Rust, &advertised, &CapabilityMatrix::default());

    assert_eq!(language_of(&summary), Language::Rust);
    assert!(
        !advertises_position_encoding(&summary),
        "the server advertised no position encoding"
    );

    assert_eq!(
        unknown_error(Language::Python).to_string(),
        "no language server registered for 'python'"
    );
    assert_eq!(
        duplicate_error(Language::Rust).to_string(),
        "language 'rust' already has a registered server"
    );
    assert_eq!(
        unavailable_error(
            Language::Rust,
            CapabilityKind::Hover,
            CapabilitySource::MissingOnServer,
        )
        .to_string(),
        "capability Hover for rust is unavailable: missing from server"
    );
    assert_eq!(
        server_error(
            Language::Rust,
            HostOperation::Initialise,
            LanguageServerError::new("boom"),
        )
        .to_string(),
        "language server for rust failed during initialisation: boom"
    );
}
