//! Test fixtures for E2E tests.
//!
//! This module provides code samples for pattern matching, rewriting,
//! and call hierarchy tests across Rust, Python, and TypeScript.

// =============================================================================
// Python Fixtures
// =============================================================================

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

/// Python module with multiple function definitions for grep tests.
pub const PYTHON_FUNCTIONS: &str = r#"def greet(name):
    print(f"Hello, {name}")

def farewell(name):
    print(f"Goodbye, {name}")

def process(data):
    result = transform(data)
    return result
"#;

/// Python module with print statements for rewrite tests.
pub const PYTHON_PRINTS: &str = r#"def log_info():
    print("Starting process")
    print("Step 1 complete")
    print("Step 2 complete")
    print("Finished")
"#;

/// Python module with class and method calls.
pub const PYTHON_CLASS: &str = r"class Service:
    def process(self, data):
        validated = self.validate(data)
        return self.transform(validated)

    def validate(self, data):
        return data

    def transform(self, data):
        return data.upper()
";

// =============================================================================
// Rust Fixtures
// =============================================================================

/// Rust module with multiple function definitions for grep tests.
pub const RUST_FUNCTIONS: &str = r#"fn main() {
    println!("Hello, world!");
}

fn helper() {
    let x = 42;
    println!("Value: {}", x);
}

fn process(data: &str) -> String {
    data.to_uppercase()
}
"#;

/// Rust module with let bindings for grep and rewrite tests.
pub const RUST_LET_BINDINGS: &str = r#"fn calculate() {
    let a = 1;
    let b = 2;
    let result = a + b;
    println!("Result: {}", result);
}
"#;

/// Rust module with debug macros for rewrite tests.
pub const RUST_DEBUG_MACROS: &str = r#"fn debug_values() {
    let x = 42;
    let y = "hello";
    dbg!(x);
    dbg!(y);
    dbg!(x + 1);
}
"#;

/// Rust module with println! statements for rewrite tests.
pub const RUST_PRINTLN: &str = r#"fn logging() {
    println!("Starting");
    println!("Processing: {}", 42);
    println!("Done");
}
"#;

/// Rust module with struct definitions.
pub const RUST_STRUCTS: &str = r"struct Point {
    x: i32,
    y: i32,
}

struct Rectangle {
    width: u32,
    height: u32,
}

fn create_point() -> Point {
    Point { x: 0, y: 0 }
}
";

// =============================================================================
// TypeScript Fixtures
// =============================================================================

/// TypeScript module with function declarations for grep tests.
pub const TYPESCRIPT_FUNCTIONS: &str = r"function greet(name: string): void {
    console.log(`Hello, ${name}`);
}

function farewell(name: string): void {
    console.log(`Goodbye, ${name}`);
}

function process(data: string): string {
    return data.toUpperCase();
}
";

/// TypeScript module with arrow functions for grep tests.
pub const TYPESCRIPT_ARROW_FUNCTIONS: &str = r"const add = (a: number, b: number): number => a + b;

const multiply = (a: number, b: number): number => {
    return a * b;
};

const greet = (name: string): void => {
    console.log(`Hello, ${name}`);
};
";

/// TypeScript module with console.log for rewrite tests.
pub const TYPESCRIPT_CONSOLE: &str = r#"function logMessages(): void {
    console.log("Starting");
    console.log("Processing", 42);
    console.log("Done");
}
"#;

/// TypeScript module with interface definitions.
pub const TYPESCRIPT_INTERFACES: &str = r"interface Point {
    x: number;
    y: number;
}

interface Rectangle {
    width: number;
    height: number;
}

interface User {
    name: string;
    email: string;
}
";

/// TypeScript module with var declarations for rewrite tests.
pub const TYPESCRIPT_VAR_DECLARATIONS: &str = r"function oldStyle(): void {
    var x = 1;
    var y = 2;
    var result = x + y;
    console.log(result);
}
";

/// TypeScript module with class for definition tests.
pub const TYPESCRIPT_CLASS: &str = r"class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}

const calc = new Calculator();
const result = calc.add(1, 2);
";
