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
            line: loc.range.start.line + 1,
            column: loc.range.start.character + 1,
        }
    }
}

impl From<&LocationLink> for DefinitionLocation {
    fn from(link: &LocationLink) -> Self {
        Self {
            uri: link.target_uri.to_string(),
            // Use selection range for precise definition location
            line: link.target_selection_range.start.line + 1,
            column: link.target_selection_range.start.character + 1,
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

    use super::*;

    fn sample_uri() -> Uri {
        "file:///src/main.rs".parse().expect("valid uri")
    }

    fn make_location(line: u32, character: u32) -> Location {
        Location {
            uri: sample_uri(),
            range: Range {
                start: Position { line, character },
                end: Position {
                    line,
                    character: character + 5,
                },
            },
        }
    }

    fn make_location_link(line: u32, character: u32) -> LocationLink {
        LocationLink {
            origin_selection_range: None,
            target_uri: sample_uri(),
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

    #[test]
    fn extracts_scalar_location() {
        let response = GotoDefinitionResponse::Scalar(make_location(9, 4));
        let locations = extract_locations(response);

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, "file:///src/main.rs");
        assert_eq!(locations[0].line, 10); // 9 + 1
        assert_eq!(locations[0].column, 5); // 4 + 1
    }

    #[test]
    fn extracts_array_of_locations() {
        let response =
            GotoDefinitionResponse::Array(vec![make_location(0, 0), make_location(41, 16)]);
        let locations = extract_locations(response);

        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].line, 1);
        assert_eq!(locations[0].column, 1);
        assert_eq!(locations[1].line, 42);
        assert_eq!(locations[1].column, 17);
    }

    #[test]
    fn extracts_location_links() {
        let response = GotoDefinitionResponse::Link(vec![make_location_link(99, 9)]);
        let locations = extract_locations(response);

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 100);
        assert_eq!(locations[0].column, 10);
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
