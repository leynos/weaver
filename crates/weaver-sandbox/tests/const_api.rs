//! Compile-time coverage for the `const fn` accessors on [`SandboxProfile`]
//! and [`NetworkPolicy`].
//!
//! Rust rejects a call to a non-`const` function from inside a `const fn`
//! body. Each wrapper below therefore pins its accessor's const-ness at
//! compile time. The wrappers are called from an ordinary test because
//! building a profile is not a const operation and the workspace's
//! warnings-as-errors gate does not accept dead code.

use weaver_sandbox::{NetworkPolicy, SandboxProfile};

/// Compiles only while [`SandboxProfile::allow_networking`] is a `const fn`.
#[must_use]
const fn allow_networking_of(profile: SandboxProfile) -> SandboxProfile {
    profile.allow_networking()
}

/// Compiles only while [`SandboxProfile::network_policy`] is a `const fn`.
#[must_use]
const fn network_policy_of(profile: &SandboxProfile) -> NetworkPolicy { profile.network_policy() }

/// Compiles only while [`NetworkPolicy::is_denied`] is a `const fn`.
#[must_use]
const fn is_denied_of(policy: NetworkPolicy) -> bool { policy.is_denied() }

/// Checks that the wrappers preserve the default-deny and opt-in policies.
#[test]
fn const_accessors_agree_with_direct_calls() {
    assert!(SandboxProfile::new().network_policy().is_denied());

    let profile = SandboxProfile::new();
    assert_eq!(network_policy_of(&profile), profile.network_policy());
    assert!(is_denied_of(network_policy_of(&profile)));

    let permissive = allow_networking_of(profile);
    assert_eq!(network_policy_of(&permissive), NetworkPolicy::Allow);
    assert!(!is_denied_of(network_policy_of(&permissive)));
}
