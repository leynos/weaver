//! Unit tests for environment snapshot and restoration.

use std::env;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::env_guard::EnvGuard;

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env mutex poisoned")
}

#[test]
fn restores_modified_and_removed_environment_variables() {
    const EXISTING: &str = "WEAVER_ENV_GUARD_EXISTING";
    const ANOTHER: &str = "WEAVER_ENV_GUARD_ANOTHER";
    const EPHEMERAL: &str = "WEAVER_ENV_GUARD_EPHEMERAL";

    let _guard = lock_env();

    unsafe { env::set_var(EXISTING, "original") };
    unsafe { env::set_var(ANOTHER, "keep") };

    let snapshot = EnvGuard::capture();

    unsafe { env::set_var(EXISTING, "changed") };
    unsafe { env::remove_var(ANOTHER) };
    unsafe { env::set_var(EPHEMERAL, "ephemeral") };

    snapshot.restore();

    assert_eq!(env::var(EXISTING).as_deref(), Ok("original"));
    assert_eq!(env::var(ANOTHER).as_deref(), Ok("keep"));
    assert!(env::var(EPHEMERAL).is_err());

    unsafe { env::remove_var(EXISTING) };
    unsafe { env::remove_var(ANOTHER) };
}

#[test]
fn removes_variables_created_during_guard_lifetime() {
    const PRE_EXISTING: &str = "WEAVER_ENV_GUARD_PRE_EXISTING";
    const CREATED: &str = "WEAVER_ENV_GUARD_CREATED";

    let _guard = lock_env();

    unsafe { env::set_var(PRE_EXISTING, "value") };

    let snapshot = EnvGuard::capture();

    unsafe { env::set_var(CREATED, "temporary") };

    assert_eq!(env::var(PRE_EXISTING).as_deref(), Ok("value"));
    assert_eq!(env::var(CREATED).as_deref(), Ok("temporary"));

    snapshot.restore();

    assert_eq!(env::var(PRE_EXISTING).as_deref(), Ok("value"));
    assert!(env::var(CREATED).is_err());

    unsafe { env::remove_var(PRE_EXISTING) };
}
