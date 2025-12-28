//! Test fixtures for E2E tests.
//!
//! This module provides Python code samples used in call hierarchy tests.

/// A simple Python module with a linear call chain.
///
/// ```python
/// def a():
///     b()
///
/// def b():
///     c()
///
/// def c():
///     pass
/// ```
pub const LINEAR_CHAIN: &str = r"def a():
    b()

def b():
    c()

def c():
    pass
";

/// A Python module with diamond-shaped call relationships.
///
/// ```python
/// def top():
///     left()
///     right()
///
/// def left():
///     bottom()
///
/// def right():
///     bottom()
///
/// def bottom():
///     pass
/// ```
pub const DIAMOND: &str = r"def top():
    left()
    right()

def left():
    bottom()

def right():
    bottom()

def bottom():
    pass
";

/// A Python module with class methods.
///
/// ```python
/// class Calculator:
///     def add(self, a, b):
///         return self.validate(a) + self.validate(b)
///
///     def validate(self, value):
///         return abs(value)
/// ```
pub const CLASS_METHODS: &str = r"class Calculator:
    def add(self, a, b):
        return self.validate(a) + self.validate(b)

    def validate(self, value):
        return abs(value)
";

/// A Python module with no function calls (empty call graph).
pub const NO_CALLS: &str = r"def standalone():
    x = 1
    y = 2
    return x + y
";
