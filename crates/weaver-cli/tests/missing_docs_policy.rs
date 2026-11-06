//! Compile-time regression checks for the workspace lint inheritance policy.
//!
//! Cargo applies `[workspace.lints]` to a package only when that package's
//! manifest opts in with `[lints] workspace = true`. A member that forgets the
//! opt-in silently loses the entire policy, so the opt-in is pinned here for
//! every member that carries it, alongside the workspace rule it inherits.

/// Members that must inherit the workspace lint table, with their manifests.
const INHERITING_MANIFESTS: [(&str, &str); 5] = [
    ("weaver-cli", include_str!("../Cargo.toml")),
    (
        "weaver-config",
        include_str!("../../weaver-config/Cargo.toml"),
    ),
    (
        "weaver-lsp-host",
        include_str!("../../weaver-lsp-host/Cargo.toml"),
    ),
    (
        "weaver-sandbox",
        include_str!("../../weaver-sandbox/Cargo.toml"),
    ),
    ("weaverd", include_str!("../../weaverd/Cargo.toml")),
];

/// The workspace manifest that defines the inherited lint table.
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

/// Parses `source` as TOML, naming `package_name` in the failure message.
fn parse_manifest(package_name: &str, source: &str) -> toml::Value {
    toml::from_str(source)
        .unwrap_or_else(|error| panic!("{package_name} manifest is invalid TOML: {error}"))
}

/// Looks up the string at `path` in `manifest`, if the path resolves to one.
#[must_use]
fn string_at<'a>(manifest: &'a toml::Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(manifest, |node, key| node.get(key))
        .and_then(toml::Value::as_str)
}

// Pin the opt-in for every member the workspace policy is meant to cover.
#[test]
fn every_member_inherits_the_workspace_lint_table() {
    for (package_name, source) in INHERITING_MANIFESTS {
        let manifest = parse_manifest(package_name, source);
        let inherits = manifest
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(toml::Value::as_bool);

        assert_eq!(
            inherits,
            Some(true),
            "{package_name} must opt in with `[lints] workspace = true`"
        );
    }
}

// The inherited table is only meaningful while it still denies missing docs.
#[test]
fn workspace_denies_undocumented_public_items() {
    let manifest = parse_manifest("workspace", WORKSPACE_MANIFEST);

    assert_eq!(
        string_at(&manifest, &["workspace", "lints", "rust", "missing_docs"]),
        Some("deny"),
        "the workspace lint table must deny undocumented public items"
    );
}

// Pin that documented public items pass and undocumented ones are rejected.
#[test]
fn missing_docs_ui_contract() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/missing_docs_documented.rs");
    cases.compile_fail("tests/ui/missing_docs_undocumented.rs");
}
