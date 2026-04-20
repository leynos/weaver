//! Locale identifier validation for the shared Weaver configuration contract.
//!
//! The roadmap requires `--locale` to be a real configuration field now, but
//! the wider CLI localisation bootstrap remains future work. This wrapper keeps
//! the current task small by validating BCP 47 tags at the config boundary
//! while exposing a serializable, clap-friendly domain type.

use std::fmt;
use std::str::FromStr;

use ortho_config::{LanguageIdentifier, langid};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Validated locale identifier stored in Weaver configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale(LanguageIdentifier);

/// Error returned when a locale string is not a valid BCP 47 identifier.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("invalid locale `{input}`")]
pub struct LocaleParseError {
    input: String,
}

impl Locale {
    /// Returns the built-in fallback locale.
    #[must_use]
    pub fn en_us() -> Self {
        Self(langid!("en-US"))
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self::en_us()
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Locale {
    type Err = LocaleParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<LanguageIdentifier>()
            .map(Self)
            .map_err(|_| Self::Err {
                input: value.to_string(),
            })
    }
}

impl Serialize for Locale {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Locale {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<Self>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Locale, LocaleParseError};
    use proptest::prelude::*;

    #[test]
    fn parses_valid_locale() {
        let locale = "en-GB".parse::<Locale>().expect("valid locale");
        assert_eq!(locale.to_string(), "en-GB");
    }

    #[test]
    fn rejects_invalid_locale() {
        let error = "not a locale"
            .parse::<Locale>()
            .expect_err("invalid locale should fail");
        assert_eq!(error.to_string(), "invalid locale `not a locale`");
    }

    prop_compose! {
        fn ascii_string()(bytes in proptest::collection::vec(0u8..=0x7f, 0..24)) -> String {
            bytes.into_iter().map(char::from).collect()
        }
    }

    prop_compose! {
        fn string_with_space_or_control()(
            prefix in proptest::collection::vec(0x21u8..=0x7eu8, 0..12),
            separator in prop_oneof![
                Just(' '),
                (0u8..=0x1f).prop_map(char::from),
                Just('\u{007f}'),
            ],
            suffix in proptest::collection::vec(0x21u8..=0x7eu8, 0..12),
        ) -> String {
            prefix
                .into_iter()
                .map(char::from)
                .chain(std::iter::once(separator))
                .chain(suffix.into_iter().map(char::from))
                .collect()
        }
    }

    proptest! {
        #[test]
        fn locale_display_round_trips_ascii_inputs_when_parsing_succeeds(input in ascii_string()) {
            if let Ok(locale) = input.parse::<Locale>() {
                let displayed = locale.to_string();
                let reparsed = displayed.parse::<Locale>().expect("display output should parse");
                prop_assert_eq!(reparsed.to_string(), displayed);
                prop_assert_eq!(reparsed, locale);
            }
        }

        #[test]
        fn locale_rejects_strings_with_spaces_or_control_chars(input in string_with_space_or_control()) {
            let error = input.parse::<Locale>().expect_err("invalid locale should fail");
            let LocaleParseError { .. } = error;
        }
    }
}
