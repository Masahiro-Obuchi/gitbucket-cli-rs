mod instance;
mod model;
mod resolve;
mod store;

pub(crate) use instance::{instance_key, instance_protocol};
pub(crate) use model::ResolvedCredential;
pub use model::{AuthConfig, CredentialSource, HostConfig, ProfileConfig, Scope};
#[cfg(test)]
mod tests;
