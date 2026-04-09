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

# After-help catalogue shown by 'weaver --help'.
weaver-after-help-header = Domains and operations:
weaver-after-help-observe-heading = observe — Query code structure and relationships
weaver-after-help-observe-get-definition = get-definition
weaver-after-help-observe-find-references = find-references
weaver-after-help-observe-grep = grep
weaver-after-help-observe-diagnostics = diagnostics
weaver-after-help-observe-call-hierarchy = call-hierarchy
weaver-after-help-observe-get-card = get-card
weaver-after-help-act-heading = act — Perform code modifications
weaver-after-help-act-rename-symbol = rename-symbol
weaver-after-help-act-apply-edits = apply-edits
weaver-after-help-act-apply-patch = apply-patch
weaver-after-help-act-apply-rewrite = apply-rewrite
weaver-after-help-act-refactor = refactor
weaver-after-help-verify-heading = verify — Validate code correctness
weaver-after-help-verify-diagnostics = diagnostics
weaver-after-help-verify-syntax = syntax
