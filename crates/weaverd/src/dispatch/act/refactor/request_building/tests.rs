//! Unit tests for request-building internals.

use rstest::rstest;
use serde_json::Value;

use super::*;

/// Wraps `offset` in the plugin-argument map these cases exercise.
fn offset_args(offset: Value) -> HashMap<String, Value> {
    HashMap::from([(String::from("offset"), offset)])
}

fn rename_mapping_context<'a>() -> CapabilityMappingContext<'a> {
    CapabilityMappingContext {
        capability: CapabilityId::RenameSymbol,
        file_path: Path::new("/tmp"),
        file_content: "hello world",
        position: None,
        metrics: &crate::dispatch::act::refactor::metrics::NullPositionMetrics,
    }
}

#[rstest]
#[case::padded_string(Value::String(String::from(" 004 ")))]
#[case::numeric(Value::Number(serde_json::Number::from(4)))]
fn apply_rename_symbol_mapping_normalizes_deprecated_offset(#[case] offset: Value) {
    let mut plugin_args = offset_args(offset);

    apply_rename_symbol_mapping(&mut plugin_args, &rename_mapping_context())
        .expect("offset should map to position");

    assert_eq!(
        plugin_args.get("position").and_then(Value::as_str),
        Some("4")
    );
    assert!(!plugin_args.contains_key("offset"));
}

#[rstest]
#[case::negative(Value::String(String::from("-1")))]
#[case::non_numeric(Value::String(String::from("abc")))]
#[case::not_a_string_or_number(Value::Bool(false))]
fn apply_rename_symbol_mapping_rejects_invalid_deprecated_offset(#[case] offset: Value) {
    let mut plugin_args = offset_args(offset);

    let err = apply_rename_symbol_mapping(&mut plugin_args, &rename_mapping_context())
        .expect_err("invalid offset must be rejected");

    assert_invalid_offset_error(err);
}

fn assert_invalid_offset_error(err: DispatchError) {
    assert!(matches!(err, DispatchError::InvalidArguments { .. }));
    let DispatchError::InvalidArguments {
        message: invalid_arguments,
    } = err
    else {
        return;
    };
    assert!(invalid_arguments.contains("must be a numeric or string byte offset"));
    assert!(invalid_arguments.contains("/tmp"));
}
