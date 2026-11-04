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
    "GB_WEB_USER",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::process::Command;

    #[test]
    fn restores_environment_after_drop_and_panic() {
        const CHILD: &str = "GB_TEST_ENV_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // Inspect restoration only in an isolated process: other unit
            // tests may acquire the shared lock immediately after a drop.
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "test_support::tests::restores_environment_after_drop_and_panic",
                ])
                .env(CHILD, "1");
            for (index, key) in CONFIG_ENV.iter().enumerate() {
                if index % 2 == 0 {
                    command.env_remove(key);
                } else {
                    command.env(key, "inherited-value");
                }
            }
            command.env("GB_TOKEN", "");
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                command.env("GB_REPO", OsString::from_vec(vec![0xff]));
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "child test failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let original: Vec<_> = CONFIG_ENV.iter().map(std::env::var_os).collect();
        // The final iteration also verifies recovery of the poisoned mutex.
        for unwind in [false, true, false] {
            let mut checked = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let env = TestEnv::new();
                for key in CONFIG_ENV {
                    assert!(std::env::var_os(key).is_none());
                    env.set(key, "temporary-value");
                    assert_eq!(std::env::var(key).unwrap(), "temporary-value");
                }
                checked = true;
                if unwind {
                    panic!("exercise TestEnv restoration during unwinding");
                }
            }));
            assert!(
                checked,
                "environment setup failed before the expected panic"
            );
            assert_eq!(result.is_err(), unwind);
            let restored: Vec<_> = CONFIG_ENV.iter().map(std::env::var_os).collect();
            assert_eq!(restored, original);
        }
    }
}
