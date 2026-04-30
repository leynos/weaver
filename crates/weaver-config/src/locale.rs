//! Locale identifier validation for the shared Weaver configuration contract.
//!
//! The roadmap requires `--locale` to be a real configuration field now, but
//! the wider CLI localization bootstrap remains future work. This wrapper keeps
//! the current task small by validating BCP 47 tags at the config boundary
//! while exposing a serializable, clap-friendly domain type.

use std::{fmt, str::FromStr};

use icu_locale_core::Locale as IcuLocale;
use ortho_config::{LanguageIdentifier, langid};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Validated locale identifier stored in Weaver configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale(LanguageIdentifier, String);

/// Error returned when a locale string is not a valid BCP 47 identifier.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("invalid locale {input:?}")]
pub struct LocaleParseError {
    input: String,
}

impl Locale {
    /// Returns the built-in fallback locale.
    #[must_use]
    pub fn en_us() -> Self {
        let language_identifier = langid!("en-US");
        Self(language_identifier, "en-US".to_string())
    }
}

impl Default for Locale {
    fn default() -> Self { Self::en_us() }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.1) }
}

impl FromStr for Locale {
    type Err = LocaleParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let locale = value.parse::<IcuLocale>().map_err(|_| Self::Err {
            input: value.to_string(),
        })?;
        let canonical = locale.to_string();
        let language_identifier = locale
            .id
            .to_string()
            .parse::<LanguageIdentifier>()
            .map_err(|_| Self::Err {
                input: value.to_string(),
            })?;
        Ok(Self(language_identifier, canonical))
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
    //! Tests for locale parsing, formatting, and rejection of invalid tags.

    use ortho_config::LanguageIdentifier;
    use proptest::prelude::*;

    use super::{Locale, LocaleParseError};

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
        assert_eq!(error.to_string(), "invalid locale \"not a locale\"");
    }

    mod json_serialization {
        //! Exercise JSON string serialization for [`Locale`].

        use super::Locale;

        #[derive(Clone, Copy)]
        enum JsonLocaleExpectation {
            Valid(&'static str),
            Invalid,
        }

        #[rstest::rstest]
        #[case::serialises_to_json_string("\"fr-FR\"", JsonLocaleExpectation::Valid("fr-FR"))]
        #[case::deserialises_from_json_string("\"de-DE\"", JsonLocaleExpectation::Valid("de-DE"))]
        #[case::round_trip_preserves_canonical_form(
            "\"en-US\"",
            JsonLocaleExpectation::Valid("en-US")
        )]
        #[case::rejects_invalid_json_string("\"not a locale!!!\"", JsonLocaleExpectation::Invalid)]
        #[case::rejects_invalid_locale_json("\"not a locale\"", JsonLocaleExpectation::Invalid)]
        fn locale_json_string_cases(
            #[case] input_json: &str,
            #[case] expected: JsonLocaleExpectation,
        ) {
            match expected {
                JsonLocaleExpectation::Valid(expected_locale) => {
                    let locale: Locale = serde_json::from_str(input_json).expect("deserialise");
                    assert_eq!(locale.to_string(), expected_locale);
                    let json = serde_json::to_string(&locale).expect("serialise");
                    assert_eq!(json, input_json);
                    let roundtripped: Locale = serde_json::from_str(&json).expect("deserialise");
                    assert_eq!(roundtripped.to_string(), expected_locale);
                }
                JsonLocaleExpectation::Invalid => {
                    let result: Result<Locale, _> = serde_json::from_str(input_json);
                    assert!(result.is_err(), "invalid locale must not deserialise");
                }
            }
        }

        #[test]
        fn locale_serialises_to_json_string() {
            let locale = "fr-FR".parse::<Locale>().expect("valid locale");
            let json = serde_json::to_string(&locale).expect("serialise");
            assert_eq!(json, "\"fr-FR\"");
        }

        #[test]
        fn locale_deserialises_from_json_string() {
            let locale: Locale = serde_json::from_str("\"de-DE\"").expect("deserialise");
            assert_eq!(locale.to_string(), "de-DE");
        }

        #[test]
        fn locale_json_round_trip_preserves_canonical_form() {
            let original = "en-US".parse::<Locale>().expect("valid locale");
            let json = serde_json::to_string(&original).expect("serialise");
            let roundtripped: Locale = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(original.to_string(), roundtripped.to_string());
        }

        #[test]
        fn locale_deserialise_rejects_invalid_json_string() {
            let result: Result<Locale, _> = serde_json::from_str("\"not a locale!!!\"");
            assert!(result.is_err(), "invalid locale must not deserialise");
        }
    }

    #[test]
    fn parses_locale_with_unicode_extension() {
        let expected: LanguageIdentifier = "en-US"
            .parse()
            .expect("language identifier from locale base");
        let locale = "en-US-u-ca-gregory"
            .parse::<Locale>()
            .expect("valid locale with Unicode extension");

        assert_eq!(locale.0, expected);
        assert_eq!(locale.to_string(), "en-US-u-ca-gregory");
    }

    #[test]
    fn locale_with_unicode_extension_serialises_round_trips() {
        let expected: LanguageIdentifier = "en-US"
            .parse()
            .expect("language identifier from locale base");
        let original = "en-US-u-ca-gregory"
            .parse::<Locale>()
            .expect("valid locale with Unicode extension");
        let json = serde_json::to_string(&original).expect("serialise");
        let roundtripped: Locale = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(roundtripped, original);
        assert_eq!(roundtripped.0, expected);
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

    prop_compose! {
        fn language_region_tag()(
            language in "[a-z]{2,3}",
            region in "[A-Z]{2}",
        ) -> String {
            format!("{language}-{region}")
        }
    }

    proptest! {
        #[test]
        fn language_region_tags_parse_and_display_canonically(input in language_region_tag()) {
            let locale = input.parse::<Locale>().expect("generated locale should parse");
            prop_assert_eq!(locale.to_string(), input);
        }

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
