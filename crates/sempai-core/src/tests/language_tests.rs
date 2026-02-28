//! Tests for the [`Language`] enum.

use rstest::rstest;

use crate::Language;

#[rstest]
#[case::rust(Language::Rust, "rust")]
#[case::python(Language::Python, "python")]
#[case::typescript(Language::TypeScript, "type_script")]
#[case::go(Language::Go, "go")]
#[case::hcl(Language::Hcl, "hcl")]
fn language_display(#[case] lang: Language, #[case] expected: &str) {
    assert_eq!(format!("{lang}"), expected);
}

#[rstest]
#[case::rust(Language::Rust, "\"rust\"")]
#[case::python(Language::Python, "\"python\"")]
#[case::typescript(Language::TypeScript, "\"type_script\"")]
#[case::go(Language::Go, "\"go\"")]
#[case::hcl(Language::Hcl, "\"hcl\"")]
fn language_serde_round_trip(#[case] lang: Language, #[case] expected_json: &str) {
    let json = serde_json::to_string(&lang).expect("serialize");
    assert_eq!(json, expected_json);

    let deserialized: Language = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, lang);
}

#[test]
fn language_copy_and_eq() {
    let a = Language::Rust;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn language_hash_is_consistent() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(Language::Python);
    set.insert(Language::Python);
    assert_eq!(set.len(), 1);
}
