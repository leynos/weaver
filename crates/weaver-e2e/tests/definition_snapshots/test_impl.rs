//! Shared assertion helpers for definition snapshot entry points.

use std::path::Path;

use insta::assert_debug_snapshot;

use super::{DefinitionSnapshot, TestContext, TestError};

fn assert_definition_snapshot(name: &str, snapshot: &DefinitionSnapshot) {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots"
    )));
    settings.bind(|| assert_debug_snapshot!(name, snapshot));
}

/// Gets definition at the given position and returns a snapshot.
fn get_definition_snapshot(
    ctx: &mut TestContext,
    line: u32,
    character: u32,
) -> Result<DefinitionSnapshot, TestError> {
    let response = ctx
        .client
        .goto_definition_at(&ctx.file_uri, line, character)?;
    Ok(DefinitionSnapshot::from(response))
}

/// Tests definition lookup from a function call to its definition.
pub fn definition_from_call_to_function_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In LINEAR_CHAIN: def a() calls b() on line 1, character ~4
    // b() is defined on line 3
    let snapshot = get_definition_snapshot(ctx, 1, 4)?;
    assert_definition_snapshot("definition_from_call_to_function", &snapshot);
    Ok(())
}

/// Tests definition lookup for a function name at its definition site.
pub fn definition_at_function_definition_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In LINEAR_CHAIN: def a() is on line 0, character 4
    let snapshot = get_definition_snapshot(ctx, 0, 4)?;
    assert_definition_snapshot("definition_at_function_definition", &snapshot);
    Ok(())
}

/// Tests definition lookup for a method call on self.
pub fn definition_self_method_call_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In PYTHON_CLASS: self.validate() is called on line 2
    // validate is defined on line 5
    let snapshot = get_definition_snapshot(ctx, 2, 25)?;
    assert_definition_snapshot("definition_self_method_call", &snapshot);
    Ok(())
}

/// Tests definition lookup for a class method definition.
pub fn definition_class_method_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In PYTHON_CLASS: def process(self, data) on line 1
    let snapshot = get_definition_snapshot(ctx, 1, 8)?;
    assert_definition_snapshot("definition_class_method", &snapshot);
    Ok(())
}

/// Tests definition lookup for the class name.
pub fn definition_class_name_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In PYTHON_CLASS: class Service on line 0
    let snapshot = get_definition_snapshot(ctx, 0, 6)?;
    assert_definition_snapshot("definition_class_name", &snapshot);
    Ok(())
}

/// Tests definition lookup for a parameter.
pub fn definition_parameter_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // In PYTHON_FUNCTIONS: def greet(name) - name parameter on line 0
    let snapshot = get_definition_snapshot(ctx, 0, 10)?;
    assert_definition_snapshot("definition_parameter", &snapshot);
    Ok(())
}

/// Tests definition lookup on whitespace (should return None).
pub fn definition_on_whitespace_impl(ctx: &mut TestContext) -> Result<(), TestError> {
    // Position on whitespace/indentation
    let snapshot = get_definition_snapshot(ctx, 1, 0)?;
    assert_definition_snapshot("definition_on_whitespace", &snapshot);
    Ok(())
}
