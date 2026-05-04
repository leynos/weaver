//! Fixture catalogue for `observe graph-slice` snapshot coverage.
//!
//! The schema milestone reuses the curated symbol positions from the
//! `get-card` batteries while the later graph extraction milestones
//! add real multi-file edges. Keeping a dedicated module here lets the
//! graph-slice harness evolve independently without changing the test
//! call sites.

use crate::card_fixtures::CardFixtureCase;

/// One source fixture plus the cursor position used for `graph-slice`.
pub type GraphSliceFixtureCase = CardFixtureCase;

mod python;
mod rust;

pub use python::PYTHON_CASES;
pub use rust::RUST_CASES;
