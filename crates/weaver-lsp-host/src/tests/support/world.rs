//! BDD test world encapsulating the host and stub servers.

use std::collections::HashMap;

use lsp_types::{Diagnostic, GotoDefinitionResponse, Location, Uri};

use crate::capability::CapabilitySummary;
use crate::errors::LspHostError;
use crate::language::Language;
use crate::server::ServerCapabilitySet;
use crate::LspHost;

use super::recording_server::{CallKind, RecordingLanguageServer, RecordingServerHandle, ResponseSet};

/// Configuration used to seed a stub server for a language.
#[derive(Debug, Clone)]
pub struct TestServerConfig {
    /// Language served by the stub.
    pub language: Language,
    /// Capabilities reported during initialisation.
    pub capabilities: ServerCapabilitySet,
    /// Responses returned for core operations.
    pub responses: ResponseSet,
    /// Optional error to surface during initialisation.
    pub initialisation_error: Option<String>,
}

impl TestServerConfig {
    /// Builds a config with the supplied capabilities and default responses.
    #[must_use]
    pub fn with_defaults(language: Language, capabilities: ServerCapabilitySet) -> Self {
        Self {
            language,
            capabilities,
            responses: ResponseSet::default(),
            initialisation_error: None,
        }
    }
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            language: Language::Rust,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: ResponseSet::default(),
            initialisation_error: None,
        }
    }
}

/// Shared state exercised by BDD step implementations.
pub struct TestWorld {
    /// Configurations used to rebuild the host between steps.
    configs: Vec<TestServerConfig>,
    /// Host instance under test.
    pub host: LspHost,
    handles: HashMap<Language, RecordingServerHandle>,
    /// Last error observed while exercising the host.
    pub last_error: Option<LspHostError>,
    /// Last definition response observed.
    pub last_definition: Option<GotoDefinitionResponse>,
    /// Last references response observed.
    pub last_references: Option<Vec<Location>>,
    /// Last diagnostics response observed.
    pub last_diagnostics: Option<Vec<Diagnostic>>,
    /// Last capability summary observed.
    pub last_capabilities: Option<CapabilitySummary>,
}

impl TestWorld {
    macro_rules! request_method {
        (
            $(#[$meta:meta])* $name:ident,
            $host_method:ident,
            $param_ty:ty,
            $field:ident,
            $response_ty:ty
        ) => {
            $(#[$meta])*
            pub fn $name(&mut self, language: Language, params: $param_ty) {
                self.$field = None;
                self.last_error = None;
                match self.host.$host_method(language, params) {
                    Ok(response) => self.$field = Some(response),
                    Err(error) => self.last_error = Some(error),
                }
            }
        };
    }

    /// Builds a world populated with the supplied stub servers.
    #[must_use]
    pub fn new(configs: Vec<TestServerConfig>, overrides: weaver_config::CapabilityMatrix) -> Self {
        let mut world = Self {
            configs,
            host: LspHost::new(overrides.clone()),
            handles: HashMap::new(),
            last_error: None,
            last_definition: None,
            last_references: None,
            last_diagnostics: None,
            last_capabilities: None,
        };
        world.rebuild_host(overrides);
        world
    }

    /// Returns the recorded call sequence for the specified language.
    pub fn calls(&self, language: Language) -> Option<Vec<CallKind>> {
        self.handles
            .get(&language)
            .map(RecordingServerHandle::calls)
    }

    /// Initialises the server for the language and stores the outcome.
    pub fn initialize(&mut self, language: Language) {
        self.last_capabilities = None;
        self.last_error = None;
        match self.host.initialize(language) {
            Ok(summary) => self.last_capabilities = Some(summary),
            Err(error) => self.last_error = Some(error),
        }
    }

    request_method!(
        /// Issues a definition request.
        request_definition,
        goto_definition,
        lsp_types::GotoDefinitionParams,
        last_definition,
        GotoDefinitionResponse
    );

    request_method!(
        /// Issues a references request.
        request_references,
        references,
        lsp_types::ReferenceParams,
        last_references,
        Vec<Location>
    );

    /// Issues a diagnostics request.
    pub fn request_diagnostics(&mut self, language: Language, uri: Uri) {
        self.last_diagnostics = None;
        self.last_error = None;
        match self.host.diagnostics(language, uri) {
            Ok(response) => self.last_diagnostics = Some(response),
            Err(error) => self.last_error = Some(error),
        }
    }

    /// Rebuilds the host using the stored server configs and supplied overrides.
    pub fn rebuild_host(&mut self, overrides: weaver_config::CapabilityMatrix) {
        self.host = LspHost::new(overrides);
        self.handles.clear();
        self.last_error = None;
        self.last_definition = None;
        self.last_references = None;
        self.last_diagnostics = None;
        self.last_capabilities = None;

        for config in &self.configs {
            let server = match &config.initialisation_error {
                Some(message) => RecordingLanguageServer::failing_initialize(
                    config.capabilities,
                    message.clone(),
                ),
                None => RecordingLanguageServer::new(
                    config.capabilities,
                    config.responses.clone(),
                ),
            };

            let handle = server.handle();
            if let Err(error) = self.host.register_language(config.language, Box::new(server)) {
                panic!(
                    "failed to register stub server for {}: {}",
                    config.language, error
                );
            }
            self.handles.insert(config.language, handle);
        }
    }
}
