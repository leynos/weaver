//! Unit tests for capability contract types and validation.

use std::collections::HashMap;

use rstest::{fixture, rstest};

use crate::capability::reason_code::ReasonCode;
use crate::capability::rename_symbol::{
    RENAME_SYMBOL_CONTRACT_VERSION, RenameSymbolContract, RenameSymbolRequest,
};
use crate::capability::{CapabilityContract, CapabilityId, ContractVersion};
use crate::error::PluginError;
use crate::protocol::{
    DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};

// ---------------------------------------------------------------------------
// CapabilityId
// ---------------------------------------------------------------------------

#[rstest]
#[case::rename_symbol(CapabilityId::RenameSymbol, "rename-symbol")]
#[case::extricate_symbol(CapabilityId::ExtricateSymbol, "extricate-symbol")]
#[case::extract_method(CapabilityId::ExtractMethod, "extract-method")]
#[case::replace_body(CapabilityId::ReplaceBody, "replace-body")]
#[case::extract_predicate(CapabilityId::ExtractPredicate, "extract-predicate")]
fn capability_id_as_str(#[case] id: CapabilityId, #[case] expected: &str) {
    assert_eq!(id.as_str(), expected);
}

#[rstest]
#[case::rename_symbol(CapabilityId::RenameSymbol, "rename-symbol")]
#[case::extricate_symbol(CapabilityId::ExtricateSymbol, "extricate-symbol")]
fn capability_id_display(#[case] id: CapabilityId, #[case] expected: &str) {
    assert_eq!(id.to_string(), expected);
}

#[rstest]
#[case::rename_symbol("\"rename-symbol\"", CapabilityId::RenameSymbol)]
#[case::extricate_symbol("\"extricate-symbol\"", CapabilityId::ExtricateSymbol)]
#[case::extract_method("\"extract-method\"", CapabilityId::ExtractMethod)]
#[case::replace_body("\"replace-body\"", CapabilityId::ReplaceBody)]
#[case::extract_predicate("\"extract-predicate\"", CapabilityId::ExtractPredicate)]
fn capability_id_serde_round_trip(#[case] json: &str, #[case] expected: CapabilityId) {
    let parsed: CapabilityId = serde_json::from_str(json).expect("deserialise");
    assert_eq!(parsed, expected);
    let back = serde_json::to_string(&parsed).expect("serialise");
    assert_eq!(back, json);
}

// ---------------------------------------------------------------------------
// ContractVersion
// ---------------------------------------------------------------------------

#[test]
fn contract_version_accessors() {
    let v = ContractVersion::new(1, 3);
    assert_eq!(v.major(), 1);
    assert_eq!(v.minor(), 3);
}

#[test]
fn contract_version_compatible_same_major() {
    let v1 = ContractVersion::new(1, 0);
    let v2 = ContractVersion::new(1, 5);
    assert!(v1.is_compatible_with(&v2));
    assert!(v2.is_compatible_with(&v1));
}

#[test]
fn contract_version_incompatible_different_major() {
    let v1 = ContractVersion::new(1, 0);
    let v2 = ContractVersion::new(2, 0);
    assert!(!v1.is_compatible_with(&v2));
}

#[test]
fn contract_version_display() {
    let v = ContractVersion::new(1, 2);
    assert_eq!(v.to_string(), "1.2");
}

#[test]
fn contract_version_serde_round_trip() {
    let v = ContractVersion::new(1, 0);
    let json = serde_json::to_string(&v).expect("serialise");
    let back: ContractVersion = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, v);
}

// ---------------------------------------------------------------------------
// RenameSymbolRequest::extract
// ---------------------------------------------------------------------------

fn make_rename_args(
    uri: &str,
    position: &str,
    new_name: &str,
) -> HashMap<String, serde_json::Value> {
    HashMap::from([
        (String::from("uri"), serde_json::Value::String(uri.into())),
        (
            String::from("position"),
            serde_json::Value::String(position.into()),
        ),
        (
            String::from("new_name"),
            serde_json::Value::String(new_name.into()),
        ),
    ])
}

#[test]
fn extract_valid_request_succeeds() {
    let args = make_rename_args("file:///src/main.py", "10:5", "bar");
    let request = PluginRequest::with_arguments("rename-symbol", vec![], args);
    let extracted = RenameSymbolRequest::extract(&request).expect("should succeed");
    assert_eq!(extracted.uri(), "file:///src/main.py");
    assert_eq!(extracted.position(), "10:5");
    assert_eq!(extracted.new_name(), "bar");
}

#[rstest]
#[case::missing_uri("uri")]
#[case::missing_position("position")]
#[case::missing_new_name("new_name")]
fn extract_missing_field_returns_error(#[case] missing_field: &str) {
    let mut args = make_rename_args("file:///src/main.py", "10:5", "bar");
    args.remove(missing_field);
    let request = PluginRequest::with_arguments("rename-symbol", vec![], args);
    let err = RenameSymbolRequest::extract(&request).expect_err("should fail");
    assert!(
        matches!(err, PluginError::InvalidOutput { .. }),
        "expected InvalidOutput, got: {err}",
    );
    assert!(
        err.to_string().contains(missing_field),
        "expected field name '{missing_field}' in error: {err}",
    );
}

#[rstest]
#[case::empty_uri("", "10:5", "bar", "uri")]
#[case::empty_position("file:///src/main.py", "", "bar", "position")]
#[case::empty_new_name("file:///src/main.py", "10:5", "  ", "new_name")]
fn extract_empty_field_returns_error(
    #[case] uri: &str,
    #[case] position: &str,
    #[case] new_name: &str,
    #[case] field: &str,
) {
    let args = make_rename_args(uri, position, new_name);
    let request = PluginRequest::with_arguments("rename-symbol", vec![], args);
    let err = RenameSymbolRequest::extract(&request).expect_err("should fail");
    let msg = err.to_string();
    assert!(msg.contains("non-empty"), "expected 'non-empty' in: {msg}");
    assert!(msg.contains(field), "expected '{field}' in: {msg}");
}

#[test]
fn extract_non_string_field_returns_error() {
    let mut args = make_rename_args("file:///src/main.py", "10:5", "bar");
    args.insert(String::from("uri"), serde_json::Value::Number(42.into()));
    let request = PluginRequest::with_arguments("rename-symbol", vec![], args);
    let err = RenameSymbolRequest::extract(&request).expect_err("should fail");
    assert!(err.to_string().contains("string"));
}

// ---------------------------------------------------------------------------
// RenameSymbolRequest construction
// ---------------------------------------------------------------------------

#[test]
fn rename_symbol_request_accessors() {
    let req = RenameSymbolRequest::new("file:///a.rs", "1:0", "new_fn");
    assert_eq!(req.uri(), "file:///a.rs");
    assert_eq!(req.position(), "1:0");
    assert_eq!(req.new_name(), "new_fn");
}

// ---------------------------------------------------------------------------
// RenameSymbolContract
// ---------------------------------------------------------------------------

#[fixture]
fn contract() -> RenameSymbolContract {
    RenameSymbolContract
}

#[fixture]
fn valid_rename_request() -> PluginRequest {
    let args = make_rename_args("file:///src/main.py", "10:5", "bar");
    PluginRequest::with_arguments("rename-symbol", vec![], args)
}

#[rstest]
fn contract_capability_id(contract: RenameSymbolContract) {
    assert_eq!(contract.capability_id(), CapabilityId::RenameSymbol);
}

#[rstest]
fn contract_version(contract: RenameSymbolContract) {
    assert_eq!(contract.version(), RENAME_SYMBOL_CONTRACT_VERSION);
    assert_eq!(contract.version().major(), 1);
    assert_eq!(contract.version().minor(), 0);
}

#[rstest]
fn contract_validate_valid_request(
    contract: RenameSymbolContract,
    valid_rename_request: PluginRequest,
) {
    assert!(contract.validate_request(&valid_rename_request).is_ok());
}

#[rstest]
fn contract_validate_invalid_request(contract: RenameSymbolContract) {
    let request = PluginRequest::new("rename-symbol", vec![]);
    assert!(contract.validate_request(&request).is_err());
}

#[rstest]
fn contract_validate_wrong_operation_rejects(contract: RenameSymbolContract) {
    let args = make_rename_args("file:///src/main.py", "10:5", "bar");
    let request = PluginRequest::with_arguments("extricate-symbol", vec![], args);
    let err = contract
        .validate_request(&request)
        .expect_err("should reject wrong operation");
    assert!(
        matches!(err, PluginError::InvalidOutput { .. }),
        "expected InvalidOutput, got: {err}",
    );
    assert!(
        err.to_string().contains("expects operation"),
        "expected operation mismatch error, got: {err}",
    );
}

#[rstest]
fn contract_validate_successful_diff_response(contract: RenameSymbolContract) {
    let response = PluginResponse::success(PluginOutput::Diff {
        content: String::from("--- a/f\n+++ b/f\n"),
    });
    assert!(contract.validate_response(&response).is_ok());
}

#[rstest]
#[case::analysis(PluginOutput::Analysis { data: serde_json::json!({}) })]
#[case::empty(PluginOutput::Empty)]
fn contract_validate_successful_non_diff_response_fails(
    contract: RenameSymbolContract,
    #[case] output: PluginOutput,
) {
    let response = PluginResponse::success(output);
    let err = contract
        .validate_response(&response)
        .expect_err("should fail");
    assert!(
        matches!(err, PluginError::InvalidOutput { .. }),
        "expected InvalidOutput, got: {err}",
    );
    assert!(
        err.to_string().contains("diff output"),
        "expected diff output error, got: {err}",
    );
}

#[rstest]
fn contract_validate_failed_response_passes(contract: RenameSymbolContract) {
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found");
    let response = PluginResponse::failure(vec![diag]);
    assert!(contract.validate_response(&response).is_ok());
}

// ---------------------------------------------------------------------------
// ReasonCode
// ---------------------------------------------------------------------------

#[rstest]
#[case::symbol_not_found(ReasonCode::SymbolNotFound, "symbol_not_found")]
#[case::macro_generated(ReasonCode::MacroGenerated, "macro_generated")]
#[case::ambiguous_references(ReasonCode::AmbiguousReferences, "ambiguous_references")]
#[case::unsupported_language(ReasonCode::UnsupportedLanguage, "unsupported_language")]
#[case::incomplete_payload(ReasonCode::IncompletePayload, "incomplete_payload")]
#[case::name_conflict(ReasonCode::NameConflict, "name_conflict")]
#[case::operation_not_supported(ReasonCode::OperationNotSupported, "operation_not_supported")]
fn reason_code_as_str(#[case] code: ReasonCode, #[case] expected: &str) {
    assert_eq!(code.as_str(), expected);
}

#[rstest]
#[case::symbol_not_found(ReasonCode::SymbolNotFound, "symbol_not_found")]
#[case::name_conflict(ReasonCode::NameConflict, "name_conflict")]
fn reason_code_display(#[case] code: ReasonCode, #[case] expected: &str) {
    assert_eq!(code.to_string(), expected);
}

#[rstest]
#[case::symbol_not_found("\"symbol_not_found\"", ReasonCode::SymbolNotFound)]
#[case::macro_generated("\"macro_generated\"", ReasonCode::MacroGenerated)]
#[case::ambiguous_references("\"ambiguous_references\"", ReasonCode::AmbiguousReferences)]
#[case::unsupported_language("\"unsupported_language\"", ReasonCode::UnsupportedLanguage)]
#[case::incomplete_payload("\"incomplete_payload\"", ReasonCode::IncompletePayload)]
#[case::name_conflict("\"name_conflict\"", ReasonCode::NameConflict)]
#[case::operation_not_supported("\"operation_not_supported\"", ReasonCode::OperationNotSupported)]
fn reason_code_serde_round_trip(#[case] json: &str, #[case] expected: ReasonCode) {
    let parsed: ReasonCode = serde_json::from_str(json).expect("deserialise");
    assert_eq!(parsed, expected);
    let back = serde_json::to_string(&parsed).expect("serialise");
    assert_eq!(back, json);
}
