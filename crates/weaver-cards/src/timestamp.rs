//! Timestamp formatting helpers for symbol-card provenance.

use std::time::SystemTime;

use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_TIMESTAMP: &str = "1970-01-01T00:00:00Z";

/// Formats the current UTC timestamp as ISO 8601 / RFC 3339.
#[must_use]
pub fn extraction_timestamp_now() -> String {
    OffsetDateTime::from(SystemTime::now())
        .format(&Iso8601::DEFAULT)
        .unwrap_or_else(|_| String::from(FALLBACK_TIMESTAMP))
}
