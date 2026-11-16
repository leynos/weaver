//! Compile-time coverage for the `const fn` accessors on [`Config`].
//!
//! Rust rejects a call to a non-`const` function from inside a `const fn`
//! body. Each wrapper below therefore pins its accessor's const-ness at
//! compile time without needing a value that is constructible in a const
//! context. The wrappers are called from an ordinary test so the workspace's
//! warnings-as-errors gate does not reject them as dead code.

use weaver_config::{Config, Locale, LogFormat, SocketEndpoint};

/// Compiles only while [`Config::daemon_socket`] is a `const fn`.
#[must_use]
const fn daemon_socket_of(config: &Config) -> &SocketEndpoint { config.daemon_socket() }

/// Compiles only while [`Config::log_filter`] is a `const fn`.
#[must_use]
const fn log_filter_of(config: &Config) -> &str { config.log_filter() }

/// Compiles only while [`Config::log_format`] is a `const fn`.
#[must_use]
const fn log_format_of(config: &Config) -> LogFormat { config.log_format() }

/// Compiles only while [`Config::locale`] is a `const fn`.
#[must_use]
const fn locale_of(config: &Config) -> &Locale { config.locale() }

/// Checks that every wrapper agrees with the direct accessor call.
#[test]
fn const_accessors_agree_with_direct_calls() {
    let config = Config::default();

    assert_eq!(daemon_socket_of(&config), config.daemon_socket());
    assert_eq!(log_filter_of(&config), config.log_filter());
    assert_eq!(log_format_of(&config), config.log_format());
    assert_eq!(locale_of(&config), config.locale());
}
