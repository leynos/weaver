//! Behavioural tests for the apply-patch command.

use std::{cell::RefCell, path::PathBuf};

use anyhow::{Context, Result, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    dispatch::act::apply_patch::{ApplyPatchError, ApplyPatchExecutor, ApplyPatchFailure},
    safety_harness::{ConfigurableSemanticLock, ConfigurableSyntacticLock, VerificationFailure},
    tests::support::fs as test_fs,
};

const DEFAULT_SOURCE: &str = "fn main() {\n    println!(\"Old Message\");\n}\n";
const MODIFY_PATCH: &str = concat!(
    "diff --git a/src/main.rs b/src/main.rs\n",
    "<<<<<<< SEARCH\n",
    "fn main() {\n",
    "    println!(\"Old Message\");\n",
    "}\n",
    "=======\n",
    "fn main() {\n",
    "    println!(\"New Message\");\n",
    "}\n",
    ">>>>>>> REPLACE\n",
);
const CREATE_PATCH: &str = concat!(
    "diff --git a/src/new.rs b/src/new.rs\n",
    "new file mode 100644\n",
    "--- /dev/null\n",
    "+++ b/src/new.rs\n",
    "@@ -0,0 +1,2 @@\n",
    "+fn hello() {}\n",
    "+fn world() {}\n",
);
const DELETE_PATCH: &str = concat!(
    "diff --git a/src/remove.rs b/src/remove.rs\n",
    "deleted file mode 100644\n",
);
const TRAVERSAL_PATCH: &str = concat!(
    "diff --git a/../escape.rs b/../escape.rs\n",
    "<<<<<<< SEARCH\n",
    "old\n",
    "=======\n",
    "new\n",
    ">>>>>>> REPLACE\n",
);
const INVALID_HEADER_PATCH: &str = concat!(
    "diff --git a/src/main.rs\n",
    "<<<<<<< SEARCH\n",
    "old\n",
    "=======\n",
    "new\n",
    ">>>>>>> REPLACE\n",
);
const MISSING_HUNK_PATCH: &str = concat!(
    "diff --git a/src/new.rs b/src/new.rs\n",
    "new file mode 100644\n",
    "--- /dev/null\n",
    "+++ b/src/new.rs\n",
);

struct ApplyPatchWorld {
    temp_dir: TempDir,
    patch: Option<String>,
    syntactic_lock: ConfigurableSyntacticLock,
    semantic_lock: ConfigurableSemanticLock,
    result: Option<Result<(), ApplyPatchFailure>>,
}

impl ApplyPatchWorld {
    fn new(temp_dir: TempDir) -> Self {
        Self {
            temp_dir,
            patch: None,
            syntactic_lock: ConfigurableSyntacticLock::passing(),
            semantic_lock: ConfigurableSemanticLock::passing(),
            result: None,
        }
    }

    fn create_file(&self, relative: &str, content: &str) -> Result<()> {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            test_fs::create_dir_all(parent).context("create fixture parent directories")?;
        }
        test_fs::write(&path, content).context("write fixture file")?;
        Ok(())
    }

    fn read_file(&self, relative: &str) -> Result<String> {
        test_fs::read_to_string(self.path(relative)).context("read fixture file")
    }

    fn file_exists(&self, relative: &str) -> Result<bool> {
        test_fs::exists(self.path(relative)).context("check fixture file existence")
    }

    fn path(&self, relative: &str) -> PathBuf { self.temp_dir.path().join(relative) }

    fn set_patch(&mut self, patch: &str) { self.patch = Some(patch.to_string()); }

    fn apply_patch(&mut self) -> Result<()> {
        let patch = self.patch.clone().context("patch should be set")?;
        let executor = ApplyPatchExecutor::new(
            self.temp_dir.path().to_path_buf(),
            &self.syntactic_lock,
            &self.semantic_lock,
        );
        self.result = Some(executor.execute(&patch).map(|_| ()));
        Ok(())
    }
}

/// Fixture payload: workspace creation can fail, so steps unwrap it themselves.
type ApplyPatchWorldFixture = Result<RefCell<ApplyPatchWorld>, String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> ApplyPatchWorldFixture {
    TempDir::new()
        .map(|temp_dir| RefCell::new(ApplyPatchWorld::new(temp_dir)))
        .map_err(|error| format!("create temporary workspace: {error}"))
}

/// Borrows the world, surfacing fixture construction failure as a step failure.
fn apply_patch_world(world: &ApplyPatchWorldFixture) -> Result<&RefCell<ApplyPatchWorld>> {
    world
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

#[given("a workspace with the default source file")]
fn given_default_source(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow()
        .create_file("src/main.rs", DEFAULT_SOURCE)
}

#[given("an empty workspace")]
#[expect(
    unused_variables,
    reason = "BDD step intentionally relies on the default empty workspace"
)]
fn given_empty_workspace(world: &ApplyPatchWorldFixture) {}

#[given("a patch that replaces the main message")]
fn given_patch_replace(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(MODIFY_PATCH);
    Ok(())
}

#[given("a patch that creates a new module")]
fn given_patch_create(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(CREATE_PATCH);
    Ok(())
}

#[given("a patch that deletes a file")]
fn given_patch_delete(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(DELETE_PATCH);
    Ok(())
}

#[given("a patch that targets a parent directory")]
fn given_patch_traversal(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(TRAVERSAL_PATCH);
    Ok(())
}

#[given("a patch with an invalid diff header")]
fn given_patch_invalid_header(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(INVALID_HEADER_PATCH);
    Ok(())
}

#[given("a patch that omits the create hunk")]
fn given_patch_missing_hunk(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow_mut()
        .set_patch(MISSING_HUNK_PATCH);
    Ok(())
}

#[given("a workspace with a deletable file")]
fn given_deletable_file(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?
        .borrow()
        .create_file("src/remove.rs", "fn old() {}\n")
}

#[given("an apply-patch syntactic lock that passes")]
fn given_syntactic_passes(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::passing();
    Ok(())
}

#[given("an apply-patch syntactic lock on {path} that fails with {message}")]
fn given_syntactic_fails(
    world: &ApplyPatchWorldFixture,
    path: String,
    message: String,
) -> Result<()> {
    let path = PathBuf::from(strip_quotes(&path));
    let failure = VerificationFailure::new(path, message.as_str());
    apply_patch_world(world)?.borrow_mut().syntactic_lock =
        ConfigurableSyntacticLock::failing(vec![failure]);
    Ok(())
}

#[given("an apply-patch semantic lock that passes")]
fn given_semantic_passes(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?.borrow_mut().semantic_lock = ConfigurableSemanticLock::passing();
    Ok(())
}

#[given("an apply-patch semantic lock on {path} that fails with {message}")]
fn given_semantic_fails(
    world: &ApplyPatchWorldFixture,
    path: String,
    message: String,
) -> Result<()> {
    let path = PathBuf::from(strip_quotes(&path));
    let failure = VerificationFailure::new(path, message.as_str());
    apply_patch_world(world)?.borrow_mut().semantic_lock =
        ConfigurableSemanticLock::failing(vec![failure]);
    Ok(())
}

#[when("the patch is applied")]
fn when_patch_applied(world: &ApplyPatchWorldFixture) -> Result<()> {
    apply_patch_world(world)?.borrow_mut().apply_patch()
}

#[then("the apply-patch file {path} contains {snippet}")]
fn then_file_contains(world: &ApplyPatchWorldFixture, path: String, snippet: String) -> Result<()> {
    let path = strip_quotes(&path);
    let snippet = strip_quotes(&snippet);
    let content = apply_patch_world(world)?.borrow().read_file(path)?;
    ensure!(
        content.contains(snippet),
        "expected {path} to contain {snippet:?}, got: {content:?}"
    );
    Ok(())
}

#[then("the file {path} is missing")]
fn then_file_missing(world: &ApplyPatchWorldFixture, path: String) -> Result<()> {
    let path = strip_quotes(&path);
    ensure!(
        !apply_patch_world(world)?.borrow().file_exists(path)?,
        "expected {path} to be missing"
    );
    Ok(())
}

#[then("the apply-patch response succeeds")]
fn then_patch_succeeds(world: &ApplyPatchWorldFixture) -> Result<()> {
    let state = apply_patch_world(world)?.borrow();
    let result = state.result.as_ref().context("result set")?;
    ensure!(result.is_ok(), "expected success, got: {result:?}");
    Ok(())
}

#[then("the apply-patch fails with {kind}")]
fn then_patch_fails(world: &ApplyPatchWorldFixture, kind: String) -> Result<()> {
    let kind = strip_quotes(&kind);
    let state = apply_patch_world(world)?.borrow();
    let result = state.result.as_ref().context("result set")?;
    let error = match result {
        Err(error) => error,
        Ok(()) => anyhow::bail!("expected failure, got success"),
    };
    match kind {
        "InvalidPath" => ensure!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::InvalidPath { .. })
        )),
        "InvalidDiffHeader" => ensure!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::InvalidDiffHeader { .. })
        )),
        "MissingHunk" => ensure!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::MissingHunk { .. })
        )),
        "SyntacticLock" => ensure!(matches!(
            error,
            ApplyPatchFailure::Verification {
                phase: "SyntacticLock",
                ..
            }
        )),
        "SemanticLock" => ensure!(matches!(
            error,
            ApplyPatchFailure::Verification {
                phase: "SemanticLock",
                ..
            }
        )),
        other => anyhow::bail!("unsupported failure kind: {other}"),
    }
    Ok(())
}

#[rustfmt::skip]
fn strip_quotes(value: &str) -> &str { value.trim_matches('"') }

#[scenario(path = "tests/features/apply_patch.feature")]
fn apply_patch_scenarios(#[from(world)] world: ApplyPatchWorldFixture) { drop(world); }
