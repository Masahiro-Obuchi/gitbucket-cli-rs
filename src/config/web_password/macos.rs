use security_framework::os::macos::keychain::SecKeychain;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use security_framework_sys::base::errSecItemNotFound;

use super::{error, Key, Result};

fn service(key: &Key) -> String {
    format!("GitBucket Web password ({})", key.instance)
}

fn keychain_error(_: security_framework::base::Error) -> crate::error::GbError {
    error(
        "keychain_failed",
        "Check that the login keychain is available and unlocked.",
    )
}

pub(super) fn read(key: &Key) -> Result<Option<Vec<u8>>> {
    // Keeping the guard alive prevents the native read from showing a dialog.
    let _interaction = if SecKeychain::user_interaction_allowed().map_err(keychain_error)? {
        Some(SecKeychain::disable_user_interaction().map_err(keychain_error)?)
    } else {
        None
    };
    match get_generic_password(&service(key), &key.user) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.code() == errSecItemNotFound => Ok(None),
        Err(err) => Err(keychain_error(err)),
    }
}

pub(super) async fn save(key: &Key, password: &[u8]) -> Result<()> {
    set_generic_password(&service(key), &key.user, password).map_err(keychain_error)
}

pub(super) async fn delete(key: &Key) -> Result<bool> {
    match delete_generic_password(&service(key), &key.user) {
        Ok(()) => Ok(true),
        Err(err) if err.code() == errSecItemNotFound => Ok(false),
        Err(err) => Err(keychain_error(err)),
    }
}
