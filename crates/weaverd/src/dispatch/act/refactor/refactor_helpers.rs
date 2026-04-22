//! Shared helper data and pure functions for `act refactor` behaviour tests.
//!
//! This file is intentionally loaded multiple times by different test modules
//! using `#[path = "refactor_helpers.rs"]` to provide shared test utilities.

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

    pub(crate) fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
        let config = Config {
            daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
            ..Config::default()
        };
        let provider =
            SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
        FusionBackends::new(config, provider)
    }

    pub(crate) fn standard_rename_args(file: &str) -> Vec<String> {
        vec![
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
            String::from(file),
            String::from("offset=1"),
            String::from("new_name=woven"),
        ]
    }

    pub(crate) fn configure_request(request: &mut CommandRequest, args: Vec<String>) {
        *request = command_request(args);
    }

    #[test]
    fn builder_helpers_are_referenced() {
        let mut request = command_request(Vec::new());
        let _ = build_backends;
        let args = standard_rename_args("notes.py");
        configure_request(&mut request, args);
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

    pub(crate) struct RefusedResolution<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) language: Option<&'a str>,
        pub(crate) requested_provider: Option<&'a str>,
        pub(crate) selection_mode: SelectionMode,
        pub(crate) refusal_reason: RefusalReason,
        pub(crate) candidates: Vec<CandidateEvaluation>,
    }

    pub(crate) struct SelectedResolution<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) language: &'a str,
        pub(crate) provider: &'a str,
        pub(crate) selection_mode: SelectionMode,
        pub(crate) requested_provider: Option<&'a str>,
    }

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

    pub(crate) struct AutoResolutionContext<'a> {
        pub(crate) capability: CapabilityId,
        pub(crate) requested_provider: Option<&'a str>,
        pub(crate) selection_mode: SelectionMode,
    }

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

    #[test]
    fn resolution_helpers_are_referenced() {
        let refused = RefusedResolution {
            capability: CapabilityId::RenameSymbol,
            language: None,
            requested_provider: None,
            selection_mode: SelectionMode::Automatic,
            refusal_reason: RefusalReason::UnsupportedLanguage,
            candidates: Vec::new(),
        };
        let selected = SelectedResolution {
            capability: CapabilityId::RenameSymbol,
            language: "python",
            provider: "rope",
            selection_mode: SelectionMode::Automatic,
            requested_provider: None,
        };
        let _ = selected_resolution(selected);
        let _ = refused_resolution(refused);
        let _ = rejected_candidate("rope", CandidateReason::UnsupportedLanguage);
        let context = AutoResolutionContext {
            capability: CapabilityId::RenameSymbol,
            requested_provider: None,
            selection_mode: SelectionMode::Automatic,
        };
        let _ = resolve_auto_language(context, Some("python"), "rope", Vec::new());
    }
}

pub(crate) mod rollback {
    //! Runtime doubles that exercise rollback paths for refactor tests.

    use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};

    use super::resolutions::{SelectedResolution, selected_resolution};
    use crate::dispatch::act::refactor::{
        RefactorPluginRuntime,
        resolution::{CapabilityResolutionEnvelope, ResolutionRequest},
    };

    pub(crate) struct RollbackRuntime {
        pub(crate) resolution: CapabilityResolutionEnvelope,
        pub(crate) execute_result: ExecuteResult,
    }

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

    pub(crate) fn selected_runtime(
        config: SelectedResolution<'_>,
        execute_result: ExecuteResult,
    ) -> RollbackRuntime {
        RollbackRuntime {
            resolution: selected_resolution(config),
            execute_result,
        }
    }

    pub(crate) fn rollback_runtime(
        resolution: CapabilityResolutionEnvelope,
        execute_result: ExecuteResult,
    ) -> RollbackRuntime {
        RollbackRuntime {
            resolution,
            execute_result,
        }
    }

    #[test]
    fn rollback_helpers_are_referenced() {
        let runtime = selected_runtime(
            SelectedResolution {
                capability: weaver_plugins::CapabilityId::RenameSymbol,
                language: "python",
                provider: "rope",
                selection_mode:
                    crate::dispatch::act::refactor::resolution::SelectionMode::Automatic,
                requested_provider: None,
            },
            ExecuteResult::MissingPlugin("rope"),
        );
        let _ = rollback_runtime(
            runtime.resolution.clone(),
            ExecuteResult::Success(PluginResponse::success(PluginOutput::Empty)),
        );
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

    pub(crate) enum FileKind {
        Python,
        Rust,
        Other,
    }

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

    pub(crate) fn original_content_for(path: &Path) -> &'static str {
        content_table(classify_file(path)).original
    }

    pub(crate) fn updated_content_for(path: &Path) -> &'static str {
        content_table(classify_file(path)).updated
    }

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

    pub(crate) fn routed_diff_for(path: &Path) -> String {
        routed_format_diff(path, |p| format!("diff --git a/{p} b/{p}"))
    }

    pub(crate) fn routed_malformed_diff_for(path: &Path) -> String {
        routed_format_diff(path, |p| format!("diff --git a/{p}"))
    }

    #[test]
    fn content_helpers_are_referenced() {
        let path = Path::new("notes.py");
        let _ = format_diff(path, "diff --git a/notes.py b/notes.py\n");
        let _ = FileKind::Python;
        let _ = classify_file(path);
        let _ = content_table(FileKind::Python);
        let _ = original_content_for(path);
        let _ = updated_content_for(path);
        let _ = routed_patch_path(path);
        let _ = routed_format_diff(path, |value| value.to_owned());
        let _ = routed_diff_for(path);
        let _ = routed_malformed_diff_for(path);
    }
}
