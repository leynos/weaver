//! Engine configuration for performance and safety limits.

/// Numeric engine limits controlling match counts, capture sizes, and
/// search depth.
///
/// Grouping these into a dedicated struct prevents accidental transposition
/// of the three positional `usize` parameters.
///
/// # Example
///
/// ```
/// use sempai_core::EngineLimits;
///
/// let limits = EngineLimits::new(5_000, 512_000, 50_000);
/// assert_eq!(limits.max_matches_per_rule(), 5_000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineLimits {
    /// Maximum number of matches emitted per rule before truncation.
    matches_per_rule: usize,
    /// Maximum bytes of source text captured per match.
    capture_text_bytes: usize,
    /// Maximum syntax tree nodes visited during deep ellipsis matching.
    deep_search_nodes: usize,
}

impl EngineLimits {
    /// Creates a new set of engine limits.
    #[must_use]
    pub const fn new(
        matches_per_rule: usize,
        capture_text_bytes: usize,
        deep_search_nodes: usize,
    ) -> Self {
        Self {
            matches_per_rule,
            capture_text_bytes,
            deep_search_nodes,
        }
    }

    /// Returns the maximum matches per rule.
    #[must_use]
    pub const fn max_matches_per_rule(&self) -> usize { self.matches_per_rule }

    /// Returns the maximum capture text bytes.
    #[must_use]
    pub const fn max_capture_text_bytes(&self) -> usize { self.capture_text_bytes }

    /// Returns the maximum deep search nodes.
    #[must_use]
    pub const fn max_deep_search_nodes(&self) -> usize { self.deep_search_nodes }
}

impl Default for EngineLimits {
    fn default() -> Self {
        Self {
            matches_per_rule: 10_000,
            capture_text_bytes: 1_048_576,
            deep_search_nodes: 100_000,
        }
    }
}

/// Engine configuration controlling match limits, capture sizes, and feature
/// gates.
///
/// # Defaults
///
/// The default configuration provides generous but bounded limits suitable
/// for interactive use:
///
/// - `max_matches_per_rule`: 10 000
/// - `max_capture_text_bytes`: 1 048 576 (1 MiB)
/// - `max_deep_search_nodes`: 100 000
/// - `enable_hcl`: `false`
///
/// # Example
///
/// ```
/// use sempai_core::{EngineConfig, EngineLimits};
///
/// let config = EngineConfig::default();
/// assert_eq!(config.limits().max_matches_per_rule(), 10_000);
/// assert!(!config.enable_hcl());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EngineConfig {
    /// Numeric limits for match counts, capture sizes, and search depth.
    limits: EngineLimits,
    /// Whether HCL support is enabled.
    enable_hcl: bool,
}

impl EngineConfig {
    /// Creates a new engine configuration with explicit values.
    #[must_use]
    pub const fn new(limits: EngineLimits, enable_hcl: bool) -> Self { Self { limits, enable_hcl } }

    /// Returns the numeric engine limits.
    #[must_use]
    pub const fn limits(&self) -> &EngineLimits { &self.limits }

    /// Returns the maximum matches per rule.
    #[must_use]
    pub const fn max_matches_per_rule(&self) -> usize { self.limits.matches_per_rule }

    /// Returns the maximum capture text bytes.
    #[must_use]
    pub const fn max_capture_text_bytes(&self) -> usize { self.limits.capture_text_bytes }

    /// Returns the maximum deep search nodes.
    #[must_use]
    pub const fn max_deep_search_nodes(&self) -> usize { self.limits.deep_search_nodes }

    /// Returns whether HCL support is enabled.
    #[must_use]
    pub const fn enable_hcl(&self) -> bool { self.enable_hcl }
}
