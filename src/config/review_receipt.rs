//! Immutable, durable intent reservation: an uncertain request is never resent.
use crate::domain::review_write::invalid;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};

#[derive(Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub fingerprint: String,
    pub before_ids: Vec<u64>,
}

pub(crate) fn digest(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn ordinary(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    !metadata.file_type().is_symlink()
}

pub(crate) fn reserve(
    key: &str,
    fingerprint: String,
    before_ids: Vec<u64>,
) -> Result<(Receipt, bool)> {
    let dir = crate::config::ensure_config_dir()?.join("review-writes");
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    match builder.create(&dir) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    let metadata = fs::symlink_metadata(&dir)?;
    if !metadata.is_dir() || !ordinary(&metadata) {
        return Err(invalid(
            "Review receipt directory must be an ordinary directory, not a symlink/reparse point.",
        ));
    }
    let path = dir.join(format!("{key}.json"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    match options.open(&path) {
        Ok(mut file) => {
            let receipt = Receipt {
                fingerprint,
                before_ids,
            };
            file.write_all(&serde_json::to_vec(&receipt)?)?;
            file.sync_all()?;
            #[cfg(unix)]
            {
                std::fs::File::open(&dir)?.sync_all()?;
            }
            Ok((receipt, true))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_file() || !ordinary(&metadata) {
                return Err(invalid("Refusing a non-regular review receipt."));
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let mut bytes = Vec::new();
            options
                .open(&path)?
                .take(1024 * 1024)
                .read_to_end(&mut bytes)?;
            let receipt: Receipt = serde_json::from_slice(&bytes)?;
            if receipt.fingerprint != fingerprint {
                return Err(invalid("This --request-id was already reserved for a different review request. Inspect the earlier result before choosing a new ID."));
            }
            Ok((receipt, false))
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservation_keeps_the_first_baseline_and_refuses_corruption_or_changed_intent() {
        let env = crate::test_support::TestEnv::new();
        let dir = tempfile::tempdir().unwrap();
        env.set("GB_CONFIG_DIR", dir.path());
        let key = digest(&"intent").unwrap();
        let (_, fresh) = reserve(&key, "fingerprint".into(), vec![1, 2]).unwrap();
        assert!(fresh);
        let (receipt, fresh) = reserve(&key, "fingerprint".into(), vec![1, 2, 3]).unwrap();
        assert!(!fresh);
        assert_eq!(receipt.before_ids, vec![1, 2]);
        assert!(reserve(&key, "changed".into(), Vec::new()).is_err());
        std::fs::write(
            dir.path().join("review-writes").join(format!("{key}.json")),
            "incomplete",
        )
        .unwrap();
        assert!(reserve(&key, "fingerprint".into(), Vec::new()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn reservation_refuses_symlink_directories() {
        let env = crate::test_support::TestEnv::new();
        let dir = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        env.set("GB_CONFIG_DIR", dir.path());
        std::os::unix::fs::symlink(target.path(), dir.path().join("review-writes")).unwrap();
        assert!(reserve(
            &digest(&"intent").unwrap(),
            "fingerprint".into(),
            Vec::new()
        )
        .is_err());
        assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 0);
    }
}
