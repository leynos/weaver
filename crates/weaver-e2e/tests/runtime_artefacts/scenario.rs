//! Prepared scenario state: two endpoints, their runtime paths, and binaries.

use std::{
    path::Path,
    process::{Command, Output},
};

use cap_std::{ambient_authority, fs::Dir};
use tempfile::TempDir;
use weaver_config::{RuntimePaths, SocketEndpoint};

use super::{
    artefacts::{
        ScenarioResult,
        expect_success,
        read_pid,
        require_text,
        runtime_paths,
        unix_endpoint,
    },
    weaver_binary::resolve_or_build_weaver_binary_path,
    weaverd_binary::resolve_or_build_weaverd_binary_path,
};

/// Identifies one of the two endpoints exercised by the scenario.
#[derive(Clone, Copy, Debug)]
pub(super) enum Side {
    /// Unix socket `a.sock`.
    A,
    /// Unix socket `b.sock`.
    B,
}

/// A prepared end-to-end scenario with two endpoint-scoped daemons.
pub(super) struct Scenario {
    /// Temporary directory used as the shared runtime directory.
    runtime: TempDir,
    /// Capability handle for the shared runtime directory.
    dir: Dir,
    /// Endpoint configured for the first daemon.
    endpoint_a: SocketEndpoint,
    /// Endpoint configured for the second daemon.
    endpoint_b: SocketEndpoint,
    /// Runtime paths derived from [`Self::endpoint_a`].
    paths_a: RuntimePaths,
    /// Runtime paths derived from [`Self::endpoint_b`].
    paths_b: RuntimePaths,
    /// Resolved `weaver` CLI binary.
    weaver: &'static Path,
    /// Resolved `weaverd` daemon binary.
    daemon: &'static Path,
}

impl Scenario {
    /// Prepares the runtime directory, endpoints, runtime paths, and binaries.
    pub(super) fn prepare() -> ScenarioResult<Self> {
        let runtime =
            TempDir::new().map_err(|source| format!("create temporary directory: {source}"))?;
        let dir = Dir::open_ambient_dir(runtime.path(), ambient_authority()).map_err(|source| {
            format!(
                "open runtime directory {}: {source}",
                runtime.path().display()
            )
        })?;
        let endpoint_a = unix_endpoint(runtime.path(), "a.sock")?;
        let endpoint_b = unix_endpoint(runtime.path(), "b.sock")?;
        let paths_a = runtime_paths(&endpoint_a)?;
        let paths_b = runtime_paths(&endpoint_b)?;
        let weaver = resolve_or_build_weaver_binary_path()
            .map_err(|reason| format!("resolve weaver binary: {reason}"))?;
        let daemon = resolve_or_build_weaverd_binary_path()
            .map_err(|reason| format!("resolve weaverd binary: {reason}"))?;
        Ok(Self {
            runtime,
            dir,
            endpoint_a,
            endpoint_b,
            paths_a,
            paths_b,
            weaver,
            daemon,
        })
    }

    /// Returns the temporary directory that doubles as the runtime directory.
    pub(super) fn runtime_dir(&self) -> &Path { self.runtime.path() }

    /// Returns the capability handle for the runtime directory.
    pub(super) const fn dir(&self) -> &Dir { &self.dir }

    /// Returns the endpoint configured for `side`.
    pub(super) const fn endpoint(&self, side: Side) -> &SocketEndpoint {
        match side {
            Side::A => &self.endpoint_a,
            Side::B => &self.endpoint_b,
        }
    }

    /// Returns the runtime paths derived for `side`.
    pub(super) const fn paths(&self, side: Side) -> &RuntimePaths {
        match side {
            Side::A => &self.paths_a,
            Side::B => &self.paths_b,
        }
    }

    /// Runs the CLI against `side`'s endpoint with `arguments`.
    pub(super) fn run(&self, side: Side, arguments: &[&str]) -> ScenarioResult<Output> {
        let endpoint = self.endpoint(side);
        Command::new(self.weaver)
            .arg("--daemon-socket")
            .arg(endpoint.to_string())
            .args(arguments)
            .env("WEAVERD_BIN", self.daemon)
            .output()
            .map_err(|source| {
                format!(
                    "run `weaver --daemon-socket {endpoint} {}`: {source}",
                    arguments.join(" ")
                )
            })
    }

    /// Starts the daemon for `side` and returns its process ID.
    pub(super) fn start(&self, side: Side) -> ScenarioResult<u32> {
        let output = self.run(side, &["daemon", "start"])?;
        let (stdout, stderr) =
            expect_success(&output, &format!("daemon start for endpoint {side:?}"))?;
        require_text(&stdout, "daemon ready (pid ", "daemon start stdout")?;
        require_text(
            &stderr,
            "runtime artefacts stored under ",
            "daemon start stderr",
        )?;
        read_pid(&self.dir, self.paths(side))
    }

    /// Runs `daemon status` for `side` and returns its combined output.
    pub(super) fn status(&self, side: Side) -> ScenarioResult<(String, String)> {
        let output = self.run(side, &["daemon", "status"])?;
        expect_success(&output, &format!("daemon status for endpoint {side:?}"))
    }
}
