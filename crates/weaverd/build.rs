//! Build script: generate a minimal weaverd man page for packaging.

use std::env::VarError;

use camino::Utf8PathBuf;
use mockable::{DefaultEnv, Env};
use weaver_build_util::{manual_date_from_env, out_dir_for_target_profile, write_man_page};

/// Reads `OUT_DIR` when Cargo supplies it as valid UTF-8.
///
/// `raw` is used rather than `string` so a value that exists but is not valid
/// Unicode stays distinguishable from an unset one, and keeps its warning.
fn cargo_out_dir(env: &impl Env) -> Option<Utf8PathBuf> {
    match env.raw("OUT_DIR") {
        Ok(path) => Some(Utf8PathBuf::from(path)),
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => {
            println!("cargo:warning=OUT_DIR was non-UTF-8; skipping OUT_DIR staging");
            None
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");

    // All environment reads in this build script go through one injected
    // reader, so the workspace's ban on ambient `std::env` access applies to
    // build scripts as well as runtime code.
    let env = DefaultEnv;

    let binary_name = env
        .string("CARGO_BIN_NAME")
        .or_else(|| env.string("CARGO_PKG_NAME"))
        .unwrap_or_else(|| "weaverd".into());
    let version = env.string("CARGO_PKG_VERSION").ok_or_else(|| {
        "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it."
    })?;

    let mut warnings = Vec::new();
    let date = manual_date_from_env(&env, &mut warnings);
    for warning in warnings {
        println!("cargo:warning={warning}");
    }

    let title = binary_name.to_uppercase();
    let man_page = format!(
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
    );
    let page_name = format!("{binary_name}.1");

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    // Man page generation is pure file output, so it works during cross-compilation.
    let target = env
        .string("TARGET")
        .unwrap_or_else(|| "unknown-target".into());
    let profile = env
        .string("PROFILE")
        .unwrap_or_else(|| "unknown-profile".into());
    let out_dir_env = cargo_out_dir(&env);
    let out_dir = out_dir_for_target_profile(&target, &profile, out_dir_env.as_deref());
    write_man_page(man_page.as_bytes(), &out_dir, &page_name)?;

    // Also write to OUT_DIR if available for build script consumers.
    if let Some(ref extra_dir_path) = out_dir_env
        && let Err(err) = write_man_page(man_page.as_bytes(), extra_dir_path, &page_name)
    {
        println!("cargo:warning=Failed to stage manual page in OUT_DIR ({extra_dir_path}): {err}");
    }

    Ok(())
}
