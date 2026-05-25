//! Runtime doubles that exercise rollback paths for refactor tests.

use weaver_plugins::{PluginError, PluginRequest, PluginResponse};

use super::resolutions::{SelectedResolution, selected_resolution};
use crate::dispatch::act::refactor::{
    RefactorPluginRuntime,
    resolution::{CapabilityResolutionEnvelope, ResolutionRequest},
};

pub(crate) struct RollbackRuntime {
    pub(crate) resolution: CapabilityResolutionEnvelope,
    pub(crate) execute_result: ExecuteResult,
}

pub(crate) enum ExecuteResult {
    Success(PluginResponse),
    MissingPlugin(&'static str),
}

impl RefactorPluginRuntime for RollbackRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Ok(self.resolution.clone())
    }

    fn execute(
        &self,
        _provider: &str,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        match &self.execute_result {
            ExecuteResult::Success(response) => Ok(response.clone()),
            ExecuteResult::MissingPlugin(name) => Err(PluginError::NotFound {
                name: String::from(*name),
            }),
        }
    }
}

pub(crate) fn selected_runtime(
    config: SelectedResolution<'_>,
    execute_result: ExecuteResult,
) -> RollbackRuntime {
    RollbackRuntime {
        resolution: selected_resolution(config),
        execute_result,
    }
}

pub(crate) fn rollback_runtime(
    resolution: CapabilityResolutionEnvelope,
    execute_result: ExecuteResult,
) -> RollbackRuntime {
    RollbackRuntime {
        resolution,
        execute_result,
    }
}
