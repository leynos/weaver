//! Invalid-argument coverage for the `observe graph-slice` dispatch handler.

use rstest::rstest;
use tempfile::TempDir;
use url::Url;

use super::{
    FusionBackends,
    SemanticBackendProvider,
    support::{backends_fixture, make_request},
};
use crate::dispatch::{
    errors::DispatchError,
    observe::graph_slice::handle,
    response::ResponseWriter,
};

/// Asserts a dispatch attempt failed with an `InvalidArguments` error whose
/// message mentions every expected fragment.
macro_rules! assert_invalid_arguments {
    ($result:expr, $($expected:expr),+ $(,)?) => {{
        match $result {
            Ok(_) => panic!("expected invalid arguments error, dispatch succeeded"),
            Err(DispatchError::InvalidArguments { message }) => {
                $(
                    assert!(
                        message.contains($expected),
                        "expected invalid-arguments message to contain {:?}, got: {message}",
                        $expected
                    );
                )+
            }
            Err(other) => panic!("expected invalid arguments error, got: {other:?}"),
        }
    }};
}

#[rstest]
#[case(&["--position", "10:5"], "missing required argument: --uri")]
#[case(
    &["--uri", "file:///src/main.rs", "--position", "bad"],
    "invalid argument value for --position"
)]
#[case(
    &["--uri", "https://example.com/main.rs", "--position", "1:1"],
    "expected a file URI"
)]
#[case(
    &["--uri", "file:///src/main.rs", "--position", "1:1", "--max-cards", "0"],
    "--max-cards must be >= 1"
)]
#[case(&["--uri", "file://%zz", "--position", "1:1"], "invalid URI")]
fn invalid_arguments_return_dispatch_error(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
    #[case] arguments: &[&str],
    #[case] expected_substring: &str,
) -> Result<(), String> {
    let (mut backends, _temp_dir) = backends_fixture?;
    let request = make_request(arguments)?;
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);

    let result = handle(&request, &mut writer, &mut backends);

    assert_invalid_arguments!(result, expected_substring);
    Ok(())
}

#[rstest]
fn missing_source_file_returns_invalid_arguments(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let path = temp_dir.path().join("missing.rs");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&["--uri", &uri, "--position", "1:1"])?;
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);

    let result = handle(&request, &mut writer, &mut backends);

    assert_invalid_arguments!(result, "unable to read source file", "missing.rs");
    Ok(())
}
