//! Shared `#[cfg(test)]` helper module for `act refactor` tests.
//!
//! Sibling test modules such as `tests.rs`, `contract_tests.rs`,
//! `rollback_tests.rs`, and `behaviour.rs` import these helpers with
//! `use crate::dispatch::act::refactor::refactor_helpers::...`.
//!
//! The parent `act::refactor` module declares this file once under
//! `#[cfg(test)]`, so the `weaverd` test build compiles it once as a named
//! module. That single compiled shape keeps cross-test helper usage visible to
//! Rust's dead-code analysis, so no dead-code suppression attributes are
//! required.

pub(crate) mod builders {
    //! Synthetic request and backend builders for refactor tests.

    use std::path::Path;

    use weaver_cards::DEFAULT_CACHE_CAPACITY;
    use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

    use crate::{
        backends::FusionBackends,
        dispatch::request::{CommandDescriptor, CommandRequest},
        semantic_provider::SemanticBackendProvider,
    };

    /// Constructs a minimal `act refactor` command request.
    ///
    /// Sets `domain` to `"act"` and `operation` to `"refactor"`, forwarding
    /// `arguments` verbatim.
    pub(crate) fn command_request(arguments: Vec<String>) -> CommandRequest {
        CommandRequest {
            command: CommandDescriptor {
                domain: String::from("act"),
                operation: String::from("refactor"),
            },
            arguments,
            patch: None,
        }
    }

    /// Builds test-ready fusion backends rooted at `socket_path`.
    ///
    /// Uses a default `Config` and a default-capacity `SemanticBackendProvider`.
    pub(crate) fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
        let config = Config {
            daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
            ..Config::default()
        };
        let provider =
            SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
        FusionBackends::new(config, provider)
    }

    /// Builds a rename command argument vector with an explicit provider selection.
    ///
    /// Prepends `--provider <provider>` to the standard rename arguments, producing
    /// a complete argument list for tests and callers that exercise the
    /// explicit-provider code path.
    ///
    /// # Parameters
    /// - `file`: workspace-relative path to the file under rename.
    /// - `provider`: the provider name to pass as `--provider` (e.g. `"rope"`, `"rust-analyzer"`).
    pub(crate) fn standard_rename_args_for_provider(file: &str, provider: &str) -> Vec<String> {
        vec![
            String::from("--provider"),
            String::from(provider),
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
            String::from(file),
            String::from("offset=1"),
            String::from("new_name=woven"),
        ]
    }

    /// Replaces `request` in-place with an `act refactor` request for `args`.
    pub(crate) fn configure_request(request: &mut CommandRequest, args: Vec<String>) {
        *request = command_request(args);
    }
}

pub(crate) mod resolutions {
    //! Shared resolution envelopes and routing helpers for refactor tests.

    use weaver_plugins::CapabilityId;

    use crate::dispatch::act::refactor::resolution::{
        CandidateEvaluation,
        CandidateReason,
        CapabilityResolutionDetails,
        CapabilityResolutionEnvelope,
        RefusalReason,
        ResolutionOutcome,
        SelectionMode,
    };

    /// Configuration bundle for building a refused provider resolution.
    pub(crate) struct RefusedResolution<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) language: Option<&'a str>,
        pub(crate) requested_provider: Option<&'a str>,
        pub(crate) selection_mode: SelectionMode,
        pub(crate) refusal_reason: RefusalReason,
        pub(crate) candidates: Vec<CandidateEvaluation>,
    }

    /// Configuration bundle for building a selected provider resolution.
    pub(crate) struct SelectedResolution<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) language: &'a str,
        pub(crate) provider: &'a str,
        pub(crate) selection_mode: SelectionMode,
        pub(crate) requested_provider: Option<&'a str>,
    }

    /// Builds a selected capability resolution envelope from `config`.
    pub(crate) fn selected_resolution(
        config: SelectedResolution<'_>,
    ) -> CapabilityResolutionEnvelope {
        CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
            capability: config.capability,
            language: Some(String::from(config.language)),
            requested_provider: config.requested_provider.map(String::from),
            selected_provider: Some(String::from(config.provider)),
            selection_mode: config.selection_mode,
            outcome: ResolutionOutcome::Selected,
            refusal_reason: None,
            candidates: vec![CandidateEvaluation {
                provider: String::from(config.provider),
                accepted: true,
                reason: CandidateReason::MatchedLanguageAndCapability,
            }],
        })
    }

    /// Builds a refused capability resolution envelope from `config`.
    pub(crate) fn refused_resolution(
        config: RefusedResolution<'_>,
    ) -> CapabilityResolutionEnvelope {
        CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
            capability: config.capability,
            language: config.language.map(String::from),
            requested_provider: config.requested_provider.map(String::from),
            selected_provider: None,
            selection_mode: config.selection_mode,
            outcome: ResolutionOutcome::Refused,
            refusal_reason: Some(config.refusal_reason),
            candidates: config.candidates,
        })
    }

    /// Constructs a rejected provider candidate evaluation.
    pub(crate) fn rejected_candidate(
        provider: &str,
        reason: CandidateReason,
    ) -> CandidateEvaluation {
        CandidateEvaluation {
            provider: String::from(provider),
            accepted: false,
            reason,
        }
    }

    /// Context shared by selected and refused automatic language resolution paths.
    pub(crate) struct AutoResolutionContext<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) requested_provider: Option<&'a str>,
        pub(crate) selection_mode: SelectionMode,
    }

    /// Resolves an automatic language selection.
    ///
    /// Returns a selected resolution when `language_name` is present, otherwise
    /// returns an unsupported-language refusal.
    pub(crate) fn resolve_auto_language(
        context: AutoResolutionContext<'_>,
        language_name: Option<&'static str>,
        provider: &'static str,
        candidates: Vec<CandidateEvaluation>,
    ) -> CapabilityResolutionEnvelope {
        if let Some(language) = language_name {
            selected_resolution(SelectedResolution {
                capability: context.capability,
                language,
                provider,
                selection_mode: context.selection_mode,
                requested_provider: context.requested_provider,
            })
        } else {
            refused_resolution(RefusedResolution {
                capability: context.capability,
                language: None,
                requested_provider: context.requested_provider,
                selection_mode: context.selection_mode,
                refusal_reason: RefusalReason::UnsupportedLanguage,
                candidates,
            })
        }
    }
}

pub(crate) mod rollback {
    //! Runtime doubles that exercise rollback paths for refactor tests.

    use weaver_plugins::{PluginError, PluginRequest, PluginResponse};

    use super::resolutions::{SelectedResolution, selected_resolution};
    use crate::dispatch::act::refactor::{
        RefactorPluginRuntime,
        resolution::{CapabilityResolutionEnvelope, ResolutionRequest},
    };

    /// Test double that returns fixed refactor resolution and execution outcomes.
    pub(crate) struct RollbackRuntime {
        pub(crate) resolution: CapabilityResolutionEnvelope,
        pub(crate) execute_result: ExecuteResult,
    }

    /// Execution outcome returned by `RollbackRuntime`.
    pub(crate) enum ExecuteResult {
        Success(PluginResponse),
        MissingPlugin(&'static str),
    }

    impl RefactorPluginRuntime for RollbackRuntime {
        fn resolve(
            &self,
            _request: ResolutionRequest<'_>,
        ) -> Result<CapabilityResolutionEnvelope, PluginError> {
            Ok(self.resolution.clone())
        }

        fn execute(
            &self,
            _provider: &str,
            _request: &PluginRequest,
        ) -> Result<PluginResponse, PluginError> {
            match &self.execute_result {
                ExecuteResult::Success(response) => Ok(response.clone()),
                ExecuteResult::MissingPlugin(name) => Err(PluginError::NotFound {
                    name: String::from(*name),
                }),
            }
        }
    }

    /// Creates a rollback runtime with a selected resolution from `config`.
    pub(crate) fn selected_runtime(
        config: SelectedResolution<'_>,
        execute_result: ExecuteResult,
    ) -> RollbackRuntime {
        RollbackRuntime {
            resolution: selected_resolution(config),
            execute_result,
        }
    }

    /// Creates a rollback runtime from an existing resolution envelope.
    pub(crate) fn rollback_runtime(
        resolution: CapabilityResolutionEnvelope,
        execute_result: ExecuteResult,
    ) -> RollbackRuntime {
        RollbackRuntime {
            resolution,
            execute_result,
        }
    }
}

pub(crate) mod content {
    //! Deterministic file contents and diff payloads for refactor tests.

    use std::path::Path;

    fn format_diff(path: &Path, git_header: &str) -> String {
        let original = original_content_for(path);
        let updated = updated_content_for(path);
        format!("{git_header}\n<<<<<<< SEARCH\n{original}=======\n{updated}>>>>>>> REPLACE\n",)
    }

    /// File type classification used to choose deterministic refactor fixtures.
    pub(crate) enum FileKind {
        Python,
        Rust,
        Other,
    }

    /// Classifies `path` by extension into a deterministic fixture kind.
    pub(crate) fn classify_file(path: &Path) -> FileKind {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("py") => FileKind::Python,
            Some("rs") => FileKind::Rust,
            _ => FileKind::Other,
        }
    }

    struct FileContents {
        original: &'static str,
        updated: &'static str,
    }

    fn content_table(kind: FileKind) -> FileContents {
        match kind {
            FileKind::Python => FileContents {
                original: "old_name = 1\nprint(old_name)\n",
                updated: "woven = 1\nprint(woven)\n",
            },
            FileKind::Rust => FileContents {
                original: concat!(
                    "fn main() {\n",
                    "    let old_name = 1;\n",
                    "    println!(\"{}\", old_name);\n",
                    "}\n",
                ),
                updated: concat!(
                    "fn main() {\n",
                    "    let woven = 1;\n",
                    "    println!(\"{}\", woven);\n",
                    "}\n",
                ),
            },
            FileKind::Other => FileContents {
                original: "hello world\n",
                updated: "hello woven\n",
            },
        }
    }

    /// Returns the deterministic pre-rename fixture content for `path`.
    pub(crate) fn original_content_for(path: &Path) -> &'static str {
        content_table(classify_file(path)).original
    }

    /// Returns the deterministic post-rename fixture content for `path`.
    pub(crate) fn updated_content_for(path: &Path) -> &'static str {
        content_table(classify_file(path)).updated
    }

    /// Returns the path targeted by routed refactor patch output.
    ///
    /// Python and Rust inputs are routed to `notes.txt`; all other inputs keep
    /// their original path.
    pub(crate) fn routed_patch_path(path: &Path) -> &Path {
        match classify_file(path) {
            FileKind::Python | FileKind::Rust => Path::new("notes.txt"),
            FileKind::Other => path,
        }
    }

    fn routed_format_diff(path: &Path, make_header: impl Fn(&str) -> String) -> String {
        let patch_path = routed_patch_path(path);
        format_diff(path, &make_header(&patch_path.to_string_lossy()))
    }

    /// Builds a well-formed routed diff for `path`.
    pub(crate) fn routed_diff_for(path: &Path) -> String {
        routed_format_diff(path, |p| format!("diff --git a/{p} b/{p}"))
    }

    /// Builds a malformed routed diff for error-path tests.
    ///
    /// The malformed header intentionally omits the `b/` segment.
    pub(crate) fn routed_malformed_diff_for(path: &Path) -> String {
        routed_format_diff(path, |p| format!("diff --git a/{p}"))
    }
}

#[cfg(test)]
mod lint_compliance_tests {
    //! Regression tests for helper-module lint policy compliance.
    #[test]
    fn refactor_helpers_avoids_dead_code_suppression_patterns() {
        let compact: String = include_str!("refactor_helpers.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let file_wide_allow = ["#![", "allow("].concat();
        let item_dead_code_allow = ["#[", "allow(dead_code"].concat();
        let dead_code_witness = "const_:";
        assert!(
            !compact.contains(&file_wide_allow),
            "refactor_helpers.rs contains forbidden pattern `{file_wide_allow}`; file-wide \
             blanket lint allows are banned by project policy.",
        );
        assert!(
            !compact.contains(&item_dead_code_allow),
            "refactor_helpers.rs contains forbidden pattern `{item_dead_code_allow}`; item-level \
             dead-code allows without a reason are banned by project policy.",
        );
        assert!(
            !compact.contains(dead_code_witness),
            "refactor_helpers.rs contains forbidden pattern `{dead_code_witness}`; anonymous \
             const witnesses must not be used to mask dead-code lints.",
        );
    }
}
