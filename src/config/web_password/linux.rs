use std::collections::HashMap;

use secret_service::{Collection, EncryptionType, Error, Item, SecretService};

use super::{error, Key, Result};

fn attributes(key: &Key) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", "gitbucket-cli"),
        ("purpose", "web-password"),
        ("instance", key.instance.as_str()),
        ("username", key.user.as_str()),
    ])
}

fn service_error(err: Error) -> crate::error::GbError {
    match err {
        Error::Locked => locked(),
        Error::Unavailable => error(
            "unavailable",
            "No Secret Service is available in this session.",
        ),
        Error::NoResult => error("unavailable", "Secret Service has no default collection."),
        _ => error("service_failed", "The Secret Service operation failed."),
    }
}

fn locked() -> crate::error::GbError {
    error("locked", "Unlock the Secret Service collection and retry.")
}

async fn find<'a>(collection: &'a Collection<'_>, key: &Key) -> Result<Option<Item<'a>>> {
    let mut items = collection
        .search_items(attributes(key))
        .await
        .map_err(service_error)?;
    if items.len() > 1 {
        return Err(error(
            "ambiguous_entry",
            "More than one Web password matches this account.",
        ));
    }
    Ok(items.pop())
}

pub(super) async fn read(key: &Key) -> Result<Option<Vec<u8>>> {
    let service = SecretService::connect(EncryptionType::Dh)
        .await
        .map_err(service_error)?;
    let collection = service
        .get_default_collection()
        .await
        .map_err(service_error)?;
    let Some(item) = find(&collection, key).await? else {
        return Ok(None);
    };
    // The read path never calls Unlock or starts a Secret Service prompt.
    if item.is_locked().await.map_err(service_error)? {
        return Err(locked());
    }
    item.get_secret().await.map(Some).map_err(service_error)
}

pub(super) async fn save(key: &Key, password: &[u8]) -> Result<()> {
    let service = SecretService::connect(EncryptionType::Dh)
        .await
        .map_err(service_error)?;
    let collection = service
        .get_default_collection()
        .await
        .map_err(service_error)?;
    if collection.is_locked().await.map_err(service_error)? {
        return Err(locked());
    }
    find(&collection, key).await?;
    collection
        .create_item(
            &format!("GitBucket Web password for {} @ {}", key.user, key.instance),
            attributes(key),
            password,
            true,
            "text/plain; charset=utf-8",
        )
        .await
        .map_err(service_error)?;
    Ok(())
}

pub(super) async fn delete(key: &Key) -> Result<bool> {
    let service = SecretService::connect(EncryptionType::Dh)
        .await
        .map_err(service_error)?;
    let collection = service
        .get_default_collection()
        .await
        .map_err(service_error)?;
    let Some(item) = find(&collection, key).await? else {
        return Ok(false);
    };
    if item.is_locked().await.map_err(service_error)? {
        return Err(locked());
    }
    item.delete().await.map_err(service_error)?;
    Ok(true)
}
