//! Build script: generate a minimal weaverd man page for packaging.

use std::{env, path::PathBuf};
use weaver_build_util::{manual_date_from_env, out_dir_for_target_profile, write_man_page};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");

    let binary_name = env::var("CARGO_BIN_NAME")
        .or_else(|_| env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| "weaverd".into());
    let version = env::var("CARGO_PKG_VERSION").map_err(
        |_| "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.",
    )?;

    let mut warnings = Vec::new();
    let date = manual_date_from_env(&mut warnings);
    for warning in warnings {
        println!("cargo:warning={warning}");
    }

    let title = binary_name.to_uppercase();
    let man_page = format!(
        ".TH \"{title}\" \"1\" \"{date}\" \"{binary_name} {version}\" \"Weaver Daemon\"\n\
.SH NAME\n\
{binary_name} \\- Weaver daemon\n\
.SH SYNOPSIS\n\
.B {binary_name}\n\
.SH DESCRIPTION\n\
Weaverd runs the Weaver background service that accepts JSONL commands and\n\
coordinates language tooling.\n"
    );
    let page_name = format!("{binary_name}.1");

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    // Man page generation is pure file output, so it works during cross-compilation.
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    let out_dir_env = env::var_os("OUT_DIR").map(PathBuf::from);
    let out_dir = out_dir_for_target_profile(&target, &profile, out_dir_env.as_deref());
    write_man_page(man_page.as_bytes(), &out_dir, &page_name)?;

    // Also write to OUT_DIR if available for build script consumers.
    if let Some(extra_dir) = env::var_os("OUT_DIR") {
        let extra_dir_path = PathBuf::from(extra_dir);
        if let Err(err) = write_man_page(man_page.as_bytes(), &extra_dir_path, &page_name) {
            println!(
                "cargo:warning=Failed to stage manual page in OUT_DIR ({}): {err}",
                extra_dir_path.display()
            );
        }
    }

    Ok(())
}
