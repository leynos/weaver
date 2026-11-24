//! Build script: generate a minimal weaverd man page for packaging.

use camino::Utf8PathBuf;
use mockable::{DefaultEnv, Env};
use weaver_build_util::{
    manual_date_from_env,
    out_dir_for_target_profile,
    out_dir_from_env,
    write_man_page,
};

/// Environment variables whose change must re-run this script.
const RERUN_VARIABLES: [&str; 6] = [
    "CARGO_PKG_VERSION",
    "CARGO_PKG_NAME",
    "CARGO_BIN_NAME",
    "SOURCE_DATE_EPOCH",
    "TARGET",
    "PROFILE",
];

/// Values substituted into the man page template.
struct ManPageFields<'a> {
    /// Binary name shown in the title and synopsis.
    binary_name: &'a str,
    /// Package version shown in the footer.
    version: &'a str,
    /// ISO date shown in the header.
    date: &'a str,
}

/// Tells Cargo which environment variables invalidate this script's output.
fn emit_rerun_directives() {
    for variable in RERUN_VARIABLES {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

/// Forwards each collected warning to Cargo.
fn emit_warnings(warnings: Vec<String>) {
    for warning in warnings {
        println!("cargo:warning={warning}");
    }
}

/// Picks the binary name Cargo reports, falling back to `weaverd`.
fn binary_name(env: &impl Env) -> String {
    env.string("CARGO_BIN_NAME")
        .or_else(|| env.string("CARGO_PKG_NAME"))
        .unwrap_or_else(|| "weaverd".into())
}

/// Reads the package version, which Cargo always supplies to build scripts.
fn package_version(env: &impl Env) -> Result<String, &'static str> {
    env.string("CARGO_PKG_VERSION")
        .ok_or("CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.")
}

/// Reads `name` from the environment, using `fallback` when it is unset.
fn string_or(env: &impl Env, name: &str, fallback: &str) -> String {
    env.string(name).unwrap_or_else(|| fallback.into())
}

/// Renders the man page text for `fields`.
fn render_man_page(fields: &ManPageFields<'_>) -> String {
    let ManPageFields {
        binary_name,
        version,
        date,
    } = fields;
    let title = binary_name.to_uppercase();
    format!(
        concat!(
            ".TH \"{title}\" \"1\" \"{date}\" \"{binary_name} {version}\" \"Weaver Daemon\"\n",
            ".SH NAME\n",
            "{binary_name} \\- Weaver daemon\n",
            ".SH SYNOPSIS\n",
            ".B {binary_name}\n",
            ".SH DESCRIPTION\n",
            "Weaverd runs the Weaver background service that accepts JSONL commands and\n",
            "coordinates language tooling.\n",
        ),
        title = title,
        date = date,
        binary_name = binary_name,
        version = version
    )
}

/// Also stages the page in `OUT_DIR` for build script consumers, warning on failure.
fn stage_in_out_dir(man_page: &str, out_dir: Option<&Utf8PathBuf>, page_name: &str) {
    if let Some(extra_dir_path) = out_dir
        && let Err(err) = write_man_page(man_page.as_bytes(), extra_dir_path, page_name)
    {
        println!("cargo:warning=Failed to stage manual page in OUT_DIR ({extra_dir_path}): {err}");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    emit_rerun_directives();

    // All environment reads in this build script go through one injected
    // reader, so the workspace's ban on ambient `std::env` access applies to
    // build scripts as well as runtime code.
    let env = DefaultEnv;

    let binary_name = binary_name(&env);
    let version = package_version(&env)?;
    let mut warnings = Vec::new();
    let date = manual_date_from_env(&env, &mut warnings);
    let man_page = render_man_page(&ManPageFields {
        binary_name: &binary_name,
        version: &version,
        date: &date,
    });
    let page_name = format!("{binary_name}.1");

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    // Man page generation is pure file output, so it works during cross-compilation.
    let target = string_or(&env, "TARGET", "unknown-target");
    let profile = string_or(&env, "PROFILE", "unknown-profile");
    let out_dir_env = out_dir_from_env(&env, &mut warnings);
    emit_warnings(warnings);
    let out_dir = out_dir_for_target_profile(&target, &profile, out_dir_env.as_deref());
    write_man_page(man_page.as_bytes(), &out_dir, &page_name)?;

    stage_in_out_dir(&man_page, out_dir_env.as_ref(), &page_name);

    Ok(())
}
