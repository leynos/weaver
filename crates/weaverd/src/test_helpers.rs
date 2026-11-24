//! Assertion helpers shared by `weaverd`'s unit and behavioural tests.
//!
//! Many of these tests return `Result` so they can propagate fixture failures
//! with `?`. A test that returns `Result` must not panic, so it cannot use
//! `assert!`/`assert_eq!`. These helpers report the same mismatch as an `Err`
//! value instead, keeping the diagnostic text identical in spirit.
//!
//! Takes small `Copy` values by value so call sites still read like
//! `assert_eq!`. Use [`ensure_same_ref`] when the values being compared are not
//! `Copy`, such as `Vec<&str>` or `String`.

/// Reports a mismatch as a test failure instead of panicking.
///
/// # Examples
///
/// ```text
/// assert!(ensure_same(2, 2, "worker count").is_ok());
/// assert_eq!(
///     ensure_same(2, 3, "worker count"),
///     Err(String::from("expected worker count 3, got 2"))
/// );
/// ```
pub fn ensure_same<T: std::fmt::Debug + PartialEq + Copy>(
    actual: T,
    expected: T,
    what: &str,
) -> Result<(), String> {
    ensure_same_ref(&actual, &expected, what)
}

/// Reports a mismatch as a test failure instead of panicking.
///
/// Borrowing variant of [`ensure_same`] for values that are not `Copy`.
///
/// # Examples
///
/// ```text
/// let actual = vec!["a", "b"];
/// let expected = vec!["a", "b"];
/// assert!(ensure_same_ref(&actual, &expected, "card order").is_ok());
/// ```
pub fn ensure_same_ref<T: std::fmt::Debug + PartialEq + ?Sized>(
    actual: &T,
    expected: &T,
    what: &str,
) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {what} {expected:?}, got {actual:?}"))
    }
}

/// Reports a false condition as a test failure instead of panicking.
///
/// # Examples
///
/// ```text
/// assert!(ensure_true(1 + 1 == 2, "arithmetic").is_ok());
/// assert_eq!(ensure_true(false, "coverage"), Err(String::from("expected coverage")));
/// ```
pub fn ensure_true(condition: bool, what: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("expected {what}"))
    }
}
