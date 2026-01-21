//! Response types for observe domain operations.
//!
//! This module provides serializable response types that transform LSP protocol
//! types into the JSON format documented in the users guide.

use lsp_types::{GotoDefinitionResponse, Location, LocationLink};
use serde::Serialize;

/// A definition location in the response format.
///
/// Serializes to the format documented in `docs/users-guide.md`:
///
/// ```json
/// {"uri":"file:///path.rs","line":42,"column":17}
/// ```
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DefinitionLocation {
    /// The document URI containing the definition.
    pub uri: String,
    /// Line number (1-indexed).
    pub line: u32,
    /// Column number (1-indexed).
    pub column: u32,
}

impl From<&Location> for DefinitionLocation {
    fn from(loc: &Location) -> Self {
        Self {
            uri: loc.uri.to_string(),
            // Convert from 0-indexed (LSP) to 1-indexed (user-facing)
            line: loc.range.start.line.saturating_add(1),
            column: loc.range.start.character.saturating_add(1),
        }
    }
}

impl From<&LocationLink> for DefinitionLocation {
    fn from(link: &LocationLink) -> Self {
        Self {
            uri: link.target_uri.to_string(),
            // Use selection range for precise definition location
            line: link.target_selection_range.start.line.saturating_add(1),
            column: link
                .target_selection_range
                .start
                .character
                .saturating_add(1),
        }
    }
}

/// Extracts definition locations from an LSP `GotoDefinitionResponse`.
///
/// Handles all response variants: scalar location, array of locations, and
/// location links. Returns an empty vector if no definitions were found.
#[must_use]
pub fn extract_locations(response: GotoDefinitionResponse) -> Vec<DefinitionLocation> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![DefinitionLocation::from(&loc)],
        GotoDefinitionResponse::Array(locs) => locs.iter().map(DefinitionLocation::from).collect(),
        GotoDefinitionResponse::Link(links) => links.iter().map(DefinitionLocation::from).collect(),
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range, Uri};
    use rstest::{fixture, rstest};

    use super::*;

    #[fixture]
    fn sample_uri() -> Uri {
        "file:///src/main.rs".parse().expect("valid uri")
    }

    fn make_location(uri: &Uri, line: u32, character: u32) -> Location {
        Location {
            uri: uri.clone(),
            range: Range {
                start: Position { line, character },
                end: Position {
                    line,
                    character: character + 5,
                },
            },
        }
    }

    fn make_location_link(uri: &Uri, line: u32, character: u32) -> LocationLink {
        LocationLink {
            origin_selection_range: None,
            target_uri: uri.clone(),
            target_range: Range {
                start: Position { line, character },
                end: Position {
                    line,
                    character: character + 10,
                },
            },
            target_selection_range: Range {
                start: Position { line, character },
                end: Position {
                    line,
                    character: character + 5,
                },
            },
        }
    }

    /// Response variant for parameterised extraction tests.
    enum ResponseVariant {
        Scalar { line: u32, character: u32 },
        Array { positions: Vec<(u32, u32)> },
        Link { line: u32, character: u32 },
        MultiLink { positions: Vec<(u32, u32)> },
    }

    fn build_response(uri: &Uri, variant: ResponseVariant) -> GotoDefinitionResponse {
        match variant {
            ResponseVariant::Scalar { line, character } => {
                GotoDefinitionResponse::Scalar(make_location(uri, line, character))
            }
            ResponseVariant::Array { positions } => GotoDefinitionResponse::Array(
                positions
                    .into_iter()
                    .map(|(line, character)| make_location(uri, line, character))
                    .collect(),
            ),
            ResponseVariant::Link { line, character } => {
                GotoDefinitionResponse::Link(vec![make_location_link(uri, line, character)])
            }
            ResponseVariant::MultiLink { positions } => GotoDefinitionResponse::Link(
                positions
                    .into_iter()
                    .map(|(line, character)| make_location_link(uri, line, character))
                    .collect(),
            ),
        }
    }

    #[rstest]
    #[case::scalar(
        ResponseVariant::Scalar { line: 9, character: 4 },
        &[(10, 5)]  // 0-indexed (9, 4) -> 1-indexed (10, 5)
    )]
    #[case::array(
        ResponseVariant::Array { positions: vec![(0, 0), (41, 16)] },
        &[(1, 1), (42, 17)]
    )]
    #[case::link(
        ResponseVariant::Link { line: 99, character: 9 },
        &[(100, 10)]
    )]
    #[case::multi_link(
        ResponseVariant::MultiLink { positions: vec![(4, 7), (19, 0), (50, 12)] },
        &[(5, 8), (20, 1), (51, 13)]
    )]
    fn extracts_locations_from_response_variants(
        sample_uri: Uri,
        #[case] variant: ResponseVariant,
        #[case] expected: &[(u32, u32)],
    ) {
        let response = build_response(&sample_uri, variant);
        let locations = extract_locations(response);

        assert_eq!(
            locations.len(),
            expected.len(),
            "expected {} locations, got {}",
            expected.len(),
            locations.len()
        );
        let expected_uri = sample_uri.to_string();
        for (i, (expected_line, expected_column)) in expected.iter().enumerate() {
            assert_eq!(locations[i].uri, expected_uri, "location[{i}].uri mismatch");
            assert_eq!(
                locations[i].line, *expected_line,
                "location[{i}].line mismatch"
            );
            assert_eq!(
                locations[i].column, *expected_column,
                "location[{i}].column mismatch"
            );
        }
    }

    #[test]
    fn handles_empty_array() {
        let response = GotoDefinitionResponse::Array(vec![]);
        let locations = extract_locations(response);

        assert!(locations.is_empty());
    }

    #[test]
    fn serializes_to_expected_json() {
        let location = DefinitionLocation {
            uri: "file:///path.rs".to_string(),
            line: 42,
            column: 17,
        };

        let json = serde_json::to_string(&location).expect("serialize");
        assert!(json.contains(r#""uri":"file:///path.rs""#));
        assert!(json.contains(r#""line":42"#));
        assert!(json.contains(r#""column":17"#));
    }
}
