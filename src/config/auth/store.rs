use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::ensure_config_dir;
use crate::error::{GbError, Result};

use super::model::AuthConfig;

const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const FILE_ACCESS_TIMEOUT: Duration = Duration::from_secs(1);

fn transient_file_access(error: &io::Error) -> bool {
    #[cfg(windows)]
    {
        // ACCESS_DENIED can describe a delete-pending file during replacement;
        // SHARING_VIOLATION and LOCK_VIOLATION can describe an active reader.
        matches!(error.raw_os_error(), Some(5 | 32 | 33))
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
}

fn retry_file_access<T>(mut operation: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + FILE_ACCESS_TIMEOUT;
    loop {
        let result = operation();
        if let Err(GbError::Io(error)) = &result {
            if transient_file_access(error) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
        return result;
    }
}

impl AuthConfig {
    pub fn load() -> Result<Self> {
        read_config(&ensure_config_dir()?.join("config.toml"))
    }

    /// Apply a change to the latest configuration under an interprocess file
    /// lock. Callers must perform prompts and network requests before entering.
    /// Return values (including success messages) are published only on commit.
    pub fn update<T>(change: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        update_config(&ensure_config_dir()?.join("config.toml"), change)
    }
}

fn read_config(path: &Path) -> Result<AuthConfig> {
    let mut file = match open_regular(path, false) {
        Ok(file) => file,
        Err(GbError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(AuthConfig::default());
        }
        Err(err) => return Err(err),
    };
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(toml::from_str(&content)?)
}

fn update_config<T>(path: &Path, change: impl FnOnce(&mut AuthConfig) -> Result<T>) -> Result<T> {
    with_config_lock(path, || update_locked(path, change))
}

pub(super) fn with_config_lock<T>(path: &Path, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    // The lock uses a stable separate inode. Never unlink it on release: doing
    // so could let a new writer lock another inode while a waiter holds this one.
    let lock_path = path.with_file_name("config.toml.lock");
    let lock = open_regular(&lock_path, true)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        lock.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let deadline = Instant::now() + LOCK_TIMEOUT;
    loop {
        match fs2::FileExt::try_lock_exclusive(&lock) {
            Ok(()) => break,
            Err(err) if err.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                if Instant::now() >= deadline {
                    return Err(GbError::Config(
                        "Configuration is busy: timed out waiting for config.toml.lock. Retry after the other gb command finishes; do not delete the lock file.".into(),
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(err.into()),
        }
    }
    // Dropping the file releases the OS lock, even after a failed change or a
    // process termination. No lock-file contents or PID expiry are involved.
    operation()
}

fn update_locked<T>(path: &Path, change: impl FnOnce(&mut AuthConfig) -> Result<T>) -> Result<T> {
    let mut config = read_config(path)?;
    let before = toml::to_string_pretty(&config)?;
    let result = change(&mut config)?;
    let content = toml::to_string_pretty(&config)?;
    if content != before {
        write_config_file(path, &content)?;
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            match open_regular(path, false) {
                Ok(file) => file.set_permissions(fs::Permissions::from_mode(0o600))?,
                Err(GbError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(result)
}

fn check_regular(path: &Path) -> Result<()> {
    match retry_file_access(|| fs::symlink_metadata(path).map_err(GbError::Io)) {
        Ok(metadata) if is_regular(&metadata) => Ok(()),
        Ok(_) => Err(GbError::Config(format!(
            "Refusing non-regular configuration path {} (symlinks and reparse points are not supported). Point GB_CONFIG_DIR at the directory containing the real config.toml.",
            path.display()
        ))),
        Err(GbError::Io(err)) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn is_regular(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            // FILE_ATTRIBUTE_REPARSE_POINT
            return false;
        }
    }
    metadata.is_file() && !metadata.file_type().is_symlink()
}

pub(super) fn open_regular(path: &Path, create: bool) -> Result<File> {
    check_regular(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    if create {
        options.write(true).create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = retry_file_access(|| options.open(path).map_err(GbError::Io))?;
    if !is_regular(&file.metadata()?) {
        return Err(GbError::Config(
            "Configuration and lock paths must be regular files.".into(),
        ));
    }
    Ok(file)
}

pub(super) fn write_config_file(path: &Path, content: &str) -> Result<()> {
    replace_config_file(path, |file| file.write_all(content.as_bytes()))
}

fn replace_config_file(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> Result<()> {
    check_regular(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| GbError::Config("Configuration has no parent directory.".into()))?;
    #[cfg(unix)]
    let directory = File::open(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".config.toml-")
        .tempfile_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    // A failed persist returns ownership of the same synced temporary file.
    // Keep it across retries; never remove the old config to make room.
    let mut pending = Some(temporary);
    let _persisted = retry_file_access(|| {
        check_regular(path)?;
        match pending
            .take()
            .expect("temporary file is retained until commit")
            .persist(path)
        {
            Ok(file) => Ok(file),
            Err(err) => {
                pending = Some(err.file);
                Err(GbError::Io(err.error))
            }
        }
    })?;
    #[cfg(unix)]
    directory.sync_all().map_err(|err| GbError::Config(format!(
        "Configuration was replaced, but syncing its directory failed: {err}. Reload configuration before retrying."
    )))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn replacement_retries_until_a_reader_releases_delete_sharing() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "old").unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE, without DELETE.
            .open(&path)
            .unwrap();
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            drop(reader);
        });
        let result = write_config_file(&path, "new");
        release.join().unwrap();
        result.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn replacement_preserves_old_config_when_delete_sharing_stays_blocked() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "old").unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(&path)
            .unwrap();
        assert!(write_config_file(&path, "new").is_err());
        drop(reader);
        assert_eq!(fs::read_to_string(&path).unwrap(), "old");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn partial_write_failure_preserves_previous_config_and_cleans_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "original configuration").unwrap();
        let result = replace_config_file(&path, |file| {
            file.write_all(b"partial new secret")?;
            Err(io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original configuration");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn rejected_change_preserves_bytes_and_invalid_toml_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        for content in ["# preserve comments\n[profiles.work]\n", "token = [invalid"] {
            fs::write(&path, content).unwrap();
            let result: Result<()> = update_config(&path, |config| {
                config.default_profile = Some("work".into());
                Err(GbError::Config("rejected".into()))
            });
            assert!(result.is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[test]
    fn no_op_does_not_rewrite_config_or_create_an_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        update_config(&path, |_| Ok(())).unwrap();
        assert!(!path.exists());
        fs::write(&path, "# keep formatting\n[profiles.work]\n").unwrap();
        update_config(&path, |_| Ok(())).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# keep formatting\n[profiles.work]\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_config_and_lock_symlinks_without_modifying_their_targets() {
        use std::os::unix::fs::symlink;
        for lock_link in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            let outside = dir.path().join("outside");
            fs::write(&outside, "# never change this\n[profiles.work]\n").unwrap();
            let link = if lock_link {
                path.with_file_name("config.toml.lock")
            } else {
                path.clone()
            };
            symlink(&outside, &link).unwrap();
            let result = update_config(&path, |config| {
                config.default_profile = Some("work".into());
                Ok(())
            });
            assert!(result.is_err());
            assert_eq!(
                fs::read_to_string(&outside).unwrap(),
                "# never change this\n[profiles.work]\n"
            );
            assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
        }
    }

    #[cfg(unix)]
    #[test]
    fn successful_no_op_hardens_existing_config_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "# preserve bytes\n[profiles.work]\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        update_config(&path, |_| Ok(())).unwrap();
        for file in [&path, &path.with_file_name("config.toml.lock")] {
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "# preserve bytes\n[profiles.work]\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_rename_preserves_previous_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "old").unwrap();
        let result = replace_config_file(&path, |file| {
            file.write_all(b"new")?;
            let temporary = fs::read_dir(dir.path())?
                .find_map(|entry| {
                    let entry = entry.ok()?;
                    entry
                        .file_name()
                        .to_str()?
                        .starts_with(".config.toml-")
                        .then(|| entry.path())
                })
                .unwrap();
            fs::remove_file(temporary)?;
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "old");
    }
}
