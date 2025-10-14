use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use thiserror::Error;

/// Directive applied to a capability during negotiation.
#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, EnumString, Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum CapabilityOverride {
    /// Force the capability to be advertised even when backends decline it.
    Force,
    /// Disable the capability regardless of backend support.
    Deny,
    /// Leave negotiation to backend discovery (default behaviour).
    #[default]
    Allow,
}

/// Errors produced when parsing [`CapabilityDirective`] values.
#[derive(Debug, Error)]
pub enum CapabilityDirectiveParseError {
    /// Language separator (`:`) was missing from the directive.
    #[error("directive '{0}' is missing the language separator ':'")]
    MissingLanguage(String),
    /// Capability override assignment (`=`) was missing from the directive.
    #[error("directive '{0}' is missing the override assignment '='")]
    MissingDirective(String),
    /// The override directive could not be parsed.
    #[error("unsupported capability directive '{0}'")]
    InvalidDirective(String),
}

/// Declarative override for a capability.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CapabilityDirective {
    /// Language identifier such as `rust` or `python`.
    pub language: String,
    /// Capability identifier in dot-separated form.
    pub capability: String,
    /// Override applied to the capability.
    pub directive: CapabilityOverride,
}

impl CapabilityDirective {
    /// Creates a new directive.
    #[must_use]
    pub fn new(
        language: impl Into<String>,
        capability: impl Into<String>,
        directive: CapabilityOverride,
    ) -> Self {
        Self {
            language: language.into(),
            capability: capability.into(),
            directive,
        }
    }
}

impl fmt::Display for CapabilityDirective {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}={}",
            self.language, self.capability, self.directive
        )
    }
}

impl FromStr for CapabilityDirective {
    type Err = CapabilityDirectiveParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (language, rest) = input
            .split_once(':')
            .ok_or_else(|| CapabilityDirectiveParseError::MissingLanguage(input.to_string()))?;
        let (capability, directive) = rest
            .split_once('=')
            .ok_or_else(|| CapabilityDirectiveParseError::MissingDirective(input.to_string()))?;
        let directive = CapabilityOverride::from_str(directive)
            .map_err(|_| CapabilityDirectiveParseError::InvalidDirective(directive.to_string()))?;
        Ok(Self::new(language, capability, directive))
    }
}

/// Set of directives grouped by language and capability.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CapabilityMatrix {
    /// Mapping of language identifiers to their overrides.
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageCapabilities>,
}

impl CapabilityMatrix {
    /// Builds a matrix from an iterator of directives.
    #[must_use]
    pub fn from_directives<'a, I>(directives: I) -> Self
    where
        I: IntoIterator<Item = &'a CapabilityDirective>,
    {
        let mut matrix = Self::default();
        for directive in directives {
            matrix.set_override(
                directive.language.clone(),
                directive.capability.clone(),
                directive.directive,
            );
        }
        matrix
    }

    /// Stores or updates an override for a capability.
    pub fn set_override(
        &mut self,
        language: impl Into<String>,
        capability: impl Into<String>,
        directive: CapabilityOverride,
    ) {
        let language = normalise_key(&language.into());
        let capability = normalise_key(&capability.into());
        let entry = self.languages.entry(language).or_default();
        entry.overrides.insert(capability, directive);
    }

    /// Retrieves an override for a capability, when present.
    #[must_use]
    pub fn override_for(&self, language: &str, capability: &str) -> Option<CapabilityOverride> {
        let language = normalise_key(language);
        let capability = normalise_key(capability);
        self.languages
            .get(&language)
            .and_then(|caps| caps.overrides.get(&capability).copied())
    }
}

/// Capability overrides scoped to a single language.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LanguageCapabilities {
    /// Overrides keyed by fully-qualified capability path.
    #[serde(default)]
    pub overrides: BTreeMap<String, CapabilityOverride>,
}

/// Deduplicates capability directives in-place, keeping the last directive per key.
pub fn deduplicate_directives(directives: &mut Vec<CapabilityDirective>) {
    let mut merged: BTreeMap<(String, String), CapabilityDirective> = BTreeMap::new();
    for mut directive in directives.drain(..) {
        let language = normalise_key(&directive.language);
        let capability = normalise_key(&directive.capability);
        directive.language = language.clone();
        directive.capability = capability.clone();
        merged.insert((language, capability), directive);
    }
    *directives = merged.into_values().collect();
}

fn normalise_key(key: &str) -> String {
    key.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_keys_on_lookup() {
        let mut matrix = CapabilityMatrix::default();
        matrix.set_override("Rust", "Observe.Get-Definition", CapabilityOverride::Force);

        assert_eq!(
            matrix.override_for(" rust ", "observe.get-definition"),
            Some(CapabilityOverride::Force)
        );
    }

    #[test]
    fn deduplicates_directives_preferring_latest() {
        let mut directives = vec![
            CapabilityDirective::new("rust", "observe.rename", CapabilityOverride::Force),
            CapabilityDirective::new("Rust", "observe.rename", CapabilityOverride::Deny),
        ];
        deduplicate_directives(&mut directives);

        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].directive, CapabilityOverride::Deny);
        assert_eq!(directives[0].language, "rust");
        assert_eq!(directives[0].capability, "observe.rename");
    }
}
