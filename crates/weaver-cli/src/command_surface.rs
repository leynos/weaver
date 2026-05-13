//! Temporary command-surface metadata for resource-first Weaver commands.
//!
//! This adapter captures Weaver-owned semantic command metadata until the
//! reusable OrthoConfig command-contract machinery is available. Keep generic
//! CLI policy out of this module; it only records Weaver resource paths,
//! capabilities, selector forms, and the legacy daemon operation used during
//! the 0.1.0 command reset.
//!
//! Removal policy: replace `CommandSurfaceRecord` with OrthoConfig 6.1
//! recursive command metadata and OrthoConfig 7.2.7 generic
//! capability/provenance metadata once those contracts can carry the
//! Weaver-specific fields below. Replace the read-only catalogue and lookup
//! helpers with the OrthoConfig 6.1 generated command registry, and use
//! OrthoConfig 6.2/6.3 output for context and skill surfaces. ADR 007 records
//! the same policy for reviewers.

/// A public selector form accepted by a command-surface record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectorForm {
    /// Editor-style `--uri` plus `--position` selection.
    Position,
}

/// Whether the command can mutate the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mutability {
    /// Read-only commands observe code and do not propose edits.
    ReadOnly,
}

/// Whether a command completes synchronously or submits recoverable work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AsyncClass {
    /// The command completes in the foreground without job-ledger recovery.
    Synchronous,
}

/// How Weaver should choose implementation providers for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderPolicy {
    /// Let Weaver choose the provider through deterministic capability routing.
    Auto,
}

/// Metadata for one resource-first command.
///
/// Temporary adapter: replaced by OrthoConfig 6.1 command metadata plus
/// OrthoConfig 7.2.7 capability/provenance metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandSurfaceRecord {
    pub(crate) resource_path: &'static [&'static str],
    pub(crate) verb: &'static str,
    pub(crate) capability_id: &'static str,
    pub(crate) selector_forms: &'static [SelectorForm],
    pub(crate) output_schema: &'static str,
    pub(crate) error_schema: &'static str,
    pub(crate) mutability: Mutability,
    pub(crate) async_class: AsyncClass,
    pub(crate) provider_policy: ProviderPolicy,
    pub(crate) daemon_domain: &'static str,
    pub(crate) daemon_operation: &'static str,
    pub(crate) examples: &'static [&'static str],
    pub(crate) skill_refs: &'static [&'static str],
}

const POSITION_SELECTOR: &[SelectorForm] = &[SelectorForm::Position];

/// Command-surface record for `weaver definitions get`.
pub(crate) const DEFINITIONS_GET: CommandSurfaceRecord = CommandSurfaceRecord {
    resource_path: &["definitions"],
    verb: "get",
    capability_id: "definition.get",
    selector_forms: POSITION_SELECTOR,
    output_schema: "weaver.definition.locations.v1",
    error_schema: "weaver.error.v1",
    mutability: Mutability::ReadOnly,
    async_class: AsyncClass::Synchronous,
    provider_policy: ProviderPolicy::Auto,
    daemon_domain: "observe",
    daemon_operation: "get-definition",
    examples: &["weaver definitions get --uri file:///src/main.rs --position 10:5"],
    skill_refs: &["code-reading-loop"],
};

/// Metadata-only sibling that proves the read command family can grow without
/// a second adapter shape.
pub(crate) const REFERENCES_LIST: CommandSurfaceRecord = CommandSurfaceRecord {
    resource_path: &["references"],
    verb: "list",
    capability_id: "references.list",
    selector_forms: POSITION_SELECTOR,
    output_schema: "weaver.reference.locations.v1",
    error_schema: "weaver.error.v1",
    mutability: Mutability::ReadOnly,
    async_class: AsyncClass::Synchronous,
    provider_policy: ProviderPolicy::Auto,
    daemon_domain: "observe",
    daemon_operation: "find-references",
    examples: &["weaver references list --uri file:///src/main.rs --position 10:5"],
    skill_refs: &["code-reading-loop"],
};

/// Pilot read-only records owned by the local adapter.
///
/// Temporary adapter: replaced by generated OrthoConfig 6.1 metadata once the
/// same records can feed routing, context, and skills.
pub(crate) const READ_ONLY_COMMANDS: &[CommandSurfaceRecord] = &[DEFINITIONS_GET, REFERENCES_LIST];

/// Finds a read-only resource command by canonical path and verb.
///
/// Temporary adapter: replaced by the OrthoConfig 6.1 generated command
/// registry lookup.
pub(crate) fn find_read_only_command(
    resource_path: &[&str],
    verb: &str,
) -> Option<&'static CommandSurfaceRecord> {
    READ_ONLY_COMMANDS
        .iter()
        .find(|record| record.resource_path == resource_path && record.verb == verb)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the temporary command-surface metadata adapter.

    use super::*;

    #[test]
    fn pilot_records_cover_definition_and_reference_capabilities() {
        let capabilities: Vec<&str> = READ_ONLY_COMMANDS
            .iter()
            .map(|record| record.capability_id)
            .collect();

        assert_eq!(capabilities, vec!["definition.get", "references.list"]);
    }

    #[test]
    fn definitions_record_maps_to_existing_daemon_operation() {
        assert_eq!(DEFINITIONS_GET.resource_path, ["definitions"]);
        assert_eq!(DEFINITIONS_GET.verb, "get");
        assert_eq!(DEFINITIONS_GET.selector_forms, [SelectorForm::Position]);
        assert_eq!(DEFINITIONS_GET.daemon_domain, "observe");
        assert_eq!(DEFINITIONS_GET.daemon_operation, "get-definition");
        assert_eq!(DEFINITIONS_GET.mutability, Mutability::ReadOnly);
        assert_eq!(DEFINITIONS_GET.provider_policy, ProviderPolicy::Auto);
    }
}
