//! Behavioural tests for the apply-patch command.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

use crate::dispatch::act::apply_patch::{ApplyPatchError, ApplyPatchExecutor, ApplyPatchFailure};
use crate::safety_harness::VerificationFailure;
use crate::safety_harness::{ConfigurableSemanticLock, ConfigurableSyntacticLock};

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
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("temp dir");
        Self {
            temp_dir,
            patch: None,
            syntactic_lock: ConfigurableSyntacticLock::passing(),
            semantic_lock: ConfigurableSemanticLock::passing(),
            result: None,
        }
    }

    fn create_file(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, content).expect("write file");
    }

    fn read_file(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).expect("read file")
    }

    fn file_exists(&self, relative: &str) -> bool {
        self.path(relative).exists()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.temp_dir.path().join(relative)
    }

    fn set_patch(&mut self, patch: &str) {
        self.patch = Some(patch.to_string());
    }

    fn apply_patch(&mut self) {
        let patch = self.patch.clone().expect("patch should be set");
        let executor = ApplyPatchExecutor::new(
            self.temp_dir.path().to_path_buf(),
            &self.syntactic_lock,
            &self.semantic_lock,
        );
        self.result = Some(executor.execute(&patch).map(|_| ()));
    }
}

#[fixture]
fn world() -> RefCell<ApplyPatchWorld> {
    RefCell::new(ApplyPatchWorld::new())
}

#[given("a workspace with the default source file")]
fn given_default_source(world: &RefCell<ApplyPatchWorld>) {
    world.borrow().create_file("src/main.rs", DEFAULT_SOURCE);
}

#[given("an empty workspace")]
#[expect(
    unused_variables,
    reason = "BDD step intentionally relies on the default empty workspace"
)]
fn given_empty_workspace(world: &RefCell<ApplyPatchWorld>) {}

#[given("a patch that replaces the main message")]
fn given_patch_replace(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(MODIFY_PATCH);
}

#[given("a patch that creates a new module")]
fn given_patch_create(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(CREATE_PATCH);
}

#[given("a patch that deletes a file")]
fn given_patch_delete(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(DELETE_PATCH);
}

#[given("a patch that targets a parent directory")]
fn given_patch_traversal(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(TRAVERSAL_PATCH);
}

#[given("a patch with an invalid diff header")]
fn given_patch_invalid_header(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(INVALID_HEADER_PATCH);
}

#[given("a patch that omits the create hunk")]
fn given_patch_missing_hunk(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().set_patch(MISSING_HUNK_PATCH);
}

#[given("a workspace with a deletable file")]
fn given_deletable_file(world: &RefCell<ApplyPatchWorld>) {
    world.borrow().create_file("src/remove.rs", "fn old() {}\n");
}

#[given("an apply-patch syntactic lock that passes")]
fn given_syntactic_passes(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::passing();
}

#[given("an apply-patch syntactic lock on {path} that fails with {message}")]
fn given_syntactic_fails(world: &RefCell<ApplyPatchWorld>, path: String, message: String) {
    let path = PathBuf::from(strip_quotes(&path));
    let failure = VerificationFailure::new(path, message.as_str());
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::failing(vec![failure]);
}

#[given("an apply-patch semantic lock that passes")]
fn given_semantic_passes(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::passing();
}

#[given("an apply-patch semantic lock on {path} that fails with {message}")]
fn given_semantic_fails(world: &RefCell<ApplyPatchWorld>, path: String, message: String) {
    let path = PathBuf::from(strip_quotes(&path));
    let failure = VerificationFailure::new(path, message.as_str());
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::failing(vec![failure]);
}

#[when("the patch is applied")]
fn when_patch_applied(world: &RefCell<ApplyPatchWorld>) {
    world.borrow_mut().apply_patch();
}

#[then("the apply-patch file {path} contains {snippet}")]
fn then_file_contains(world: &RefCell<ApplyPatchWorld>, path: String, snippet: String) {
    let path = strip_quotes(&path);
    let snippet = strip_quotes(&snippet);
    let content = world.borrow().read_file(path);
    assert!(
        content.contains(snippet),
        "expected {path} to contain {snippet:?}, got: {content:?}"
    );
}

#[then("the file {path} is missing")]
fn then_file_missing(world: &RefCell<ApplyPatchWorld>, path: String) {
    let path = strip_quotes(&path);
    assert!(
        !world.borrow().file_exists(path),
        "expected {path} to be missing"
    );
}

#[then("the apply-patch response succeeds")]
fn then_patch_succeeds(world: &RefCell<ApplyPatchWorld>) {
    let world = world.borrow();
    let result = world.result.as_ref().expect("result set");
    assert!(result.is_ok(), "expected success, got: {result:?}");
}

#[then("the apply-patch fails with {kind}")]
fn then_patch_fails(world: &RefCell<ApplyPatchWorld>, kind: String) {
    let kind = strip_quotes(&kind);
    let world = world.borrow();
    let result = world.result.as_ref().expect("result set");
    let Err(error) = result else {
        panic!("expected failure, got success");
    };
    match kind {
        "InvalidPath" => assert!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::InvalidPath { .. })
        )),
        "InvalidDiffHeader" => assert!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::InvalidDiffHeader { .. })
        )),
        "MissingHunk" => assert!(matches!(
            error,
            ApplyPatchFailure::Patch(ApplyPatchError::MissingHunk { .. })
        )),
        "SyntacticLock" => assert!(matches!(
            error,
            ApplyPatchFailure::Verification {
                phase: "SyntacticLock",
                ..
            }
        )),
        "SemanticLock" => assert!(matches!(
            error,
            ApplyPatchFailure::Verification {
                phase: "SemanticLock",
                ..
            }
        )),
        other => panic!("unsupported failure kind: {other}"),
    }
}

#[rustfmt::skip]
fn strip_quotes(value: &str) -> &str { value.trim_matches('"') }

#[scenario(path = "tests/features/apply_patch.feature")]
fn apply_patch_scenarios(#[from(world)] world: RefCell<ApplyPatchWorld>) {
    drop(world);
}
