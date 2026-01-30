//! Build script: generate a minimal weaverd man page for packaging.

use std::{env, fs, io, path::PathBuf};
use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_DATE: &str = "1970-01-01";

fn manual_date() -> String {
    let Ok(raw) = env::var("SOURCE_DATE_EPOCH") else {
        return FALLBACK_DATE.into();
    };

    let Ok(ts) = raw.parse::<i64>() else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; expected integer seconds since \
             Unix epoch; falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    let Ok(dt) = OffsetDateTime::from_unix_timestamp(ts) else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; not a valid Unix timestamp; \
             falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    dt.format(&Iso8601::DATE).unwrap_or_else(|_| {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; formatting failed; falling back \
             to {FALLBACK_DATE}"
        );
        FALLBACK_DATE.into()
    })
}

/// Derive the workspace target directory from OUT_DIR.
///
/// OUT_DIR has the structure: `{workspace_root}/target/{profile}/build/{crate}-{hash}/out`
/// We navigate up to find the `target` directory, which is the workspace target root.
fn workspace_target_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    // out -> {crate}-{hash} -> build -> {profile} -> target
    out_dir
        .parent()?
        .parent()?
        .parent()?
        .parent()
        .map(PathBuf::from)
}

fn out_dir_for_target_profile() -> PathBuf {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());

    // Use workspace target directory if available, otherwise fall back to relative path
    let base = workspace_target_dir().unwrap_or_else(|| PathBuf::from("target"));
    base.join(format!("generated-man/{target}/{profile}"))
}

fn write_man_page(data: &[u8], dir: &std::path::Path, page_name: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let destination = dir.join(page_name);
    let tmp = dir.join(format!("{page_name}.tmp"));
    fs::write(&tmp, data)?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&tmp, &destination)?;
    Ok(destination)
}

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

    let title = binary_name.to_uppercase();
    let date = manual_date();
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
    let out_dir = out_dir_for_target_profile();
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
