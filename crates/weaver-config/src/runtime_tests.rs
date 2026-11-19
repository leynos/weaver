//! Unit tests for runtime path derivation from configuration.
//!
//! The cases pin the legacy filenames for the default endpoint, the SHA-256
//! identifier that scopes every other endpoint, and the property that distinct
//! endpoints sharing a runtime directory never contend for the same files.

use proptest::prelude::*;

use super::*;

#[test]
fn default_endpoint_preserves_legacy_runtime_file_names() -> Result<(), RuntimePathsError> {
    let paths = RuntimePaths::from_config_readonly(&Config::default())?;

    assert_eq!(paths.lock_file_name(), Path::new("weaverd.lock"));
    assert_eq!(paths.pid_file_name(), Path::new("weaverd.pid"));
    assert_eq!(paths.health_file_name(), Path::new("weaverd.health"));
    Ok(())
}

#[test]
fn unix_socket_paths_with_one_parent_have_distinct_runtime_files() -> Result<(), RuntimePathsError>
{
    let first = runtime_paths(SocketEndpoint::unix("/tmp/weaver/first.sock"))?;
    let second = runtime_paths(SocketEndpoint::unix("/tmp/weaver/second.sock"))?;

    assert_eq!(first.runtime_dir(), second.runtime_dir());
    assert_distinct_runtime_files(&first, &second);
    Ok(())
}

#[test]
fn tcp_endpoints_with_different_hosts_have_distinct_runtime_files() -> Result<(), RuntimePathsError>
{
    let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
    let second = runtime_paths(SocketEndpoint::tcp("127.0.0.2", 9000))?;

    assert_eq!(first.runtime_dir(), second.runtime_dir());
    assert_distinct_runtime_files(&first, &second);
    Ok(())
}

#[test]
fn tcp_endpoints_with_different_ports_have_distinct_runtime_files() -> Result<(), RuntimePathsError>
{
    let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
    let second = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9001))?;

    assert_eq!(first.runtime_dir(), second.runtime_dir());
    assert_distinct_runtime_files(&first, &second);
    Ok(())
}

#[test]
fn identical_endpoints_have_identical_runtime_files() -> Result<(), RuntimePathsError> {
    let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
    let second = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;

    assert_eq!(first.lock_path(), second.lock_path());
    assert_eq!(first.pid_path(), second.pid_path());
    assert_eq!(first.health_path(), second.health_path());
    Ok(())
}

proptest! {
    #[test]
    fn unix_endpoint_runtime_paths_are_deterministic_and_contained(segment in any::<u32>()) {
        let endpoint = SocketEndpoint::unix(format!("/tmp/weaver/socket-{segment}.sock"));
        let first = runtime_paths(endpoint.clone())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let second = runtime_paths(endpoint)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(first.lock_path(), second.lock_path());
        prop_assert_eq!(first.pid_path(), second.pid_path());
        prop_assert_eq!(first.health_path(), second.health_path());
        assert_runtime_paths_are_contained(&first);
    }

    #[test]
    fn tcp_endpoint_runtime_paths_are_deterministic_and_contained(
        host_suffix in "[a-z]{1,12}",
        port in any::<u16>(),
    ) {
        let endpoint = SocketEndpoint::tcp(format!("host-{host_suffix}.example"), port);
        let first = runtime_paths(endpoint.clone())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let second = runtime_paths(endpoint)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(first.lock_path(), second.lock_path());
        prop_assert_eq!(first.pid_path(), second.pid_path());
        prop_assert_eq!(first.health_path(), second.health_path());
        assert_runtime_paths_are_contained(&first);
    }

    #[test]
    fn distinct_unix_endpoints_in_one_directory_get_distinct_runtime_files(
        first_name in "[a-z]{1,12}",
        second_name in "[a-z]{1,12}",
    ) {
        prop_assume!(first_name != second_name);
        let first = runtime_paths(SocketEndpoint::unix(format!(
            "/tmp/weaver/{first_name}.sock"
        )))
        .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let second = runtime_paths(SocketEndpoint::unix(format!(
            "/tmp/weaver/{second_name}.sock"
        )))
        .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(first.runtime_dir(), second.runtime_dir());
        assert_distinct_runtime_file_names(&first, &second);
    }

    #[test]
    fn distinct_tcp_endpoints_get_distinct_runtime_file_names(
        first_host in "[a-z]{1,12}",
        second_host in "[a-z]{1,12}",
        first_port in any::<u16>(),
        second_port in any::<u16>(),
    ) {
        prop_assume!(
            (first_host.as_str(), first_port) != (second_host.as_str(), second_port)
        );
        let first = runtime_paths(SocketEndpoint::tcp(first_host, first_port))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let second = runtime_paths(SocketEndpoint::tcp(second_host, second_port))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        assert_distinct_runtime_file_names(&first, &second);
    }
}

#[test]
fn fixed_unix_endpoint_identifier_matches_sha256() {
    let identifier = endpoint_identifier(&SocketEndpoint::unix("/tmp/weaver/first.sock"));

    assert_eq!(
        identifier,
        "38d86225b8a640437734f953d057dabe3358a6c109ece28c4b8b336830930a76"
    );
    assert_identifier_shape(&identifier);
}

#[test]
fn fixed_tcp_endpoint_identifier_matches_sha256() {
    let identifier = endpoint_identifier(&SocketEndpoint::tcp("127.0.0.1", 9000));

    assert_eq!(
        identifier,
        "841ea0c674a7fb51754036db0122e290da00975656ea8268552e66b4f3ae22ee"
    );
    assert_identifier_shape(&identifier);
}

#[test]
fn endpoint_kind_is_framed_into_the_identifier() {
    let unix = endpoint_identifier(&SocketEndpoint::unix("same"));
    let tcp = endpoint_identifier(&SocketEndpoint::tcp("same", 0));

    assert_eq!(
        unix,
        "b038db41b85427d13aaf6bd92ee7b0770ce17ecf42b6382014eadb5011b527ac"
    );
    assert_eq!(
        tcp,
        "59974ee49ab866c5f5a7feeeb92be96b167c0c82214ce4ae534f772e8f7c53b4"
    );
    assert_ne!(unix, tcp);
}

#[test]
fn derives_paths_for_tcp_socket() {
    let config = Config {
        daemon_socket: SocketEndpoint::tcp("127.0.0.1", 9000),
        ..Config::default()
    };
    let paths = RuntimePaths::from_config_readonly(&config).expect("paths should derive for tcp");
    let tail = paths
        .runtime_dir()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("runtime dir should have trailing component");
    assert!(
        tail == "weaver" || tail.starts_with("uid-"),
        "unexpected runtime tail: {tail}"
    );
    assert!(paths.lock_path().starts_with(paths.runtime_dir()));
    assert!(paths.pid_path().starts_with(paths.runtime_dir()));
    assert!(paths.health_path().starts_with(paths.runtime_dir()));
}

#[test]
fn rejects_unix_socket_without_parent() {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("weaver.sock"),
        ..Config::default()
    };
    let error = RuntimePaths::from_config(&config)
        .expect_err("paths should fail for sockets without parents");
    assert!(matches!(
        error,
        RuntimePathsError::MissingSocketParent { .. }
    ));
}

fn runtime_paths(endpoint: SocketEndpoint) -> Result<RuntimePaths, RuntimePathsError> {
    let config = Config {
        daemon_socket: endpoint,
        ..Config::default()
    };
    RuntimePaths::from_config_readonly(&config)
}

fn assert_distinct_runtime_files(first: &RuntimePaths, second: &RuntimePaths) {
    assert_ne!(first.lock_path(), second.lock_path());
    assert_ne!(first.pid_path(), second.pid_path());
    assert_ne!(first.health_path(), second.health_path());
}

fn assert_distinct_runtime_file_names(first: &RuntimePaths, second: &RuntimePaths) {
    assert_ne!(first.lock_file_name(), second.lock_file_name());
    assert_ne!(first.pid_file_name(), second.pid_file_name());
    assert_ne!(first.health_file_name(), second.health_file_name());
}

fn assert_identifier_shape(identifier: &str) {
    assert_eq!(identifier.len(), 64, "sha-256 hex digest has 64 characters");
    assert!(
        identifier
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "identifier should be lowercase hexadecimal: {identifier}"
    );
}

fn assert_runtime_paths_are_contained(paths: &RuntimePaths) {
    for (name, path, suffix) in [
        (paths.lock_file_name(), paths.lock_path(), ".lock"),
        (paths.pid_file_name(), paths.pid_path(), ".pid"),
        (paths.health_file_name(), paths.health_path(), ".health"),
    ] {
        assert_eq!(name.components().count(), 1);
        assert!(name.to_string_lossy().ends_with(suffix));
        assert!(path.starts_with(paths.runtime_dir()));
    }
}
