//! Shared synchronization for tests that temporarily change process environment.

use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::{Result, anyhow};

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn lock_env() -> Result<MutexGuard<'static, ()>> {
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow!("environment test mutex is poisoned"))
}
