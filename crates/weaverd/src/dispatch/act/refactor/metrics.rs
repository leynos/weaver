//! Position metrics for `act refactor`.
//!
//! This module records errors from the user-facing position pipeline. Parse
//! errors count rejected `--position LINE:COL` values in
//! `parse_refactor_args`, while conversion errors count failures in request
//! building when a validated line/column position cannot be translated into
//! the byte offset expected by rename providers.
//!
//! `PositionMetrics` is threaded through argument parsing, request building,
//! and position conversion so those command-side call sites increment
//! counters only when they observe an error. `AtomicPositionMetrics` is the
//! production implementation backed by private, process-local atomics, while
//! `NullPositionMetrics` is the test-only no-op implementation.

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

#[cfg(test)]
fn test_counter_values() -> (u64, u64) {
    (
        POSITION_PARSE_ERROR_COUNT.load(Ordering::Relaxed),
        POSITION_CONVERSION_ERROR_COUNT.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
fn reset_test_counters() {
    POSITION_PARSE_ERROR_COUNT.store(0, Ordering::Relaxed);
    POSITION_CONVERSION_ERROR_COUNT.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    //! Unit tests for position metrics implementations.
    //!
    //! Tests that read or mutate the global AtomicU64 counters must be
    //! annotated with `#[serial]` to prevent data races when the test suite is
    //! executed with the default multi-threaded test runner.

    use serial_test::serial;

    use super::{AtomicPositionMetrics, PositionMetrics, reset_test_counters, test_counter_values};

    #[test]
    #[serial]
    fn atomic_metrics_increment_each_counter() {
        reset_test_counters();
        let metrics = AtomicPositionMetrics;

        metrics.increment_parse_error();
        assert_eq!(test_counter_values(), (1, 0));

        metrics.increment_conversion_error();
        assert_eq!(test_counter_values(), (1, 1));
        reset_test_counters();
    }
}
