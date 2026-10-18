//! Compile-time regression checks for the workspace's package-level
//! `missing_docs` policy.

const PACKAGE_MANIFESTS: [(&str, &str); 4] = [
    ("weaver-cli", include_str!("../Cargo.toml")),
    (
        "weaver-config",
        include_str!("../../weaver-config/Cargo.toml"),
    ),
    (
        "weaver-sandbox",
        include_str!("../../weaver-sandbox/Cargo.toml"),
    ),
    ("weaverd", include_str!("../../weaverd/Cargo.toml")),
];

// Keep the four package manifests aligned with the public documentation rule.
#[test]
fn affected_manifests_deny_missing_public_docs() {
    for (package_name, source) in PACKAGE_MANIFESTS {
        let manifest = toml::from_str::<toml::Value>(source)
            .unwrap_or_else(|error| panic!("{package_name} manifest is invalid TOML: {error}"));
        let level = manifest
            .get("lints")
            .and_then(|lints| lints.get("rust"))
            .and_then(|rust| rust.get("missing_docs"))
            .and_then(toml::Value::as_str);

        assert_eq!(
            level,
            Some("deny"),
            "{package_name} must deny undocumented public items"
        );
    }
}

// Pin that documented public items pass and undocumented ones are rejected.
#[test]
fn missing_docs_ui_contract() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/missing_docs_documented.rs");
    cases.compile_fail("tests/ui/missing_docs_undocumented.rs");
}
