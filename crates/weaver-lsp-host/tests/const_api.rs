//! Compile-time coverage for the public `const fn` surface of
//! `weaver-lsp-host`.
//!
//! Rust rejects a call to a non-`const` function from inside a `const fn`
//! body, so each wrapper below pins its accessor's const-ness at compile time.
//! The wrappers are called from an ordinary test because the workspace's
//! warnings-as-errors gate rejects dead code, and because building the values
//! they inspect is not a const operation.
//!
//! Items that only the crate itself can name are covered by
//! `src/tests/const_api.rs`; a fixture here could not observe them.

use weaver_lsp_host::{
    CapabilityKind,
    CapabilitySource,
    CapabilityState,
    Language,
    LanguageServerError,
    ServerCapabilitySet,
};

/// Compiles only while [`CapabilityKind::key`] is a `const fn`.
#[must_use]
const fn key_of(kind: CapabilityKind) -> &'static str { kind.key() }

/// Compiles only while [`CapabilityState::new`] is a `const fn`.
#[must_use]
const fn state_from(
    kind: CapabilityKind,
    enabled: bool,
    source: CapabilitySource,
) -> CapabilityState {
    CapabilityState::new(kind, enabled, source)
}

/// Compiles only while [`Language::as_str`] is a `const fn`.
#[must_use]
const fn as_str_of(language: Language) -> &'static str { language.as_str() }

/// Compiles only while [`ServerCapabilitySet::position_encoding`] is a
/// `const fn`.
///
/// The wrapper maps the encoding to a `bool` so the test file need not name
/// `lsp_types::PositionEncodingKind`, which `weaver-lsp-host` does not
/// re-export.
#[must_use]
const fn advertises_position_encoding(set: &ServerCapabilitySet) -> bool {
    set.position_encoding().is_some()
}

/// Compiles only while [`LanguageServerError::message`] is a `const fn`.
#[must_use]
const fn message_of(error: &LanguageServerError) -> &str { error.message() }

/// Checks that every wrapper agrees with the direct call.
#[test]
fn const_accessors_agree_with_direct_calls() {
    assert_eq!(
        key_of(CapabilityKind::Definition),
        CapabilityKind::Definition.key()
    );

    let state = state_from(
        CapabilityKind::Hover,
        true,
        CapabilitySource::ForcedOverride,
    );
    assert_eq!(
        state,
        CapabilityState {
            kind: CapabilityKind::Hover,
            enabled: true,
            source: CapabilitySource::ForcedOverride,
        },
        "the wrapper must not alter the state it builds"
    );

    assert_eq!(as_str_of(Language::Rust), "rust");

    let without_encoding = ServerCapabilitySet::new(true, false, true).with_position_encoding(None);
    assert!(!advertises_position_encoding(&without_encoding));

    let error = LanguageServerError::new("handshake failed");
    assert_eq!(message_of(&error), "handshake failed");
}
