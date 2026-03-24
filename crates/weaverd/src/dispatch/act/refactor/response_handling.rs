//! Response handling and diff forwarding for `act refactor`.

use std::io::Write;
use std::path::Path;

use weaver_plugins::{PluginOutput, PluginResponse};

use crate::backends::FusionBackends;
use crate::dispatch::act::apply_patch;
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::{CommandDescriptor, CommandRequest};
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::DispatchResult;
use crate::semantic_provider::SemanticBackendProvider;

pub(super) fn handle_plugin_response<W: Write>(
    response: PluginResponse,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
) -> Result<DispatchResult, DispatchError> {
    if !response.is_success() {
        let diagnostics: Vec<String> = response
            .diagnostics()
            .iter()
            .map(|diag| diag.message().to_owned())
            .collect();
        let message = if diagnostics.is_empty() {
            String::from("plugin reported failure without diagnostics")
        } else {
            diagnostics.join("; ")
        };
        writer.write_stderr(format!("act refactor failed: {message}\n"))?;
        return Ok(DispatchResult::with_status(1));
    }

    match response.output() {
        PluginOutput::Diff { content } => {
            forward_diff_to_apply_patch(content, writer, backends, workspace_root)
        }
        PluginOutput::Analysis { .. } | PluginOutput::Empty => {
            writer.write_stderr(
                "act refactor failed: plugin succeeded but did not return diff output\n",
            )?;
            Ok(DispatchResult::with_status(1))
        }
    }
}

fn forward_diff_to_apply_patch<W: Write>(
    patch: &str,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
) -> Result<DispatchResult, DispatchError> {
    let patch_request = CommandRequest {
        command: CommandDescriptor {
            domain: String::from("act"),
            operation: String::from("apply-patch"),
        },
        arguments: Vec::new(),
        patch: Some(patch.to_owned()),
    };
    apply_patch::handle(&patch_request, writer, backends, workspace_root)
}
