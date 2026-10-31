//! Time sources and time-based policies for the language server adapter.
//!
//! The adapter makes two decisions that depend on the monotonic clock: how long
//! to wait for one response ([`Deadline`]) and how long to leave a server alone
//! after it stopped answering ([`RestartCooldown`]). Both are pure functions of
//! an [`Instant`] they are handed, and the adapter reads the instant from an
//! injected [`Clock`], so tests can decide expiry and cooldown without sleeping
//! or racing the real clock.

use std::{
    fmt,
    time::{Duration, Instant},
};

/// A monotonic time source.
pub trait Clock: Send + Sync + fmt::Debug {
    /// Returns the current instant.
    fn now(&self) -> Instant;
}

/// The operating system's monotonic clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant { Instant::now() }
}

/// The instant by which a response must arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline(Instant);

impl Deadline {
    /// Returns the deadline `timeout` after `now`, or `None` when the sum does
    /// not fit in an [`Instant`] (a timeout such as [`Duration::MAX`]).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let deadline = Deadline::after(clock.now(), Duration::from_secs(30)).expect("fits");
    /// assert_eq!(deadline.remaining(clock.now()), Duration::from_secs(30));
    /// ```
    #[must_use]
    pub fn after(now: Instant, timeout: Duration) -> Option<Self> {
        now.checked_add(timeout).map(Self)
    }

    /// Returns how long is left at `now`; zero once the deadline has passed.
    #[must_use]
    pub fn remaining(self, now: Instant) -> Duration { self.0.saturating_duration_since(now) }
}

/// How long a server that stopped answering is left alone before the adapter
/// will start a fresh process for it.
///
/// Without a cooldown, every request in a batch (for example one hover per card
/// in a graph slice) would start a new process and wait out its own deadline,
/// multiplying the stall by the batch size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartCooldown {
    period: Duration,
}

impl RestartCooldown {
    /// Creates a cooldown lasting `period`.
    #[must_use]
    pub const fn new(period: Duration) -> Self { Self { period } }

    /// Returns the length of the cooldown.
    #[must_use]
    pub const fn period(self) -> Duration { self.period }

    /// Returns whether a server stopped at `stopped_at` is still cooling down
    /// at `now`. A server that never stopped is not cooling down, and the
    /// cooldown is over once a full `period` has passed.
    #[must_use]
    pub fn is_cooling(self, stopped_at: Option<Instant>, now: Instant) -> bool {
        stopped_at.is_some_and(|at| now.saturating_duration_since(at) < self.period)
    }
}

/// A clock that only moves when a test says so.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ManualClock {
    origin: Instant,
    elapsed: std::sync::Mutex<Duration>,
}

#[cfg(test)]
impl ManualClock {
    /// Creates a clock frozen at the moment of creation.
    pub(crate) fn new() -> Self {
        Self {
            origin: Instant::now(),
            elapsed: std::sync::Mutex::new(Duration::ZERO),
        }
    }

    /// Moves the clock forward by `by`.
    pub(crate) fn advance(&self, by: Duration) {
        *self
            .elapsed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) += by;
    }
}

#[cfg(test)]
impl Clock for ManualClock {
    fn now(&self) -> Instant {
        let elapsed = *self
            .elapsed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        self.origin + elapsed
    }
}

#[cfg(test)]
mod tests {
    //! Deterministic tests for the deadline and cooldown policies.

    use rstest::rstest;

    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[rstest]
    #[case::fresh(Duration::ZERO, SECOND * 30)]
    #[case::partway(SECOND * 10, SECOND * 20)]
    #[case::exactly_due(SECOND * 30, Duration::ZERO)]
    #[case::long_past(SECOND * 31, Duration::ZERO)]
    fn deadline_remaining_counts_down_and_stops_at_zero(
        #[case] elapsed: Duration,
        #[case] expected: Duration,
    ) {
        let clock = ManualClock::new();
        let deadline = Deadline::after(clock.now(), SECOND * 30).expect("30s fits");

        clock.advance(elapsed);

        assert_eq!(deadline.remaining(clock.now()), expected);
    }

    #[rstest]
    fn deadline_refuses_a_timeout_the_clock_cannot_represent() {
        assert_eq!(Deadline::after(Instant::now(), Duration::MAX), None);
    }

    #[rstest]
    #[case::never_stopped(None, Duration::ZERO, false)]
    #[case::just_stopped(Some(Duration::ZERO), Duration::ZERO, true)]
    #[case::inside_the_period(Some(Duration::ZERO), SECOND * 59, true)]
    #[case::exactly_at_the_period(Some(Duration::ZERO), SECOND * 60, false)]
    #[case::after_the_period(Some(Duration::ZERO), SECOND * 61, false)]
    fn cooldown_refuses_inside_the_period_and_allows_after_it(
        #[case] stopped_after: Option<Duration>,
        #[case] now_after: Duration,
        #[case] expected: bool,
    ) {
        let clock = ManualClock::new();
        let origin = clock.now();
        let cooldown = RestartCooldown::new(SECOND * 60);

        let stopped_at = stopped_after.map(|after| origin + after);

        assert_eq!(
            cooldown.is_cooling(stopped_at, origin + now_after),
            expected
        );
    }
}
