//! Build script: generate the CLI manual page into target/generated-man/<target>/<profile> for
//! release packaging.

use std::env;

use camino::Utf8PathBuf;
use clap::CommandFactory;
use clap_mangen::Man;
use weaver_build_util::{manual_date_from_env, out_dir_for_target_profile, write_man_page};

#[path = "src/cli.rs"]
mod cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Regenerate the manual page when the CLI or metadata changes.
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");

    // The top-level page documents the entire command interface.
    let cmd = cli::Cli::command();
    let default_name = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_owned();
    let binary_name = env::var("CARGO_BIN_NAME").unwrap_or(default_name);

    let version = env::var("CARGO_PKG_VERSION").map_err(|_| {
        "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it."
    })?;

    let mut warnings = Vec::new();
    let date = manual_date_from_env(&mut warnings);
    for warning in warnings {
        println!("cargo:warning={warning}");
    }

    let man = Man::new(cmd)
        .section("1")
        .source(format!("{binary_name} {version}"))
        .date(date);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let page_name = format!("{binary_name}.1");

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    // Man page generation is pure file output, so it works during cross-compilation.
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    let out_dir_env: Option<Utf8PathBuf> =
        env::var_os("OUT_DIR").and_then(|p| p.to_str().map(Utf8PathBuf::from));
    let out_dir = out_dir_for_target_profile(&target, &profile, out_dir_env.as_deref());
    write_man_page(&buf, &out_dir, &page_name)?;

    // Also write to OUT_DIR if available for build script consumers.
    if let Some(extra_dir) = out_dir_env
        && let Err(err) = write_man_page(&buf, &extra_dir, &page_name)
    {
        println!(
            "cargo:warning=Failed to stage manual page in OUT_DIR ({extra_dir}): {err}"
        );
    }

    Ok(())
}
