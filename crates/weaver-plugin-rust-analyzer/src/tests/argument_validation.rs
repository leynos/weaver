//! Argument-validation tests for rust-analyzer plugin requests.

use std::collections::HashMap;

use rstest::rstest;
use weaver_plugins::capability::ReasonCode;

use super::support::{adapter_returning, adapter_unused, rename_arguments, request_with_args};
use crate::execute_request;

fn remove_uri(arguments: &mut HashMap<String, serde_json::Value>) { arguments.remove("uri"); }

fn set_empty_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::from("  ")),
    );
}

fn set_numeric_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("uri"),
        serde_json::Value::Number(serde_json::Number::from(4)),
    );
}

fn remove_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.remove("position");
}

fn set_boolean_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(String::from("position"), serde_json::Value::Bool(true));
}

fn set_negative_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("position"),
        serde_json::Value::String(String::from("-1")),
    );
}

fn set_numeric_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("position"),
        serde_json::Value::Number(serde_json::Number::from(3)),
    );
}

fn set_empty_new_name(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("  ")),
    );
}

fn set_numeric_new_name(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::Number(serde_json::Number::from(42)),
    );
}

fn remove_new_name(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.remove("new_name");
}

#[rstest]
#[case::missing_uri(remove_uri as fn(&mut _), Some("uri"))]
#[case::empty_uri(set_empty_uri as fn(&mut _), Some("uri"))]
#[case::numeric_uri(set_numeric_uri as fn(&mut _), Some("uri argument must be a string"))]
#[case::missing_position(remove_position as fn(&mut _), Some("position"))]
#[case::boolean_position(set_boolean_position as fn(&mut _), Some("position"))]
#[case::negative_position(set_negative_position as fn(&mut _), Some("non-negative integer"))]
#[case::numeric_position_succeeds(set_numeric_position as fn(&mut _), None)]
#[case::missing_new_name(remove_new_name as fn(&mut _), Some("new_name"))]
#[case::numeric_new_name(set_numeric_new_name as fn(&mut _), Some("new_name argument must be a string"))]
#[case::empty_new_name(set_empty_new_name as fn(&mut _), Some("new_name"))]
fn rename_argument_validation(
    #[case] mutate: fn(&mut HashMap<String, serde_json::Value>),
    #[case] expected_error: Option<&str>,
) {
    let mut arguments = rename_arguments();
    mutate(&mut arguments);

    if let Some(needle) = expected_error {
        let adapter = adapter_unused();
        let err = execute_request(&adapter, &request_with_args(arguments))
            .expect_err("invalid arguments should fail");
        assert!(
            err.message().contains(needle),
            "expected error mentioning '{needle}', got: {err}"
        );
        assert_eq!(err.reason_code(), Some(ReasonCode::IncompletePayload));
    } else {
        let adapter = adapter_returning(Ok(String::from("fn new_name() -> i32 {\n    1\n}\n")));
        let response = execute_request(&adapter, &request_with_args(arguments))
            .expect("valid arguments should succeed");
        assert!(response.is_success());
    }
}
