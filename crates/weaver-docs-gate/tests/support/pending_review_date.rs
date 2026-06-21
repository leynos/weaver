//! Pending dependency review-date expiry checks for the boundary manifest gate.
//!
//! This support module is compiled only by the boundary manifest integration
//! test. It keeps the time-window rule isolated from file parsing and document
//! rendering: callers provide the build date explicitly, and this module only
//! parses `next_review_by` values and checks them against the 270-day pending
//! review budget.

use time::{Date, format_description::well_known::Iso8601};

const PENDING_REVIEW_WINDOW_DAYS: i64 = 270;

type TestResult<T = ()> = Result<T, String>;

/// Return an error when a pending-review invariant is false.
fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Validate that a pending row's review date is not stale.
pub(crate) fn validate_pending_review_date(
    value: &str,
    task_id: &str,
    build_date: Date,
) -> TestResult {
    let review_date = parse_review_date(value, task_id)?;
    ensure(
        is_review_current(review_date, build_date),
        format!(
            "task {task_id} has stale next_review_by date {value:?}; pending reviews must be no \
             more than {PENDING_REVIEW_WINDOW_DAYS} days behind the build date",
        ),
    )
}

/// Parse a pending-review date for one manifest task.
fn parse_review_date(value: &str, task_id: &str) -> TestResult<Date> {
    Date::parse(value, &Iso8601::DATE).map_err(|_| invalid_date_message(value, task_id))
}

/// Build a diagnostic for an invalid pending-review date.
fn invalid_date_message(value: &str, task_id: &str) -> String {
    format!("task {task_id} has invalid next_review_by date {value:?}")
}

/// Return whether the review date is within the allowed review window.
fn is_review_current(review_date: Date, build_date: Date) -> bool {
    (build_date - review_date).whole_days() <= PENDING_REVIEW_WINDOW_DAYS
}

#[cfg(test)]
mod tests {
    //! Unit tests for pending review age-window comparisons.

    use proptest::prelude::*;
    use time::{Duration, macros::date};

    use super::{PENDING_REVIEW_WINDOW_DAYS, is_review_current, validate_pending_review_date};

    /// Prove the review window accepts the exact maximum age.
    #[test]
    fn review_window_accepts_exact_boundary() {
        assert!(is_review_current(
            date!(2026 - 01 - 01),
            date!(2026 - 09 - 28)
        ));
    }

    /// Prove the review window rejects stale pending reviews.
    #[test]
    fn review_window_rejects_expired_pending_review() {
        assert!(!is_review_current(
            date!(2026 - 01 - 01),
            date!(2026 - 09 - 29)
        ));
    }

    proptest! {
        #[test]
        /// Prove all generated dates inside the review window are accepted.
        fn review_window_accepts_generated_in_range(age in 0i64..=PENDING_REVIEW_WINDOW_DAYS) {
            let build_date = date!(2026 - 12 - 31);
            let review_date = build_date - Duration::days(age);

            prop_assert!(validate_pending_review_date(
                &review_date.to_string(),
                "12.1.5",
                build_date,
            ).is_ok());
        }

        #[test]
        /// Prove generated dates older than the review window are rejected.
        fn review_window_rejects_generated_stale_dates(age in (PENDING_REVIEW_WINDOW_DAYS + 1)..=720) {
            let build_date = date!(2026 - 12 - 31);
            let review_date = build_date - Duration::days(age);

            prop_assert!(validate_pending_review_date(
                &review_date.to_string(),
                "12.1.5",
                build_date,
            ).is_err());
        }
    }
}
