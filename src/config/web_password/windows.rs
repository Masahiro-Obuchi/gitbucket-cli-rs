use std::fmt::Write as _;
use std::ptr;

use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
use windows_sys::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_MAX_CREDENTIAL_BLOB_SIZE,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

use super::{error, Key, Result};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn target(key: &Key) -> Vec<u16> {
    // Credential Manager target names are case-insensitive. Hash the key so
    // case-sensitive deployment paths still select different credentials.
    let input = format!("{}\0{}", key.instance, key.user);
    let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
    let mut name = String::from("gitbucket-cli/web-password/");
    for byte in digest.as_ref() {
        let _ = write!(name, "{byte:02x}");
    }
    wide(&name)
}

fn manager_error() -> crate::error::GbError {
    error(
        "credential_manager_failed",
        "Check that Windows Credential Manager is available in this session.",
    )
}

pub(super) async fn read(key: &Key) -> Result<Option<Vec<u8>>> {
    let target = target(key);
    let mut value = ptr::null_mut();
    // The target is NUL-terminated and CredReadW owns the returned allocation.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut value) } == 0 {
        return if unsafe { GetLastError() } == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(manager_error())
        };
    }
    // A successful CredReadW returns a credential and its blob in one buffer.
    let bytes = unsafe {
        let credential = &*value;
        let size = credential.CredentialBlobSize as usize;
        let bytes = if size == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(credential.CredentialBlob, size).to_vec()
        };
        CredFree(value.cast());
        bytes
    };
    Ok(Some(bytes))
}

pub(super) async fn save(key: &Key, password: &[u8]) -> Result<()> {
    if password.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        return Err(error(
            "password_too_long",
            "The password is too long for Windows Credential Manager.",
        ));
    }
    let mut target = target(key);
    let mut user = wide(&key.user);
    let data = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: password.len() as u32,
        CredentialBlob: password.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: user.as_mut_ptr(),
        ..Default::default()
    };
    // All pointed-to buffers remain valid for the duration of the call.
    if unsafe { CredWriteW(&data, 0) } == 0 {
        return Err(manager_error());
    }
    Ok(())
}

pub(super) async fn delete(key: &Key) -> Result<bool> {
    let target = target(key);
    // The target is NUL-terminated and remains valid during the call.
    if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0 {
        Ok(true)
    } else if unsafe { GetLastError() } == ERROR_NOT_FOUND {
        Ok(false)
    } else {
        Err(manager_error())
    }
}
