//! Exit-status and refusal helpers for `observe graph-slice`.

use weaver_cards::{
    GraphSliceResponse,
    graph_slice::{SliceRefusal, SliceRefusalReason},
};

pub(super) const GRAPH_SLICE_SCHEMA_VERSION: &str = "graph_slice.v1";

pub(super) fn exit_status(response: &GraphSliceResponse) -> i32 {
    match response {
        GraphSliceResponse::Success { .. } => 0,
        GraphSliceResponse::Refusal { refusal, .. } => match refusal.reason {
            SliceRefusalReason::UnsupportedLanguage => 10,
            SliceRefusalReason::NoSymbolAtPosition => 11,
            SliceRefusalReason::PositionOutOfRange => 12,
            SliceRefusalReason::NotYetImplemented => 13,
            SliceRefusalReason::BackendUnavailable => 14,
            _ => 15,
        },
        _ => 15,
    }
}

pub(super) fn refusal(reason: SliceRefusalReason, message: String) -> GraphSliceResponse {
    GraphSliceResponse::Refusal {
        schema_version: String::from(GRAPH_SLICE_SCHEMA_VERSION),
        refusal: SliceRefusal { reason, message },
    }
}
