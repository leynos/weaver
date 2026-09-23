//! Unit tests for environment snapshot and restoration.

use std::{collections::BTreeMap, process::Command};

use anyhow::{Context, Result, ensure};

use crate::{env_guard::EnvGuard, tests::env::lock_env};

// Observe restoration from a child rather than reading ambient state through
// the environment APIs that the final lint policy forbids in test bodies.
fn observed_environment() -> Result<BTreeMap<String, String>> {
    let output = Command::new("env")
        .output()
        .context("launch env observer")?;
    ensure!(
        output.status.success(),
        "env observer exited unsuccessfully"
    );
    let stdout = String::from_utf8(output.stdout).context("decode env observer output")?;
    Ok(stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

#[test]
fn restores_modified_and_removed_environment_variables() -> Result<()> {
    const EXISTING: &str = "WEAVER_ENV_GUARD_EXISTING";
    const ANOTHER: &str = "WEAVER_ENV_GUARD_ANOTHER";
    const EPHEMERAL: &str = "WEAVER_ENV_GUARD_EPHEMERAL";

    let _guard = lock_env()?;
    temp_env::with_vars(
        [
            (EXISTING, Some("original")),
            (ANOTHER, Some("keep")),
            (EPHEMERAL, None),
        ],
        || -> Result<()> {
            let snapshot = EnvGuard::capture();
            temp_env::with_vars(
                [
                    (EXISTING, Some("changed")),
                    (ANOTHER, None),
                    (EPHEMERAL, Some("ephemeral")),
                ],
                || -> Result<()> {
                    snapshot.restore();
                    let observed = observed_environment()?;
                    ensure!(
                        observed.get(EXISTING).map(String::as_str) == Some("original"),
                        "existing value should be restored"
                    );
                    ensure!(
                        observed.get(ANOTHER).map(String::as_str) == Some("keep"),
                        "other value should be restored"
                    );
                    ensure!(
                        !observed.contains_key(EPHEMERAL),
                        "ephemeral value should be removed"
                    );
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn removes_variables_created_during_guard_lifetime() -> Result<()> {
    const PRE_EXISTING: &str = "WEAVER_ENV_GUARD_PRE_EXISTING";
    const CREATED: &str = "WEAVER_ENV_GUARD_CREATED";

    let _guard = lock_env()?;
    temp_env::with_vars(
        [(PRE_EXISTING, Some("value")), (CREATED, None)],
        || -> Result<()> {
            let snapshot = EnvGuard::capture();
            temp_env::with_vars([(CREATED, Some("temporary"))], || -> Result<()> {
                let observed = observed_environment()?;
                ensure!(
                    observed.get(PRE_EXISTING).map(String::as_str) == Some("value"),
                    "existing value should remain"
                );
                ensure!(
                    observed.get(CREATED).map(String::as_str) == Some("temporary"),
                    "created value should exist"
                );

                snapshot.restore();

                let observed = observed_environment()?;
                ensure!(
                    observed.get(PRE_EXISTING).map(String::as_str) == Some("value"),
                    "existing value should remain"
                );
                ensure!(
                    !observed.contains_key(CREATED),
                    "created value should be removed"
                );
                Ok(())
            })
        },
    )
}

#[test]
fn restores_environment_on_drop() -> Result<()> {
    const PRE_EXISTING: &str = "WEAVER_ENV_GUARD_DROP_PRE_EXISTING";
    const CREATED: &str = "WEAVER_ENV_GUARD_DROP_CREATED";

    let _guard = lock_env()?;
    temp_env::with_vars(
        [(PRE_EXISTING, Some("original")), (CREATED, None)],
        || -> Result<()> {
            let snapshot = EnvGuard::capture();
            temp_env::with_vars(
                [
                    (PRE_EXISTING, Some("modified")),
                    (CREATED, Some("temporary")),
                ],
                || -> Result<()> {
                    let observed = observed_environment()?;
                    ensure!(
                        observed.get(PRE_EXISTING).map(String::as_str) == Some("modified"),
                        "existing value should change"
                    );
                    ensure!(
                        observed.get(CREATED).map(String::as_str) == Some("temporary"),
                        "created value should exist"
                    );

                    drop(snapshot);

                    let observed = observed_environment()?;
                    ensure!(
                        observed.get(PRE_EXISTING).map(String::as_str) == Some("original"),
                        "existing value should be restored"
                    );
                    ensure!(
                        !observed.contains_key(CREATED),
                        "created value should be removed"
                    );
                    Ok(())
                },
            )
        },
    )
}
