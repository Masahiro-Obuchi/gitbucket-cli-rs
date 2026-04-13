use crate::api::client::ApiClient;
use crate::error::Result;
use crate::models::user::User;

impl ApiClient {
    /// Get the authenticated user
    pub async fn current_user(&self) -> Result<User> {
        self.get("/user").await
    }
}
