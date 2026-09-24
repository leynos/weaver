//! Build script: generate a minimal weaverd man page for packaging.

use camino::Utf8PathBuf;
use ortho_config::{EnvSource, ProcessEnv};
use weaver_build_util::{manual_date, out_dir_for_target_profile, write_man_page};

/// Read Cargo's output directory, warning when its path is not Unicode.
fn cargo_out_dir(environment: &impl EnvSource) -> Option<Utf8PathBuf> {
    let path = environment.get("OUT_DIR")?;
    let Some(utf8) = path.to_str() else {
        println!("cargo:warning=OUT_DIR was non-UTF-8; skipping OUT_DIR staging");
        return None;
    };
    Some(Utf8PathBuf::from(utf8))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let environment = ProcessEnv;
    let env_value = |key| {
        environment
            .get(key)
            .and_then(|value| value.into_string().ok())
    };
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-env-changed=OUT_DIR");

    let binary_name = env_value("CARGO_BIN_NAME")
        .or_else(|| env_value("CARGO_PKG_NAME"))
        .unwrap_or_else(|| "weaverd".into());
    let version = env_value("CARGO_PKG_VERSION")
        .ok_or("CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.")?;

    let mut warnings = Vec::new();
    let source_date_epoch = env_value("SOURCE_DATE_EPOCH");
    let date = manual_date(source_date_epoch.as_deref(), &mut warnings);
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
    let target = env_value("TARGET").unwrap_or_else(|| "unknown-target".into());
    let profile = env_value("PROFILE").unwrap_or_else(|| "unknown-profile".into());
    let out_dir_env = cargo_out_dir(&environment);
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
