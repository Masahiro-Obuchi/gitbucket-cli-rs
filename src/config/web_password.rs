use crate::error::{GbError, Result};

#[cfg(target_os = "linux")]
#[path = "web_password/linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "web_password/macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "web_password/windows.rs"]
mod platform;

#[derive(Clone)]
pub(crate) struct Key {
    instance: String,
    user: String,
}

impl Key {
    pub(crate) fn new(instance: &str, user: &str) -> Result<Self> {
        crate::target::validate_component(user)
            .map_err(|_| GbError::Config("Invalid GitBucket Web username.".into()))?;
        Ok(Self {
            instance: super::auth::instance_key(instance, "https")?,
            user: user.to_ascii_lowercase(),
        })
    }

    pub(crate) fn instance(&self) -> &str {
        &self.instance
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    pub(crate) async fn read(&self) -> Result<Option<String>> {
        let read = tokio::time::timeout(std::time::Duration::from_secs(3), self.read_bytes());
        let value = tokio::select! {
            biased;
            _ = crate::cancel::notified() => return Err(GbError::Cancelled),
            result = read => result
                .map_err(|_| error("timeout", "Reading the Web password timed out."))??,
        };

        value
            .map(|bytes| {
                String::from_utf8(bytes).map_err(|_| {
                    error(
                        "invalid_password",
                        "The stored Web password is not UTF-8. Register it again.",
                    )
                })
            })
            .transpose()
    }

    #[cfg(any(target_os = "macos", windows))]
    async fn read_bytes(&self) -> Result<Option<Vec<u8>>> {
        // Native calls are synchronous. An OS thread lets the deadline and
        // cancellation take effect without waiting for Tokio to join the call.
        let key = self.clone();
        let (send, receive) = tokio::sync::oneshot::channel();
        std::thread::Builder::new().spawn(move || {
            let _ = send.send(platform::read(&key));
        })?;
        receive
            .await
            .map_err(|_| error("read_failed", "Reading the Web password failed."))?
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    async fn read_bytes(&self) -> Result<Option<Vec<u8>>> {
        platform::read(self).await
    }

    pub(crate) async fn save(&self, password: &str) -> Result<()> {
        if crate::cancel::requested() {
            return Err(GbError::Cancelled);
        }
        platform::save(self, password.as_bytes()).await
    }

    pub(crate) async fn delete(&self) -> Result<bool> {
        if crate::cancel::requested() {
            return Err(GbError::Cancelled);
        }
        platform::delete(self).await
    }
}

fn error(cause: &'static str, message: &'static str) -> GbError {
    GbError::CredentialStore { cause, message }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::{error, Key, Result};

    pub async fn read(_: &Key) -> Result<Option<Vec<u8>>> {
        Err(error(
            "unsupported",
            "This platform has no supported credential store.",
        ))
    }

    pub async fn save(_: &Key, _: &[u8]) -> Result<()> {
        Err(error(
            "unsupported",
            "This platform has no supported credential store.",
        ))
    }

    pub async fn delete(_: &Key) -> Result<bool> {
        Err(error(
            "unsupported",
            "This platform has no supported credential store.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Key;

    #[test]
    fn credentials_use_the_normalized_full_instance_and_web_actor() {
        let key = Key::new("https://Example.test:443/Bucket/api/v3", "Alice").unwrap();
        assert_eq!(key.instance, "https://example.test/Bucket");
        assert_eq!(key.user, "alice");
        assert!(Key::new("https://example.test", "").is_err());
        assert!(Key::new("https://user:secret@example.test", "alice").is_err());
    }
}
