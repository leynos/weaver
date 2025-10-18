//! Configuration loading helpers for the Weaver CLI.
//!
//! The logic here filters CLI arguments destined for `ortho-config` so the
//! loader only receives supported flags while the main runtime can operate on
//! the remaining command tokens.

use std::ffi::{OsStr, OsString};

use weaver_config::Config;

use crate::AppError;

pub(crate) trait ConfigLoader {
    /// Loads configuration for the CLI.
    ///
    /// # Flag Ordering
    ///
    /// Configuration flags (listed in `CONFIG_CLI_FLAGS`) must appear before any
    /// command tokens. Configuration flags appearing after positional arguments
    /// will be treated as command arguments and passed to the daemon rather than
    /// configuring the CLI.
    fn load(&self, args: &[OsString]) -> Result<Config, AppError>;
}

pub(crate) struct OrthoConfigLoader;

#[derive(Debug, Clone, Copy)]
enum FlagAction {
    Include { needs_value: bool },
    Skip,
}

impl ConfigLoader for OrthoConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError> {
        Config::load_from_iter(args.iter().cloned()).map_err(AppError::LoadConfiguration)
    }
}

impl OrthoConfigLoader {
    fn process_config_flag(argument: &OsStr) -> FlagAction {
        let argument_text = argument.to_string_lossy();
        if !argument_text.starts_with("--") {
            return FlagAction::Skip;
        }

        let mut flag_parts = argument_text.splitn(2, '=');
        let flag = flag_parts.next().unwrap();
        let has_inline_value = flag_parts.next().is_some();

        if super::CONFIG_CLI_FLAGS.contains(&flag) {
            return FlagAction::Include {
                needs_value: !has_inline_value,
            };
        }

        FlagAction::Skip
    }
}

pub(crate) struct ConfigArgumentSplit {
    pub(crate) config_arguments: Vec<OsString>,
    pub(crate) command_start: usize,
}

pub(crate) fn split_config_arguments(args: &[OsString]) -> ConfigArgumentSplit {
    if args.is_empty() {
        return ConfigArgumentSplit {
            config_arguments: Vec::new(),
            command_start: 0,
        };
    }

    let mut filtered: Vec<OsString> = Vec::new();
    filtered.push(args[0].clone());

    let mut command_start = 1usize;
    let mut index = 1usize;
    let mut pending_values = 0usize;

    while index < args.len() {
        let argument = &args[index];
        if pending_values > 0 {
            filtered.push(argument.clone());
            pending_values -= 1;
            index += 1;
            command_start = index;
            continue;
        }

        match OrthoConfigLoader::process_config_flag(argument.as_os_str()) {
            FlagAction::Include { needs_value } => {
                filtered.push(argument.clone());
                index += 1;
                command_start = index;
                if needs_value {
                    pending_values = 1;
                }
            }
            FlagAction::Skip => {
                break;
            }
        }
    }

    ConfigArgumentSplit {
        config_arguments: filtered,
        command_start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn inline_value_flags_do_not_need_follow_up_value() {
        let result = OrthoConfigLoader::process_config_flag(OsStr::new("--log-filter=debug"));
        match result {
            FlagAction::Include { needs_value } => assert!(!needs_value),
            _ => panic!("expected include for known inline flag"),
        }
    }

    #[test]
    fn separate_value_flags_consume_following_argument() {
        let result = OrthoConfigLoader::process_config_flag(OsStr::new("--log-filter"));
        match result {
            FlagAction::Include { needs_value } => assert!(needs_value),
            _ => panic!("expected include for known separated flag"),
        }
    }

    #[test]
    fn non_flag_arguments_signal_stop() {
        let result = OrthoConfigLoader::process_config_flag(OsStr::new("observe"));
        assert!(matches!(result, FlagAction::Skip), "should skip");
    }

    #[test]
    fn unknown_flags_are_skipped() {
        let result = OrthoConfigLoader::process_config_flag(OsStr::new("--unknown"));
        assert!(matches!(result, FlagAction::Skip), "should skip");
    }
}
