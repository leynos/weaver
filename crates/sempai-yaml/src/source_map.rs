//! Source-location helpers for YAML parsing diagnostics.

use saphyr::{LoadableYamlNode, MarkedYamlOwned, YamlDataOwned};
use serde_saphyr::Location;

use sempai_core::SourceSpan;

/// Retains coarse source locations from the raw YAML document.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    source_uri: Option<String>,
    root_span: Option<SourceSpan>,
    rules_span: Option<SourceSpan>,
    rule_spans: Vec<SourceSpan>,
}

impl SourceMap {
    /// Builds a source map from the raw YAML text.
    #[must_use]
    pub fn parse(yaml: &str, source_uri: Option<String>) -> Self {
        let Ok(documents) = MarkedYamlOwned::load_from_str(yaml) else {
            return Self {
                source_uri,
                root_span: None,
                rules_span: None,
                rule_spans: Vec::new(),
            };
        };

        let Some(document) = documents.first() else {
            return Self {
                source_uri,
                root_span: None,
                rules_span: None,
                rule_spans: Vec::new(),
            };
        };

        let root_span = source_span_for_node(yaml, document, source_uri.as_deref());
        let rules_opt = document.data.as_mapping_get("rules");
        let rules_span =
            rules_opt.and_then(|rules| source_span_for_node(yaml, rules, source_uri.as_deref()));
        let rule_spans = rules_opt
            .and_then(|rules| match &rules.data {
                YamlDataOwned::Sequence(items) => Some(
                    items
                        .iter()
                        .filter_map(|item| source_span_for_node(yaml, item, source_uri.as_deref()))
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        Self {
            source_uri,
            root_span,
            rules_span,
            rule_spans,
        }
    }

    /// Returns the whole-document span when known.
    #[must_use]
    pub const fn root_span(&self) -> Option<&SourceSpan> {
        self.root_span.as_ref()
    }

    /// Returns the top-level `rules` span when known.
    #[must_use]
    pub const fn rules_span(&self) -> Option<&SourceSpan> {
        self.rules_span.as_ref()
    }

    /// Returns the span of the indexed rule object when known.
    #[must_use]
    pub fn rule_span(&self, index: usize) -> Option<&SourceSpan> {
        self.rule_spans.get(index)
    }

    /// Converts a `serde-saphyr` location into a diagnostic span.
    #[must_use]
    pub fn span_from_location(&self, location: Option<Location>) -> Option<SourceSpan> {
        let serde_location = location?;
        let span = serde_location.span();
        let start = u32::try_from(span.byte_offset()?).ok()?;
        let len = u32::try_from(span.byte_len().unwrap_or(1)).ok()?;
        let end = start.saturating_add(len.max(1));
        Some(SourceSpan::new(start, end, self.source_uri.clone()))
    }
}

fn source_span_for_node(
    yaml: &str,
    node: &MarkedYamlOwned,
    source_uri: Option<&str>,
) -> Option<SourceSpan> {
    let start_offset = char_index_to_byte(yaml, node.span.start.index())?;
    let end_offset = char_index_to_byte(yaml, node.span.end.index())?;
    let start = u32::try_from(start_offset).ok()?;
    let end = u32::try_from(end_offset.max(start_offset)).ok()?;
    Some(SourceSpan::new(
        start,
        end.max(start.saturating_add(1)),
        source_uri.map(ToOwned::to_owned),
    ))
}

fn char_index_to_byte(source: &str, index: usize) -> Option<usize> {
    if index == 0 {
        return Some(0);
    }

    source
        .char_indices()
        .nth(index)
        .map(|(offset, _)| offset)
        .or(Some(source.len()))
}
