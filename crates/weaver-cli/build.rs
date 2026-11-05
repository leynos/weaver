//! Build script: generate the CLI manual page into target/generated-man/<target>/<profile> for
//! release packaging.

use camino::Utf8PathBuf;
use clap_mangen::Man;
use mockable::{DefaultEnv, Env};
use weaver_build_util::{manual_date_from_env, out_dir_for_target_profile, write_man_page};

#[path = "src/cli.rs"]
mod cli;
// The build script cannot depend on its own library, so it includes the same
// pure command metadata modules used by runtime help. Keep this list bounded.
#[path = "src/command_ir/mod.rs"]
mod command_ir;
#[path = "src/command_surface/tree.rs"]
mod command_tree;
#[path = "src/help.rs"]
mod help;

/// Generates the package manual page from the shared augmented help command.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reference the runtime help writer so the build script fails to compile
    // if its signature changes, without executing it at build time.
    const _: fn(&[std::ffi::OsString], &mut Vec<u8>) -> Result<(), help::HelpWriteError> =
        help::write_help_for_args::<Vec<u8>>;

    // Regenerate the manual page when the CLI or metadata changes.
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=src/command_surface/mod.rs");
    println!("cargo:rerun-if-changed=src/command_surface/tree.rs");
    println!("cargo:rerun-if-changed=src/command_ir/mod.rs");
    println!("cargo:rerun-if-changed=src/help.rs");
    println!("cargo:rerun-if-changed=src/help_metadata.rs");
    println!("cargo:rerun-if-changed=src/locales/en-US/messages.ftl");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");

    // All environment reads in this build script go through one injected
    // reader, so the workspace's ban on ambient `std::env` access applies to
    // build scripts as well as runtime code.
    let env = DefaultEnv;

    // The top-level page documents the entire command interface.
    let cmd = help::try_command()?;
    let default_name = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_owned();
    let binary_name = env.string("CARGO_BIN_NAME").unwrap_or(default_name);

    let version = env.string("CARGO_PKG_VERSION").ok_or_else(|| {
        "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it."
    })?;

    let mut warnings = Vec::new();
    let date = manual_date_from_env(&env, &mut warnings);
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
    let target = env
        .string("TARGET")
        .unwrap_or_else(|| "unknown-target".into());
    let profile = env
        .string("PROFILE")
        .unwrap_or_else(|| "unknown-profile".into());
    let out_dir_env = env
        .path_buf("OUT_DIR")
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok());
    let out_dir = out_dir_for_target_profile(&target, &profile, out_dir_env.as_deref());
    write_man_page(&buf, &out_dir, &page_name)?;

    Ok(())
}
