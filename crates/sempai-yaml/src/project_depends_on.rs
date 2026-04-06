//! Typed support for the Semgrep compatibility dependency principal.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Validated payload for the Semgrep compatibility dependency principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub struct ProjectDependsOnPayload {
    namespace: String,
    package: String,
}

impl ProjectDependsOnPayload {
    /// Returns the dependency namespace.
    #[must_use]
    pub fn namespace(&self) -> &str { &self.namespace }

    /// Returns the dependency package.
    #[must_use]
    pub fn package(&self) -> &str { &self.package }

    /// Consumes the wrapper and returns the underlying payload.
    #[must_use]
    pub fn into_inner(self) -> Value {
        Value::Object(
            [
                (String::from("namespace"), Value::String(self.namespace)),
                (String::from("package"), Value::String(self.package)),
            ]
            .into_iter()
            .collect(),
        )
    }
}

impl TryFrom<Value> for ProjectDependsOnPayload {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Some(object) = value.as_object() else {
            return Err(String::from(
                "`r2c-internal-project-depends-on` must be a mapping",
            ));
        };

        let has_namespace = object.get("namespace").and_then(Value::as_str).is_some();
        let has_package = object.get("package").and_then(Value::as_str).is_some();
        if !(has_namespace && has_package) {
            return Err(String::from(
                "`r2c-internal-project-depends-on` must define string `namespace` and `package` \
                 fields",
            ));
        }

        let namespace = object
            .get("namespace")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                String::from(
                    "`r2c-internal-project-depends-on` must define string `namespace` and \
                     `package` fields",
                )
            })?
            .to_owned();
        let package = object
            .get("package")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                String::from(
                    "`r2c-internal-project-depends-on` must define string `namespace` and \
                     `package` fields",
                )
            })?
            .to_owned();

        Ok(Self { namespace, package })
    }
}

impl From<ProjectDependsOnPayload> for Value {
    fn from(payload: ProjectDependsOnPayload) -> Self { payload.into_inner() }
}
