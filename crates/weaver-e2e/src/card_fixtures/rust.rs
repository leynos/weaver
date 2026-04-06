//! Rust fixtures for `observe get-card` snapshots.

use super::CardFixtureCase;

/// Rust `observe get-card` fixtures spanning functions, types, impl methods,
/// imports, and module-level constructs.
pub const RUST_CASES: [CardFixtureCase; 20] = [
    CardFixtureCase {
        name: "rust_simple_function",
        file_name: "simple.rs",
        source: "fn greet(name: &str) -> String {\n    format!(\"hello {name}\")\n}\n",
        line: 1,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_generics_function",
        file_name: "generics.rs",
        source: "fn wrap<T: Clone>(value: T) -> Option<T> {\n    Some(value.clone())\n}\n",
        line: 1,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_async_function",
        file_name: "async_fn.rs",
        source: "async fn fetch() -> Result<(), ()> {\n    Ok(())\n}\n",
        line: 1,
        column: 10,
    },
    CardFixtureCase {
        name: "rust_struct_definition",
        file_name: "struct.rs",
        source: "struct Point {\n    x: i32,\n    y: i32,\n}\n",
        line: 1,
        column: 8,
    },
    CardFixtureCase {
        name: "rust_enum_definition",
        file_name: "enum.rs",
        source: "enum Message {\n    Ping,\n    Data(String),\n    Exit { code: i32 },\n}\n",
        line: 1,
        column: 6,
    },
    CardFixtureCase {
        name: "rust_impl_methods",
        file_name: "impl_methods.rs",
        source: "struct Counter(u32);\n\nimpl Counter {\n    fn increment(&mut self) {\n        \
                 self.0 += 1;\n    }\n}\n",
        line: 4,
        column: 8,
    },
    CardFixtureCase {
        name: "rust_trait_definition",
        file_name: "trait.rs",
        source: "trait Render {\n    fn render(&self) -> String;\n}\n",
        line: 1,
        column: 7,
    },
    CardFixtureCase {
        name: "rust_trait_impl",
        file_name: "trait_impl.rs",
        source: "trait Render {\n    fn render(&self) -> String;\n}\n\nstruct Card;\n\nimpl \
                 Render for Card {\n    fn render(&self) -> String {\n        \
                 String::from(\"ok\")\n    }\n}\n",
        line: 7,
        column: 8,
    },
    CardFixtureCase {
        name: "rust_const_static",
        file_name: "const_static.rs",
        source: "const VERSION: &str = \"v1\";\nstatic ENABLED: bool = true;\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "rust_lifetime_function",
        file_name: "lifetimes.rs",
        source: "fn first<'a>(items: &'a [String]) -> Option<&'a String> {\n    items.first()\n}\n",
        line: 1,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_closure_assignment",
        file_name: "closure.rs",
        source: "let_it = |value: i32| value + 1;\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "rust_control_flow",
        file_name: "control_flow.rs",
        source: "fn classify(value: i32) -> &'static str {\n    match value {\n        0 => \
                 \"zero\",\n        value if value > 10 => \"big\",\n        _ => \"small\",\n    \
                 }\n}\n",
        line: 1,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_doc_comments",
        file_name: "doc_comments.rs",
        source: "/// Greets callers.\nfn greet() {}\n",
        line: 2,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_derive_macro",
        file_name: "derive.rs",
        source: "#[derive(Clone, Debug)]\nstruct User {\n    name: String,\n}\n",
        line: 2,
        column: 8,
    },
    CardFixtureCase {
        name: "rust_attribute_macro",
        file_name: "attribute.rs",
        source: "#[cfg(test)]\nfn sample_test() {}\n",
        line: 2,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_type_alias",
        file_name: "type_alias.rs",
        source: "type UserId = u64;\n",
        line: 1,
        column: 6,
    },
    CardFixtureCase {
        name: "rust_use_block",
        file_name: "use_block.rs",
        source: "use std::collections::HashMap;\nuse std::fmt::Debug;\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "rust_result_function",
        file_name: "result_fn.rs",
        source: "fn load(path: &str) -> Result<String, std::io::Error> {\n    \
                 std::fs::read_to_string(path)\n}\n",
        line: 1,
        column: 4,
    },
    CardFixtureCase {
        name: "rust_tuple_struct",
        file_name: "tuple_struct.rs",
        source: "struct UserId(u64);\n",
        line: 1,
        column: 8,
    },
    CardFixtureCase {
        name: "rust_module_doc_and_uses",
        file_name: "module_doc.rs",
        source: "//! Cache snapshot helpers.\n\nuse std::path::Path;\n",
        line: 1,
        column: 1,
    },
];
