//! Tests for the environment-reading build helpers.
//!
//! Both helpers read through an injected reader, so each case states the
//! variable it exercises instead of depending on the build's own environment.

use std::{env::VarError, ffi::OsString};

use camino::Utf8PathBuf;
use mockable::MockEnv;

use super::{manual_date_from_env, out_dir_from_env};

/// Builds a reader answering `SOURCE_DATE_EPOCH` with `value`.
fn epoch_env(value: Option<&'static str>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_string()
        .withf(|key| key == "SOURCE_DATE_EPOCH")
        .returning(move |_| value.map(str::to_owned));
    env
}

/// Builds a reader answering `OUT_DIR` with `answer`.
fn out_dir_env(answer: fn() -> Result<String, VarError>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|key| key == "OUT_DIR")
        .returning(move |_| answer());
    env
}

#[test]
fn a_valid_epoch_gives_its_date_without_warnings() {
    let mut warnings = Vec::new();

    let date = manual_date_from_env(&epoch_env(Some("86400")), &mut warnings);

    assert_eq!(date, "1970-01-02");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn an_unset_epoch_gives_the_fallback_date_without_warnings() {
    let mut warnings = Vec::new();

    let date = manual_date_from_env(&epoch_env(None), &mut warnings);

    assert_eq!(date, "1970-01-01");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn an_unusable_epoch_gives_the_fallback_date_and_a_warning() {
    let mut warnings = Vec::new();

    let date = manual_date_from_env(&epoch_env(Some("yesterday")), &mut warnings);

    assert_eq!(date, "1970-01-01");
    assert_eq!(
        warnings,
        [
            "Invalid SOURCE_DATE_EPOCH 'yesterday'; expected integer seconds since Unix epoch; \
             falling back to 1970-01-01"
        ]
    );
}

#[test]
fn a_valid_out_dir_is_returned_without_warnings() {
    let mut warnings = Vec::new();

    let out_dir = out_dir_from_env(
        &out_dir_env(|| Ok(String::from("/work/target/out"))),
        &mut warnings,
    );

    assert_eq!(out_dir, Some(Utf8PathBuf::from("/work/target/out")));
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn an_unset_out_dir_gives_none_without_warnings() {
    let mut warnings = Vec::new();

    let out_dir = out_dir_from_env(&out_dir_env(|| Err(VarError::NotPresent)), &mut warnings);

    assert_eq!(out_dir, None);
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn a_non_unicode_out_dir_gives_none_and_a_warning() {
    let mut warnings = Vec::new();

    let out_dir = out_dir_from_env(
        &out_dir_env(|| Err(VarError::NotUnicode(OsString::from("out")))),
        &mut warnings,
    );

    assert_eq!(out_dir, None);
    assert_eq!(
        warnings,
        ["OUT_DIR was non-UTF-8; skipping OUT_DIR staging"]
    );
}
