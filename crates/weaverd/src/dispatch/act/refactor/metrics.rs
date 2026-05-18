//! Position metrics for `act refactor`.

use std::sync::atomic::{AtomicU64, Ordering};

static POSITION_PARSE_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);
static POSITION_CONVERSION_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// Records position parsing and conversion failures.
pub(super) trait PositionMetrics {
    fn increment_parse_error(&self);
    fn increment_conversion_error(&self);
}

/// Production position metrics backed by process-local atomic counters.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct AtomicPositionMetrics;

impl PositionMetrics for AtomicPositionMetrics {
    fn increment_parse_error(&self) {
        increment_error_counter(&POSITION_PARSE_ERROR_COUNT, "position_parse_error");
    }

    fn increment_conversion_error(&self) {
        increment_error_counter(
            &POSITION_CONVERSION_ERROR_COUNT,
            "position_conversion_error",
        );
    }
}

/// No-op position metrics for pure unit tests.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct NullPositionMetrics;

#[cfg(test)]
impl PositionMetrics for NullPositionMetrics {
    fn increment_parse_error(&self) {}

    fn increment_conversion_error(&self) {}
}

fn increment_error_counter(counter: &AtomicU64, counter_name: &str) {
    let count = counter.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    tracing::debug!(
        counter = counter_name,
        count,
        "incremented position error counter"
    );
}
