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

    pub(crate) async fn read(&self) -> Result<Option<String>> {
        let value = tokio::time::timeout(std::time::Duration::from_secs(3), platform::read(self))
            .await
            .map_err(|_| error("timeout", "Reading the Web password timed out."))??;
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

    pub(crate) async fn save(&self, password: &str) -> Result<()> {
        platform::save(self, password.as_bytes()).await
    }

    pub(crate) async fn delete(&self) -> Result<bool> {
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
        Ok(None)
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
