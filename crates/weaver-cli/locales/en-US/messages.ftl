# Bare-invocation help block shown when weaver is run without arguments.
weaver-bare-help-command-domain-required = command domain must be provided
weaver-bare-help-usage = Usage: weaver <DOMAIN> <OPERATION> [ARG]...
weaver-bare-help-header = Domains:
weaver-bare-help-domain-observe = observe   Query code structure and relationships
weaver-bare-help-domain-act = act       Perform code modifications
weaver-bare-help-domain-verify = verify    Validate code correctness
weaver-bare-help-pointer = Run 'weaver --help' for more information.

# Preflight domain guidance for missing operations and unknown domain validation.
# Includes messages for weaver-domain-guidance-missing-operation-error,
# weaver-domain-guidance-unknown-domain-error, weaver-domain-guidance-valid-domains,
# and weaver-domain-guidance-did-you-mean-domain.
weaver-domain-guidance-missing-operation-error =
    operation required for domain '{$domain}'
weaver-domain-guidance-unknown-domain-error =
    unknown domain '{$domain}'
weaver-domain-guidance-available-operations = Available operations:
weaver-domain-guidance-valid-domains = Valid domains: {$domains}
weaver-domain-guidance-did-you-mean-domain =
    Did you mean '{$suggested_domain}'?
weaver-domain-guidance-help-hint =
    Run 'weaver {$domain} {$hint_operation} --help' for operation details.
weaver-domain-guidance-help-hint-unknown-domain =
    Run 'weaver {$hint_domain} {$hint_operation} --help' for operation details.

# Recursive command metadata consumed by help and manpage rendering.
weaver-command-root = Semantic code intelligence tool for observing, acting on, and verifying code
weaver-command-definitions = Query symbol definitions
weaver-command-definitions-get = Returns the definition location for a source position
weaver-command-definitions-get-uri = The document URI containing the reference position
weaver-command-definitions-get-position = The 1-indexed line:column position to resolve
weaver-command-daemon = Runs daemon lifecycle commands
weaver-command-daemon-start = Starts the daemon and waits for readiness
weaver-command-daemon-stop = Stops the daemon gracefully
weaver-command-daemon-status = Prints daemon health information
weaver-command-domain-operation = Passes a legacy domain and operation to the daemon
weaver-command-domain-observe = Query code structure and relationships
weaver-command-domain-act = Perform code modifications
weaver-command-domain-verify = Validate code correctness
weaver-doc-heading-name = NAME
weaver-doc-heading-synopsis = SYNOPSIS
weaver-doc-heading-description = DESCRIPTION
weaver-doc-heading-options = OPTIONS
weaver-doc-heading-environment = ENVIRONMENT
weaver-doc-heading-files = FILES
weaver-doc-heading-precedence = PRECEDENCE
weaver-doc-heading-exit-status = EXIT STATUS
weaver-doc-heading-examples = EXAMPLES
weaver-doc-heading-see-also = SEE ALSO
weaver-doc-heading-commands = COMMANDS
