//! Unit tests for small host behaviours.

use std::str::FromStr;

use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::{CapabilityKind, CapabilitySource},
    errors::{HostOperation, LspHostError},
    language::Language,
    server::{LanguageServer, LanguageServerError, ServerCapabilitySet},
    tests::support::{
        CallKind,
        RecordingLanguageServer,
        ResponseSet,
        TestWorld,
        definition_params,
        did_change_params,
        did_close_params,
        did_open_params,
        sample_uri,
    },
};

macro_rules! failing_server {
    ($name:ident, $method:ident, $message:expr) => {
        struct $name;

        impl LanguageServer for $name {
            fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
                Ok(ServerCapabilitySet::new(true, true, true))
            }

            fn goto_definition(
                &mut self,
                _params: lsp_types::GotoDefinitionParams,
            ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
                fail_if(
                    FailingMethod::$method,
                    FailingMethod::GotoDefinition,
                    $message,
                )?;
                Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
            }

            fn references(
                &mut self,
                _params: lsp_types::ReferenceParams,
            ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
                Ok(Vec::new())
            }

            fn diagnostics(
                &mut self,
                _uri: lsp_types::Uri,
            ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
                Ok(Vec::new())
            }

            fn did_open(
                &mut self,
                _params: lsp_types::DidOpenTextDocumentParams,
            ) -> Result<(), LanguageServerError> {
                fail_if(FailingMethod::$method, FailingMethod::DidOpen, $message)?;
                Ok(())
            }

            fn did_change(
                &mut self,
                _params: lsp_types::DidChangeTextDocumentParams,
            ) -> Result<(), LanguageServerError> {
                fail_if(FailingMethod::$method, FailingMethod::DidChange, $message)?;
                Ok(())
            }

            fn did_close(
                &mut self,
                _params: lsp_types::DidCloseTextDocumentParams,
            ) -> Result<(), LanguageServerError> {
                fail_if(FailingMethod::$method, FailingMethod::DidClose, $message)?;
                Ok(())
            }

            fn prepare_call_hierarchy(
                &mut self,
                _params: lsp_types::CallHierarchyPrepareParams,
            ) -> Result<Option<Vec<lsp_types::CallHierarchyItem>>, LanguageServerError> {
                Ok(None)
            }

            fn incoming_calls(
                &mut self,
                _params: lsp_types::CallHierarchyIncomingCallsParams,
            ) -> Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>, LanguageServerError>
            {
                Ok(None)
            }

            fn outgoing_calls(
                &mut self,
                _params: lsp_types::CallHierarchyOutgoingCallsParams,
            ) -> Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>, LanguageServerError>
            {
                Ok(None)
            }

            fn hover(
                &mut self,
                _params: lsp_types::HoverParams,
            ) -> Result<Option<lsp_types::Hover>, LanguageServerError> {
                Ok(None)
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailingMethod {
    GotoDefinition,
    DidOpen,
    DidChange,
    DidClose,
}

fn fail_if(
    failing: FailingMethod,
    current: FailingMethod,
    message: &str,
) -> Result<(), LanguageServerError> {
    if failing == current {
        return Err(LanguageServerError::new(message));
    }
    Ok(())
}

failing_server!(FailingDefinitionServer, GotoDefinition, "boom");
failing_server!(FailingDidChangeServer, DidChange, "change failed");
failing_server!(FailingDidOpenServer, DidOpen, "open failed");
failing_server!(FailingDidCloseServer, DidClose, "close failed");

#[rstest]
fn applies_force_and_deny_overrides() {
    let mut overrides = CapabilityMatrix::default();
    overrides.set_override(
        Language::Rust.as_str(),
        CapabilityKind::Diagnostics.key(),
        CapabilityOverride::Deny,
    );
    overrides.set_override(
        Language::Rust.as_str(),
        CapabilityKind::References.key(),
        CapabilityOverride::Force,
    );

    let config = vec![crate::tests::support::TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, false, false),
        responses: ResponseSet::default(),
        initialization_error: None,
    }];
    let mut world = TestWorld::new(config, overrides).expect("stub server should register");

    world.initialize(Language::Rust);
    let summary = world
        .last_capabilities
        .take()
        .expect("missing capabilities");

    let references = summary.state(CapabilityKind::References);
    assert!(references.enabled);
    assert_eq!(references.source, CapabilitySource::ForcedOverride);

    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    assert!(!diagnostics.enabled);
    assert_eq!(diagnostics.source, CapabilitySource::DeniedOverride);
}

#[rstest]
fn parses_known_languages() {
    assert_eq!(
        Language::from_str("rust").expect("rust should parse"),
        Language::Rust
    );
    assert_eq!(
        Language::from_str("python").expect("python should parse"),
        Language::Python
    );
    assert_eq!(
        Language::from_str("typescript").expect("typescript should parse"),
        Language::TypeScript
    );
}

#[rstest]
fn parses_typescript_alias_ts() {
    assert_eq!(
        Language::from_str("ts").expect("ts alias should parse"),
        Language::TypeScript
    );
}

#[rstest]
fn trims_whitespace_in_language_parse() {
    assert_eq!(
        Language::from_str(" rust ").expect("padded rust should parse"),
        Language::Rust
    );
    assert_eq!(
        Language::from_str("\tpython\n").expect("padded python should parse"),
        Language::Python
    );
}

#[rstest]
fn rejects_invalid_language_with_message() {
    let err = Language::from_str("go").unwrap_err();
    assert_eq!(err.input(), "go");
    assert_eq!(err.to_string(), "unsupported language 'go'");
}

#[rstest]
fn rejects_duplicate_language_registration() {
    let server = RecordingLanguageServer::new(
        ServerCapabilitySet::new(true, true, true),
        ResponseSet::default(),
    );
    let mut host = crate::LspHost::new(CapabilityMatrix::default());

    assert!(
        host.register_language(Language::Rust, Box::new(server.clone()))
            .is_ok()
    );
    match host.register_language(Language::Rust, Box::new(server)) {
        Err(LspHostError::DuplicateLanguage { .. }) => {}
        other => panic!("expected duplicate language error, got {other:?}"),
    }
}

#[rstest]
fn reports_unknown_language_on_request() {
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    let params = definition_params().expect("definition params should build");
    match host.goto_definition(Language::Rust, params) {
        Err(LspHostError::UnknownLanguage { .. }) => {}
        other => panic!("expected unknown language error, got {other:?}"),
    }
}

#[rstest]
fn propagates_server_error_from_definition() {
    let params = definition_params().expect("definition params should build");
    let context = propagated_server_error(FailingDefinitionServer, |host| {
        host.goto_definition(Language::Rust, params)
    })
    .expect("stub server should register");
    assert_eq!(context, Some((Language::Rust, HostOperation::Definition)));
}

#[rstest]
fn propagates_server_error_from_did_change() {
    let params = did_change_params().expect("did-change params should build");
    let context = propagated_server_error(FailingDidChangeServer, |host| {
        host.did_change(Language::Rust, params)
    })
    .expect("stub server should register");
    assert_eq!(context, Some((Language::Rust, HostOperation::DidChange)));
}

#[rstest]
fn propagates_server_error_from_did_open() {
    let params = did_open_params().expect("did-open params should build");
    let context = propagated_server_error(FailingDidOpenServer, |host| {
        host.did_open(Language::Rust, params)
    })
    .expect("stub server should register");
    assert_eq!(context, Some((Language::Rust, HostOperation::DidOpen)));
}

#[rstest]
fn propagates_server_error_from_did_close() {
    let params = did_close_params().expect("did-close params should build");
    let context = propagated_server_error(FailingDidCloseServer, |host| {
        host.did_close(Language::Rust, params)
    })
    .expect("stub server should register");
    assert_eq!(context, Some((Language::Rust, HostOperation::DidClose)));
}

#[rstest]
fn calls_initialise_before_requests() {
    let uri = sample_uri().expect("sample URI should parse");
    let calls = recorded_calls(|host| host.diagnostics(Language::Rust, uri))
        .expect("recording server should register");
    assert!(
        calls.starts_with(&[CallKind::Initialise]),
        "initialise should precede requests: {calls:?}"
    );
}

#[rstest]
fn calls_initialise_before_document_sync() {
    let params = did_open_params().expect("did-open params should build");
    let calls = recorded_calls(|host| host.did_open(Language::Rust, params))
        .expect("recording server should register");
    assert!(
        calls.starts_with(&[CallKind::Initialise, CallKind::DidOpen]),
        "initialise should precede didOpen: {calls:?}"
    );
}

/// Builds a host with `server` registered as the Rust language server.
fn host_with_rust_server(
    server: impl LanguageServer + 'static,
) -> Result<crate::LspHost, LspHostError> {
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    host.register_language(Language::Rust, Box::new(server))?;
    Ok(host)
}

/// Reads the language and operation out of a host server error, if that is
/// what the outcome holds.
fn server_error_context<T>(outcome: &Result<T, LspHostError>) -> Option<(Language, HostOperation)> {
    match outcome {
        Err(LspHostError::Server {
            language,
            operation,
            ..
        }) => Some((*language, *operation)),
        _ => None,
    }
}

/// Runs `call` against a host registered with `server` and reports the
/// server-error context of the outcome, or the registration failure.
fn propagated_server_error<T, F>(
    server: impl LanguageServer + 'static,
    call: F,
) -> Result<Option<(Language, HostOperation)>, LspHostError>
where
    F: FnOnce(&mut crate::LspHost) -> Result<T, LspHostError>,
    T: std::fmt::Debug,
{
    let mut host = host_with_rust_server(server)?;
    let outcome = call(&mut host);
    Ok(server_error_context(&outcome))
}

/// Exercises `call` against a recording Rust server and returns the calls it
/// observed, discarding the call's own outcome.
fn recorded_calls<T, F>(call: F) -> Result<Vec<CallKind>, LspHostError>
where
    F: FnOnce(&mut crate::LspHost) -> Result<T, LspHostError>,
{
    let server = RecordingLanguageServer::new(
        ServerCapabilitySet::new(true, true, true),
        ResponseSet::default(),
    );
    let handle = server.handle();
    let mut host = host_with_rust_server(server)?;

    let _ = call(&mut host);

    Ok(handle.calls())
}
