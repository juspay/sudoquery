use serde::Deserialize;
use super::{admin::KeycloakAdmin, error::KeycloakError};

#[derive(Deserialize, Debug)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub enabled: bool,
    #[serde(rename = "emailVerified")]
    pub email_verified: bool,
}

impl KeycloakAdmin {
    pub async fn create_user(&self, username: &str, email: &str) -> Result<String, KeycloakError> {
        let body = serde_json::json!({
            "username": username,
            "email": email,
            "enabled": true,
            "emailVerified": true,
            "requiredActions": []
        });

        let res = self.post(&format!("/admin/realms/{}/users", self.realm), &body).await?;
        let res = KeycloakAdmin::check(res).await?;

        let user_id = res.headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|loc| loc.split('/').last())
            .map(|s| s.to_string())
            .ok_or_else(|| KeycloakError::Parse("Missing Location header".to_string()))?;

        Ok(user_id)
    }

    pub async fn set_password(&self, user_id: &str, password: &str, temporary: bool) -> Result<(), KeycloakError> {
        let body = serde_json::json!({
            "type": "password",
            "value": password,
            "temporary": temporary,
        });
        let res = self.put(&format!("/admin/realms/{}/users/{}/reset-password", self.realm, user_id), &body).await?;
        KeycloakAdmin::check(res).await?;

        // Clear any required actions after setting password
        let body = serde_json::json!({
            "requiredActions": []
        });
        let res = self.put(&format!("/admin/realms/{}/users/{}", self.realm, user_id), &body).await?;
        KeycloakAdmin::check(res).await?;

        Ok(())
    }

    pub async fn get_user(&self, user_id: &str) -> Result<UserInfo, KeycloakError> {
        let res = self.get(&format!("/admin/realms/{}/users/{}", self.realm, user_id)).await?;
        let res = KeycloakAdmin::check(res).await?;
        Ok(res.json().await?)
    }

    pub async fn find_users_by_email(&self, email: &str) -> Result<Vec<UserInfo>, KeycloakError> {
        let res = self.get(&format!("/admin/realms/{}/users?email={}&exact=true", self.realm, email)).await?;
        let res = KeycloakAdmin::check(res).await?;
        Ok(res.json().await?)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), KeycloakError> {
        let res = self.delete(&format!("/admin/realms/{}/users/{}", self.realm, user_id)).await?;
        KeycloakAdmin::check(res).await?;
        Ok(())
    }
}
