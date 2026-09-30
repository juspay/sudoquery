use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::error::KeycloakError;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

#[derive(Clone)]
pub struct KeycloakAdmin {
    pub base_url: String,
    pub realm: String,
    admin_username: String,
    admin_password: String,
    client: Client,
    cached_token: Arc<RwLock<Option<CachedToken>>>,
}

impl KeycloakAdmin {
    pub fn new(base_url: &str, admin_username: &str, admin_password: &str, realm: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            realm: realm.to_string(),
            admin_username: admin_username.to_string(),
            admin_password: admin_password.to_string(),
            client: Client::new(),
            cached_token: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn admin_token(&self) -> Result<String, KeycloakError> {
        {
            let cache = self.cached_token.read().await;
            if let Some(ref t) = *cache {
                if t.expires_at > std::time::Instant::now() {
                    return Ok(t.token.clone());
                }
            }
        }

        // Admin auth uses master realm (for managing users in any realm)
        let url = format!("{}/realms/master/protocol/openid-connect/token", self.base_url);

        let res = self.client.post(&url)
            .form(&[
                ("grant_type", "password"),
                ("client_id", "admin-cli"),
                ("username", &self.admin_username),
                ("password", &self.admin_password),
            ])
            .send().await?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let message = res.text().await.unwrap_or_default();
            return Err(KeycloakError::Api { status, message });
        }

        let token_res: TokenResponse = res.json().await?;

        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(token_res.expires_in.saturating_sub(10));

        let mut cache = self.cached_token.write().await;
        *cache = Some(CachedToken { token: token_res.access_token.clone(), expires_at });

        Ok(token_res.access_token)
    }

    pub async fn post<T: serde::Serialize>(&self, path: &str, body: &T) -> Result<reqwest::Response, KeycloakError> {
        let token = self.admin_token().await?;
        Ok(self.client.post(&format!("{}{}", self.base_url, path))
            .bearer_auth(&token).json(body).send().await?)
    }

    pub async fn put<T: serde::Serialize>(&self, path: &str, body: &T) -> Result<reqwest::Response, KeycloakError> {
        let token = self.admin_token().await?;
        Ok(self.client.put(&format!("{}{}", self.base_url, path))
            .bearer_auth(&token).json(body).send().await?)
    }

    pub async fn get(&self, path: &str) -> Result<reqwest::Response, KeycloakError> {
        let token = self.admin_token().await?;
        Ok(self.client.get(&format!("{}{}", self.base_url, path))
            .bearer_auth(&token).send().await?)
    }

    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, KeycloakError> {
        let token = self.admin_token().await?;
        Ok(self.client.delete(&format!("{}{}", self.base_url, path))
            .bearer_auth(&token).send().await?)
    }

    pub async fn check(res: reqwest::Response) -> Result<reqwest::Response, KeycloakError> {
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let message = res.text().await.unwrap_or_default();
            return Err(KeycloakError::Api { status, message });
        }
        Ok(res)
    }
}
