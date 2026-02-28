//! Engine configuration for performance and safety limits.

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
/// use sempai_core::EngineConfig;
///
/// let config = EngineConfig::default();
/// assert_eq!(config.max_matches_per_rule(), 10_000);
/// assert!(!config.enable_hcl());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// Maximum number of matches emitted per rule before truncation.
    max_matches_per_rule: usize,
    /// Maximum bytes of source text captured per match.
    max_capture_text_bytes: usize,
    /// Maximum syntax tree nodes visited during deep ellipsis matching.
    max_deep_search_nodes: usize,
    /// Whether HCL support is enabled.
    enable_hcl: bool,
}

impl EngineConfig {
    /// Creates a new engine configuration with explicit values.
    #[must_use]
    pub const fn new(
        max_matches_per_rule: usize,
        max_capture_text_bytes: usize,
        max_deep_search_nodes: usize,
        enable_hcl: bool,
    ) -> Self {
        Self {
            max_matches_per_rule,
            max_capture_text_bytes,
            max_deep_search_nodes,
            enable_hcl,
        }
    }

    /// Returns the maximum matches per rule.
    #[must_use]
    pub const fn max_matches_per_rule(&self) -> usize {
        self.max_matches_per_rule
    }

    /// Returns the maximum capture text bytes.
    #[must_use]
    pub const fn max_capture_text_bytes(&self) -> usize {
        self.max_capture_text_bytes
    }

    /// Returns the maximum deep search nodes.
    #[must_use]
    pub const fn max_deep_search_nodes(&self) -> usize {
        self.max_deep_search_nodes
    }

    /// Returns whether HCL support is enabled.
    #[must_use]
    pub const fn enable_hcl(&self) -> bool {
        self.enable_hcl
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_matches_per_rule: 10_000,
            max_capture_text_bytes: 1_048_576,
            max_deep_search_nodes: 100_000,
            enable_hcl: false,
        }
    }
}
