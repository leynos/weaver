//! Semantic-enrichment tests for `observe::get_card`.

use super::*;

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

    assert_eq!(status, 0);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["card"]["lsp"]["source"], "lsp_hover");
    assert_eq!(
        payload["card"]["lsp"]["type"],
        "fn greet(name: &str) -> usize"
    );
    assert_eq!(payload["card"]["lsp"]["deprecated"], true);
    assert_eq!(
        payload["card"]["provenance"]["sources"],
        serde_json::json!(["tree_sitter", "lsp_hover"])
    );
    Ok(())
}

#[rstest]
fn handle_returns_semantic_success_with_degraded_provenance_when_hover_is_unavailable(
    temp_dir: Result<TempDir, String>,
) -> Result<(), String> {
    let dir = temp_dir?;
    let (server, _hover_params) =
        StubLanguageServer::missing_hover(ServerCapabilitySet::new(false, false, false));

    let (status, payload) = dispatch_semantic_card(&dir, server)?;

    assert_eq!(status, 0);
    assert_eq!(payload["status"], "success");
    assert!(payload["card"]["lsp"].is_null());
    assert_eq!(
        payload["card"]["provenance"]["sources"],
        serde_json::json!(["tree_sitter", "tree_sitter_degraded_semantic"])
    );
    Ok(())
}
