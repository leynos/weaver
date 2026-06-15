//! Pending dependency review-date expiry checks for the boundary manifest gate.

use time::{Date, OffsetDateTime, format_description::well_known::Iso8601};

const PENDING_REVIEW_WINDOW_DAYS: i64 = 270;

type TestResult<T = ()> = Result<T, String>;

fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub(crate) fn validate_pending_review_date(value: &str, task_id: &str) -> TestResult {
    let review_date = parse_review_date(value, task_id)?;
    let build_date = build_date()?;
    ensure(
        is_review_current(review_date, build_date),
        format!(
            "task {task_id} has stale next_review_by date {value:?}; pending reviews must be no \
             more than {PENDING_REVIEW_WINDOW_DAYS} days behind the build date",
        ),
    )
}

fn parse_review_date(value: &str, task_id: &str) -> TestResult<Date> {
    Date::parse(value, &Iso8601::DATE).map_err(|_| invalid_date_message(value, task_id))
}

fn build_date() -> TestResult<Date> {
    if let Ok(epoch) = std::env::var("SOURCE_DATE_EPOCH") {
        let seconds = epoch
            .parse::<i64>()
            .map_err(|error| format!("SOURCE_DATE_EPOCH must be Unix seconds: {error}"))?;
        OffsetDateTime::from_unix_timestamp(seconds)
            .map(OffsetDateTime::date)
            .map_err(|error| format!("SOURCE_DATE_EPOCH is outside supported range: {error}"))
    } else {
        Ok(OffsetDateTime::now_utc().date())
    }
}

fn invalid_date_message(value: &str, task_id: &str) -> String {
    format!("task {task_id} has invalid next_review_by date {value:?}")
}

fn is_review_current(review_date: Date, build_date: Date) -> bool {
    (build_date - review_date).whole_days() <= PENDING_REVIEW_WINDOW_DAYS
}

#[cfg(test)]
mod tests {
    //! Unit tests for pending review age-window comparisons.

    use time::macros::date;

    use super::is_review_current;

    #[test]
    fn review_window_accepts_exact_boundary() {
        assert!(is_review_current(
            date!(2026 - 01 - 01),
            date!(2026 - 09 - 28)
        ));
    }

    #[test]
    fn review_window_rejects_expired_pending_review() {
        assert!(!is_review_current(
            date!(2026 - 01 - 01),
            date!(2026 - 09 - 29)
        ));
    }
}
