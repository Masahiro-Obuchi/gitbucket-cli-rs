use std::{collections::HashMap, fmt::Write as _};

use keyring_core::{api::CredentialStoreApi, Entry, Error};
use windows_native_keyring_store::Store;

use super::{error, Key, Result};

fn target(key: &Key) -> String {
    // Credential Manager target names are case-insensitive. Hash the key so
    // case-sensitive deployment paths still select different credentials.
    let input = format!("{}\0{}", key.instance, key.user);
    let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
    let mut name = String::from("gitbucket-cli/web-password/");
    for byte in digest.as_ref() {
        let _ = write!(name, "{byte:02x}");
    }
    name
}

fn entry(key: &Key) -> Result<Entry> {
    let target = target(key);
    let modifiers = HashMap::from([("target", target.as_str()), ("persistence", "Local")]);
    Store::new()
        .and_then(|store| store.build("gitbucket-cli", &key.user, Some(&modifiers)))
        .map_err(|_| manager_error())
}

fn manager_error() -> crate::error::GbError {
    error(
        "credential_manager_failed",
        "Check that Windows Credential Manager is available in this session.",
    )
}

pub(super) fn read(key: &Key) -> Result<Option<Vec<u8>>> {
    // The stored blob contains the password's UTF-8 bytes.
    match entry(key)?.get_secret() {
        Ok(secret) => Ok(Some(secret)),
        Err(Error::NoEntry) => Ok(None),
        Err(_) => Err(manager_error()),
    }
}

pub(super) async fn save(key: &Key, password: &[u8]) -> Result<()> {
    match entry(key)?.set_secret(password) {
        Ok(()) => Ok(()),
        Err(Error::TooLong(..)) => Err(error(
            "password_too_long",
            "The password is too long for Windows Credential Manager.",
        )),
        Err(_) => Err(manager_error()),
    }
}

pub(super) async fn delete(key: &Key) -> Result<bool> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(Error::NoEntry) => Ok(false),
        Err(_) => Err(manager_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_matches_the_v0_8_0_credential_name() {
        let key = Key::new("https://example.test/Bucket", "Alice").unwrap();
        assert_eq!(
            target(&key),
            "gitbucket-cli/web-password/ea1ca8a4ef69dba91b9401da56aa59176f1084c48a1c569bf82b931cdad004f5"
        );
    }

    #[tokio::test]
    #[ignore = "uses a temporary Windows Credential Manager entry"]
    async fn windows_credential_manager_round_trip() {
        struct Cleanup(Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let nonce = temp.path().file_name().unwrap().to_str().unwrap();
        let instance = format!("https://keyring-test.invalid/{nonce}");
        let key = Key::new(&instance, "Alice").unwrap();
        assert!(read(&key).unwrap().is_none());
        let cleanup = Cleanup(entry(&key).unwrap());

        let legacy = "旧テスト-password".as_bytes();
        write_native_entry(&key, legacy);
        assert_eq!(read(&key).unwrap().as_deref(), Some(legacy));

        let replacement = "新-password".as_bytes();
        save(&key, replacement).await.unwrap();
        assert_eq!(read(&key).unwrap().as_deref(), Some(replacement));
        let attributes = cleanup.0.get_attributes().unwrap();
        assert_eq!(attributes["persistence"], "Local");
        assert_eq!(attributes["username"], "alice");

        assert!(delete(&key).await.unwrap());
        assert!(read(&key).unwrap().is_none());
        assert!(!delete(&key).await.unwrap());

        save(&key, replacement).await.unwrap();
        assert_eq!(read(&key).unwrap().as_deref(), Some(replacement));
        assert_eq!(cleanup.0.get_attributes().unwrap()["persistence"], "Local");
        assert!(delete(&key).await.unwrap());
    }

    fn write_native_entry(key: &Key, password: &[u8]) {
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target_name: Vec<u16> = target(key).encode_utf16().chain([0]).collect();
        let mut user: Vec<u16> = key.user.encode_utf16().chain([0]).collect();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target_name.as_mut_ptr(),
            UserName: user.as_mut_ptr(),
            CredentialBlobSize: password.len() as u32,
            CredentialBlob: password.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // Seed the fixture with the same native record layout as v0.8.0.
        assert_ne!(unsafe { CredWriteW(&credential, 0) }, 0);
    }
}
