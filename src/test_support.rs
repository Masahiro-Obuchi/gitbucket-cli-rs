//! Serialize unit tests that read or modify process-wide CLI configuration.

use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const CONFIG_ENV: &[&str] = &[
    "GB_CONFIG_DIR",
    "GB_HOST",
    "GB_PROFILE",
    "GB_TOKEN",
    "GB_PROTOCOL",
    "GB_USER",
    "GB_REPO",
];

pub(crate) struct TestEnv {
    original: Vec<(&'static str, Option<OsString>)>,
    // Keep the lock until after Drop restores the original environment.
    _lock: MutexGuard<'static, ()>,
}

impl TestEnv {
    pub(crate) fn new() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let original = CONFIG_ENV
            .iter()
            .map(|&key| (key, std::env::var_os(key)))
            .collect();
        let env = Self {
            original,
            _lock: lock,
        };
        for key in CONFIG_ENV {
            std::env::remove_var(key);
        }
        env
    }

    pub(crate) fn set(&self, key: &str, value: impl AsRef<OsStr>) {
        assert!(CONFIG_ENV.contains(&key), "unmanaged test environment key");
        std::env::set_var(key, value);
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        for (key, value) in &self.original {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
