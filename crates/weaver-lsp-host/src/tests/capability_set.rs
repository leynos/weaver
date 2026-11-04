//! Unit tests for [`ServerCapabilitySet`] construction and lookup.

use lsp_types::PositionEncodingKind;
use proptest::prelude::*;
use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::{CapabilityKind, CapabilitySource, resolve_capabilities},
    language::Language,
    server::ServerCapabilitySet,
};

/// The capabilities a server can advertise, in construction-bit order.
const ALL_KINDS: [CapabilityKind; 5] = [
    CapabilityKind::Definition,
    CapabilityKind::References,
    CapabilityKind::Diagnostics,
    CapabilityKind::CallHierarchy,
    CapabilityKind::Hover,
];

/// Builds a capability set from a five-bit advertisement mask.
fn set_from_bits(bits: u8) -> ServerCapabilitySet {
    ServerCapabilitySet::new(
        bits & 0b0_0001 != 0,
        bits & 0b0_0010 != 0,
        bits & 0b0_0100 != 0,
    )
    .with_call_hierarchy(bits & 0b0_1000 != 0)
    .with_hover(bits & 0b1_0000 != 0)
}

#[rstest]
#[case(0b0_0000)]
#[case(0b1_1111)]
#[case(0b0_0001)]
#[case(0b0_1010)]
fn capability_set_reports_every_construction_flag(#[case] mask: u8) {
    let set = set_from_bits(mask);

    for (index, kind) in ALL_KINDS.iter().enumerate() {
        let expected = mask & (1u8 << index) != 0;
        assert_eq!(set.supports(*kind), expected, "supports({kind:?})");
    }

    assert_eq!(set.supports_definition(), mask & 0b0_0001 != 0);
    assert_eq!(set.supports_references(), mask & 0b0_0010 != 0);
    assert_eq!(set.supports_diagnostics(), mask & 0b0_0100 != 0);
    assert_eq!(set.supports_call_hierarchy(), mask & 0b0_1000 != 0);
    assert_eq!(set.supports_hover(), mask & 0b1_0000 != 0);
}

#[rstest]
fn capability_set_builders_can_clear_support() {
    let set = ServerCapabilitySet::new(true, true, true)
        .with_call_hierarchy(true)
        .with_hover(true)
        .with_call_hierarchy(false)
        .with_hover(false);

    assert!(!set.supports(CapabilityKind::CallHierarchy));
    assert!(!set.supports(CapabilityKind::Hover));
    assert!(set.supports(CapabilityKind::Definition));
}

#[rstest]
fn capability_set_tracks_the_negotiated_position_encoding() {
    let negotiated = ServerCapabilitySet::new(true, true, true)
        .with_position_encoding(Some(PositionEncodingKind::UTF8));
    assert_eq!(
        negotiated.position_encoding(),
        Some(&PositionEncodingKind::UTF8)
    );

    let cleared = negotiated.with_position_encoding(None);
    assert_eq!(cleared.position_encoding(), None);
}

proptest! {
    /// The accessors must agree with the advertisement mask for every one of
    /// the 32 possible advertised sets, including sets built by clearing a
    /// flag that a previous builder call had set.
    #[test]
    fn accessors_match_the_advertisement_mask(mask in 0u8..32) {
        let set = set_from_bits(mask);

        for (index, kind) in ALL_KINDS.iter().enumerate() {
            let expected = mask & (1u8 << index) != 0;
            prop_assert_eq!(set.supports(*kind), expected, "supports({:?})", kind);
        }
        prop_assert_eq!(set.supports_definition(), mask & 0b0_0001 != 0);
        prop_assert_eq!(set.supports_references(), mask & 0b0_0010 != 0);
        prop_assert_eq!(set.supports_diagnostics(), mask & 0b0_0100 != 0);
        prop_assert_eq!(set.supports_call_hierarchy(), mask & 0b0_1000 != 0);
        prop_assert_eq!(set.supports_hover(), mask & 0b1_0000 != 0);

        let cleared = set.with_call_hierarchy(false).with_hover(false);
        prop_assert!(!cleared.supports_call_hierarchy());
        prop_assert!(!cleared.supports_hover());
        prop_assert_eq!(cleared.supports_definition(), mask & 0b0_0001 != 0);
    }

    /// `Deny` and `Force` must win over the advertisement, while `Allow` must
    /// leave the advertised state untouched. Each override is applied to a
    /// distinct capability so the precedence between them is not exercised.
    #[test]
    fn resolution_matches_advertisement_and_overrides(
        mask in 0u8..32,
        denied in prop::sample::select(ALL_KINDS.to_vec()),
        forced in prop::sample::select(ALL_KINDS.to_vec()),
        allowed in prop::sample::select(ALL_KINDS.to_vec()),
    ) {
        prop_assume!(denied != forced && denied != allowed && forced != allowed);
        let advertised = set_from_bits(mask);
        let mut overrides = CapabilityMatrix::default();
        overrides.set_override(
            Language::Rust.as_str(),
            denied.key(),
            CapabilityOverride::Deny,
        );
        overrides.set_override(
            Language::Rust.as_str(),
            forced.key(),
            CapabilityOverride::Force,
        );
        overrides.set_override(
            Language::Rust.as_str(),
            allowed.key(),
            CapabilityOverride::Allow,
        );

        let summary = resolve_capabilities(Language::Rust, &advertised, &overrides);

        for (index, kind) in ALL_KINDS.iter().enumerate() {
            let expected = if *kind == denied {
                (false, CapabilitySource::DeniedOverride)
            } else if *kind == forced {
                (true, CapabilitySource::ForcedOverride)
            } else if mask & (1u8 << index) != 0 {
                (true, CapabilitySource::ServerAdvertised)
            } else {
                (false, CapabilitySource::MissingOnServer)
            };
            let state = summary.state(*kind);
            prop_assert_eq!(state.enabled, expected.0, "enabled for {:?}", kind);
            prop_assert_eq!(state.source, expected.1, "source for {:?}", kind);
        }
    }
}
