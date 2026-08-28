//! Defines Weaver's canonical, framework-independent command-surface tree.
//!
//! [`crate::command_ir`] projects this tree into recursive `DocMetadata`, while
//! runtime help applies that metadata to Clap and `build.rs` reuses the same
//! path for manual-page generation. Keeping these consumers rooted here makes
//! command paths, arguments, and localisation identifiers agree across every
//! rendered documentation surface.

/// One argument accepted by a command-surface node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandArgument {
    /// Long flag name without its leading dashes.
    pub(crate) long: &'static str,
    /// Placeholder displayed for a value-taking flag.
    pub(crate) value_name: Option<&'static str>,
    /// Whether the runtime parser requires this flag.
    pub(crate) required: bool,
    /// Fluent identifier describing the argument.
    pub(crate) help_id: &'static str,
    /// English fallback describing the argument.
    pub(crate) help: &'static str,
}

/// Describes how the runtime parser reaches a command node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandSemantics {
    /// A structured clap subcommand.
    Structured,
    /// The legacy free-text domain and operation passthrough.
    DaemonPassthrough {
        /// Legacy domain and operation spellings accepted by the daemon.
        domains: &'static [DomainOperations],
    },
}

/// Operations accepted beneath one legacy daemon domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainOperations {
    /// Domain spelling accepted by the runtime parser.
    pub domain: &'static str,
    /// Fluent identifier describing the domain.
    pub summary_id: &'static str,
    /// English fallback describing the domain.
    pub summary: &'static str,
    /// Operations accepted within this domain, in display order.
    pub operations: &'static [&'static str],
}

/// One node in Weaver's canonical command surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandNode {
    /// Resource path, excluding the action verb.
    pub(crate) resource_path: &'static [&'static str],
    /// Canonical command label or action verb.
    pub(crate) verb: &'static str,
    /// Fluent identifier describing the node.
    pub(crate) summary_id: &'static str,
    /// English fallback describing the node.
    pub(crate) summary: &'static str,
    /// Arguments accepted by the node.
    pub(crate) arguments: &'static [CommandArgument],
    /// Runtime parsing model for the node.
    pub(crate) semantics: CommandSemantics,
    /// Nested command nodes in display order.
    pub(crate) children: &'static [CommandNode],
}

const DEFINITIONS_GET_ARGUMENTS: &[CommandArgument] = &[
    CommandArgument {
        long: "uri",
        value_name: Some("URI"),
        required: true,
        help_id: "weaver-command-definitions-get-uri",
        help: "The document URI containing the reference position",
    },
    CommandArgument {
        long: "position",
        value_name: Some("LINE:COLUMN"),
        required: true,
        help_id: "weaver-command-definitions-get-position",
        help: "The 1-indexed line:column position to resolve",
    },
];

const DEFINITIONS_CHILDREN: &[CommandNode] = &[CommandNode {
    resource_path: &["definitions"],
    verb: "get",
    summary_id: "weaver-command-definitions-get",
    summary: "Returns the definition location for a source position",
    arguments: DEFINITIONS_GET_ARGUMENTS,
    semantics: CommandSemantics::Structured,
    children: &[],
}];

const DAEMON_CHILDREN: &[CommandNode] = &[
    CommandNode {
        resource_path: &["daemon"],
        verb: "start",
        summary_id: "weaver-command-daemon-start",
        summary: "Starts the daemon and waits for readiness",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: &[],
    },
    CommandNode {
        resource_path: &["daemon"],
        verb: "stop",
        summary_id: "weaver-command-daemon-stop",
        summary: "Stops the daemon gracefully",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: &[],
    },
    CommandNode {
        resource_path: &["daemon"],
        verb: "status",
        summary_id: "weaver-command-daemon-status",
        summary: "Prints daemon health information",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: &[],
    },
];

const OBSERVE_OPERATIONS: &[&str] = &[
    "get-definition",
    "find-references",
    "grep",
    "diagnostics",
    "call-hierarchy",
    "get-card",
    "graph-slice",
];

const ACT_OPERATIONS: &[&str] = &[
    "rename-symbol",
    "apply-edits",
    "apply-patch",
    "apply-rewrite",
    "refactor",
];

const VERIFY_OPERATIONS: &[&str] = &["diagnostics", "syntax"];

const DOMAIN_OPERATIONS: &[DomainOperations] = &[
    DomainOperations {
        domain: "observe",
        summary_id: "weaver-command-domain-observe",
        summary: "Query code structure and relationships",
        operations: OBSERVE_OPERATIONS,
    },
    DomainOperations {
        domain: "act",
        summary_id: "weaver-command-domain-act",
        summary: "Perform code modifications",
        operations: ACT_OPERATIONS,
    },
    DomainOperations {
        domain: "verify",
        summary_id: "weaver-command-domain-verify",
        summary: "Validate code correctness",
        operations: VERIFY_OPERATIONS,
    },
];

const ROOT_CHILDREN: &[CommandNode] = &[
    CommandNode {
        resource_path: &["definitions"],
        verb: "definitions",
        summary_id: "weaver-command-definitions",
        summary: "Query symbol definitions",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: DEFINITIONS_CHILDREN,
    },
    CommandNode {
        resource_path: &["daemon"],
        verb: "daemon",
        summary_id: "weaver-command-daemon",
        summary: "Runs daemon lifecycle commands",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: DAEMON_CHILDREN,
    },
    CommandNode {
        resource_path: &[],
        verb: "domain-operation",
        summary_id: "weaver-command-domain-operation",
        summary: "Passes a legacy domain and operation to the daemon",
        arguments: &[],
        semantics: CommandSemantics::DaemonPassthrough {
            domains: DOMAIN_OPERATIONS,
        },
        children: &[],
    },
];

const ROOT: CommandNode = CommandNode {
    resource_path: &[],
    verb: "weaver",
    summary_id: "weaver-command-root",
    summary: "Semantic code intelligence tool for observing, acting on, and verifying code",
    arguments: &[],
    semantics: CommandSemantics::Structured,
    children: ROOT_CHILDREN,
};

/// Returns the canonical root of Weaver's command surface.
pub(crate) const fn root() -> &'static CommandNode { &ROOT }

/// Returns the canonical legacy domain-operation catalogue.
pub const fn domain_operations() -> &'static [DomainOperations] { DOMAIN_OPERATIONS }
