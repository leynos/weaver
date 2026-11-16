//! Semantic-enrichment tests for `observe::get_card`.

use super::*;
use crate::test_helpers::ensure_same;

/// Returns the `key` member of a JSON object, reporting a missing field.
fn field<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a serde_json::Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field `{key}`"))
}

/// Dispatches the standard Rust fixture against `server` at semantic detail.
fn dispatch_semantic_card(
    temp_dir: &TempDir,
    server: StubLanguageServer,
) -> Result<(i32, serde_json::Value), String> {
    let path = write_source(
        temp_dir,
        SourceFile {
            name: "card.rs",
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = \
                      name.len();\n    count\n}\n",
        },
    )?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| String::from("failed to convert source path to file URI"))?
        .to_string();
    let request = make_request(&uri, 2, 4, DetailLevel::Semantic)?;
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server)?;
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = handle(&request, &mut writer, &mut backends)
        .map_err(|error| format!("handler should succeed: {error}"))?;
    let payload = response_payload(output)?;
    Ok((result.status, payload))
}

#[rstest]
fn handle_returns_semantic_success_with_enrichment_and_rewritten_provenance(
    temp_dir: Result<TempDir, String>,
) -> Result<(), String> {
    let dir = temp_dir?;
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        markdown_hover(concat!(
            "```rust\nfn greet(name: &str) -> usize\n```\n",
            "**Deprecated**: use `welcome` instead"
        )),
    );

    let (status, payload) = dispatch_semantic_card(&dir, server)?;

    ensure_same(status, 0, "handler status")?;
    ensure_same(
        payload.get("status").and_then(serde_json::Value::as_str),
        Some("success"),
        "envelope status",
    )?;

    let card = field(&payload, "card")?;
    let lsp = field(card, "lsp")?;
    ensure_same(
        lsp.get("source").and_then(serde_json::Value::as_str),
        Some("lsp_hover"),
        "lsp.source",
    )?;
    ensure_same(
        lsp.get("type").and_then(serde_json::Value::as_str),
        Some("fn greet(name: &str) -> usize"),
        "lsp.type",
    )?;
    ensure_same(
        lsp.get("deprecated").and_then(serde_json::Value::as_bool),
        Some(true),
        "lsp.deprecated",
    )?;

    let provenance = field(card, "provenance")?;
    let expected_sources = serde_json::json!(["tree_sitter", "lsp_hover"]);
    ensure_same(
        provenance.get("sources"),
        Some(&expected_sources),
        "provenance.sources",
    )
}

#[rstest]
fn handle_returns_semantic_success_with_degraded_provenance_when_hover_is_unavailable(
    temp_dir: Result<TempDir, String>,
) -> Result<(), String> {
    let dir = temp_dir?;
    let (server, _hover_params) =
        StubLanguageServer::missing_hover(ServerCapabilitySet::new(false, false, false));

    let (status, payload) = dispatch_semantic_card(&dir, server)?;

    ensure_same(status, 0, "handler status")?;
    ensure_same(
        payload.get("status").and_then(serde_json::Value::as_str),
        Some("success"),
        "envelope status",
    )?;

    let card = field(&payload, "card")?;
    // A card without LSP enrichment omits `lsp` entirely, because the field is
    // skipped when it is `None`, so an absent key means "no LSP data".
    ensure_same(
        card.get("lsp").is_none_or(serde_json::Value::is_null),
        true,
        "lsp to be null or absent",
    )?;

    let provenance = field(card, "provenance")?;
    let expected_sources = serde_json::json!(["tree_sitter", "tree_sitter_degraded_semantic"]);
    ensure_same(
        provenance.get("sources"),
        Some(&expected_sources),
        "provenance.sources",
    )
}
