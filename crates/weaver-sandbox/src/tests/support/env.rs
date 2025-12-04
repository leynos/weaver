use std::sync::{Mutex, MutexGuard, OnceLock};

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn lock_env() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env mutex poisoned")
}
