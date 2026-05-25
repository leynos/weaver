//! Shared helper modules for `act refactor` behaviour tests.
//!
//! This file is intentionally loaded multiple times by different test modules
//! using `#[path = "refactor_helpers.rs"]` to provide shared test utilities.

#[path = "refactor_helpers/builders.rs"]
pub(crate) mod builders;
#[path = "refactor_helpers/content.rs"]
pub(crate) mod content;
#[path = "refactor_helpers/resolutions.rs"]
pub(crate) mod resolutions;
#[path = "refactor_helpers/rollback.rs"]
pub(crate) mod rollback;
