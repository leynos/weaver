//! Response types for the `observe get-card` operation.
//!
//! The response is either a successfully constructed [`SymbolCard`] wrapped
//! in [`GetCardResponse::Success`], or a structured refusal in
//! [`GetCardResponse::Refusal`] explaining why a card could not be
//! produced.

use serde::{Deserialize, Serialize};

use crate::{DetailLevel, SymbolCard};

/// Reason why a card could not be produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RefusalReason {
    /// No symbol found at the requested position.
    NoSymbolAtPosition,
    /// The requested position is outside the file bounds.
    PositionOutOfRange,
    /// The requested language is not supported.
    UnsupportedLanguage,
    /// The operation is not yet implemented.
    NotYetImplemented,
    /// The requested detail level requires a backend that is unavailable.
    BackendUnavailable,
}

/// Structured refusal payload returned when a card cannot be produced.
///
/// # Example
///
/// ```
/// use weaver_cards::{CardRefusal, DetailLevel, RefusalReason};
///
/// let refusal = CardRefusal {
///     reason: RefusalReason::NotYetImplemented,
///     message: String::from("not yet implemented"),
///     requested_detail: DetailLevel::Structure,
/// };
/// assert_eq!(refusal.reason, RefusalReason::NotYetImplemented);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRefusal {
    /// Machine-readable reason code.
    pub reason: RefusalReason,
    /// Human-readable explanation.
    pub message: String,
    /// The detail level that was requested.
    pub requested_detail: DetailLevel,
}

/// Response from the `observe get-card` operation.
///
/// Either a successfully constructed card or a structured refusal
/// explaining why a card could not be produced.
///
/// # Example
///
/// ```
/// use weaver_cards::{DetailLevel, GetCardResponse};
///
/// let response = GetCardResponse::not_yet_implemented(DetailLevel::Structure);
/// assert!(matches!(response, GetCardResponse::Refusal { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GetCardResponse {
    /// A card was successfully constructed.
    Success {
        /// The symbol card.
        card: Box<SymbolCard>,
    },
    /// A card could not be constructed.
    Refusal {
        /// Structured refusal with reason and message.
        refusal: CardRefusal,
    },
}

impl GetCardResponse {
    /// Creates a refusal response indicating that card extraction is not
    /// yet implemented.
    #[must_use]
    pub fn not_yet_implemented(detail: DetailLevel) -> Self {
        Self::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::NotYetImplemented,
                message: String::from(
                    "observe get-card: Tree-sitter card extraction is not yet implemented",
                ),
                requested_detail: detail,
            },
        }
    }
}
