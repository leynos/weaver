//! Reading Cargo's `OUT_DIR` through an injected environment reader.

use std::env::VarError;

use camino::Utf8PathBuf;
use mockable::Env;

/// Warning recorded when `OUT_DIR` exists but is not valid Unicode.
const NON_UTF8_OUT_DIR_WARNING: &str = "OUT_DIR was non-UTF-8; skipping OUT_DIR staging";

/// Reads `OUT_DIR` when Cargo supplies it as valid UTF-8.
///
/// `raw` is used rather than `string` so a value that exists but is not valid
/// Unicode stays distinguishable from an unset one. An unset variable is the
/// ordinary case and yields `None` silently; a non-Unicode value yields `None`
/// and pushes a warning onto `warnings`.
///
/// # Examples
/// ```no_run
/// use mockable::DefaultEnv;
/// use weaver_build_util::out_dir_from_env;
///
/// let mut warnings = Vec::new();
/// let out_dir = out_dir_from_env(&DefaultEnv, &mut warnings);
///
/// for warning in warnings {
///     println!("cargo:warning={warning}");
/// }
/// # let _ = out_dir;
/// ```
pub fn out_dir_from_env(env: &impl Env, warnings: &mut Vec<String>) -> Option<Utf8PathBuf> {
    match env.raw("OUT_DIR") {
        Ok(path) => Some(Utf8PathBuf::from(path)),
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => {
            warnings.push(NON_UTF8_OUT_DIR_WARNING.to_owned());
            None
        }
    }
}
