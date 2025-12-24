//! Capability modelling and resolution.

use std::collections::BTreeMap;
use std::fmt;

use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::language::Language;
use crate::server::ServerCapabilitySet;

/// LSP feature exposed through the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CapabilityKind {
    /// `textDocument/definition`.
    Definition,
    /// `textDocument/references`.
    References,
    /// Diagnostics for a document.
    Diagnostics,
    /// `textDocument/prepareCallHierarchy` and related requests.
    CallHierarchy,
}

impl CapabilityKind {
    /// Returns the capability key used for overrides.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Definition => "observe.get-definition",
            Self::References => "observe.find-references",
            Self::Diagnostics => "verify.diagnostics",
            Self::CallHierarchy => "observe.call-hierarchy",
        }
    }
}

/// Provenance for a capability's availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySource {
    /// Provided directly by the language server.
    ServerAdvertised,
    /// Enabled by a force override.
    ForcedOverride,
    /// Disabled by an explicit deny override.
    DeniedOverride,
    /// Unavailable because the server does not support it.
    MissingOnServer,
}

impl fmt::Display for CapabilitySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ServerAdvertised => "advertised by server",
            Self::ForcedOverride => "forced by override",
            Self::DeniedOverride => "denied by override",
            Self::MissingOnServer => "missing from server",
        };
        formatter.write_str(label)
    }
}

/// Effective state for a single capability after negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityState {
    /// Capability kind being described.
    pub kind: CapabilityKind,
    /// Whether the capability is usable.
    pub enabled: bool,
    /// Why the capability is (un)available.
    pub source: CapabilitySource,
}

impl CapabilityState {
    /// Constructs a new capability state.
    #[must_use]
    pub fn new(kind: CapabilityKind, enabled: bool, source: CapabilitySource) -> Self {
        Self {
            kind,
            enabled,
            source,
        }
    }
}

/// Capability summary for a single language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySummary {
    language: Language,
    states: BTreeMap<CapabilityKind, CapabilityState>,
}

impl CapabilitySummary {
    /// Returns the language associated with this summary.
    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Returns the state for the requested capability.
    #[must_use]
    pub fn state(&self, capability: CapabilityKind) -> CapabilityState {
        match self.states.get(&capability) {
            Some(state) => *state,
            None => CapabilityState::new(capability, false, CapabilitySource::MissingOnServer),
        }
    }

    /// Returns an iterator over all resolved capability states.
    pub fn states(&self) -> impl Iterator<Item = CapabilityState> + '_ {
        self.states.values().copied()
    }
}

/// Resolves capability availability for a language using server data and overrides.
pub(crate) fn resolve_capabilities(
    language: Language,
    advertised: ServerCapabilitySet,
    overrides: &CapabilityMatrix,
) -> CapabilitySummary {
    let mut states = BTreeMap::new();
    for capability in [
        CapabilityKind::Definition,
        CapabilityKind::References,
        CapabilityKind::Diagnostics,
        CapabilityKind::CallHierarchy,
    ] {
        let state = resolve_state(language, capability, advertised, overrides);
        states.insert(capability, state);
    }
    CapabilitySummary { language, states }
}

fn resolve_state(
    language: Language,
    capability: CapabilityKind,
    advertised: ServerCapabilitySet,
    overrides: &CapabilityMatrix,
) -> CapabilityState {
    match overrides.override_for(language.as_str(), capability.key()) {
        Some(CapabilityOverride::Force) => {
            return CapabilityState::new(capability, true, CapabilitySource::ForcedOverride);
        }
        Some(CapabilityOverride::Deny) => {
            return CapabilityState::new(capability, false, CapabilitySource::DeniedOverride);
        }
        None | Some(CapabilityOverride::Allow) => {}
    }

    let (available, source) = match capability {
        CapabilityKind::Definition => {
            let available = advertised.supports_definition();
            (available, capability_source(available))
        }
        CapabilityKind::References => {
            let available = advertised.supports_references();
            (available, capability_source(available))
        }
        CapabilityKind::Diagnostics => {
            let available = advertised.supports_diagnostics();
            (available, capability_source(available))
        }
        CapabilityKind::CallHierarchy => {
            let available = advertised.supports_call_hierarchy();
            (available, capability_source(available))
        }
    };

    CapabilityState::new(capability, available, source)
}

fn capability_source(available: bool) -> CapabilitySource {
    if available {
        CapabilitySource::ServerAdvertised
    } else {
        CapabilitySource::MissingOnServer
    }
}
