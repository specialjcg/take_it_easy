//! OAuth2 providers (Google, GitHub, Discord)

use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use reqwest::Client as HttpClient;
use serde::Deserialize;

use super::models::{OAuthProvider, OAuthUserInfo};

/// OAuth configuration for all providers
#[derive(Clone)]
pub struct OAuthConfig {
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub discord_client_id: Option<String>,
    pub discord_client_secret: Option<String>,
    pub redirect_base_url: String,
}

impl OAuthConfig {
    pub fn from_env() -> Self {
        Self {
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").ok(),
            github_client_id: std::env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").ok(),
            discord_client_id: std::env::var("DISCORD_CLIENT_ID").ok(),
            discord_client_secret: std::env::var("DISCORD_CLIENT_SECRET").ok(),
            redirect_base_url: std::env::var("OAUTH_REDIRECT_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
        }
    }

    pub fn is_provider_configured(&self, provider: OAuthProvider) -> bool {
        match provider {
            OAuthProvider::Google => {
                self.google_client_id.is_some() && self.google_client_secret.is_some()
            }
            OAuthProvider::Github => {
                self.github_client_id.is_some() && self.github_client_secret.is_some()
            }
            OAuthProvider::Discord => {
                self.discord_client_id.is_some() && self.discord_client_secret.is_some()
            }
        }
    }
}

/// OAuth manager
pub struct OAuthManager {
    config: OAuthConfig,
    http_client: HttpClient,
}

impl OAuthManager {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http_client: HttpClient::new(),
        }
    }

    /// Get authorization URL for a provider
    pub fn get_auth_url(&self, provider: OAuthProvider) -> Result<(String, CsrfToken), String> {
        let client = self.create_client(provider)?;

        let mut auth_request = client.authorize_url(CsrfToken::new_random);

        // Add scopes based on provider
        auth_request = match provider {
            OAuthProvider::Google => auth_request
                .add_scope(Scope::new("email".to_string()))
                .add_scope(Scope::new("profile".to_string())),
            OAuthProvider::Github => auth_request
                .add_scope(Scope::new("user:email".to_string()))
                .add_scope(Scope::new("read:user".to_string())),
            OAuthProvider::Discord => auth_request
                .add_scope(Scope::new("identify".to_string()))
                .add_scope(Scope::new("email".to_string())),
        };

        let (url, csrf_token) = auth_request.url();
        Ok((url.to_string(), csrf_token))
    }

    /// Exchange authorization code for user info
    pub async fn exchange_code(
        &self,
        provider: OAuthProvider,
        code: &str,
    ) -> Result<OAuthUserInfo, String> {
        let client = self.create_client(provider)?;

        // Exchange code for token
        let token_result = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let access_token = token_result.access_token().secret();

        // Fetch user info based on provider
        match provider {
            OAuthProvider::Google => self.fetch_google_user_info(access_token).await,
            OAuthProvider::Github => self.fetch_github_user_info(access_token).await,
            OAuthProvider::Discord => self.fetch_discord_user_info(access_token).await,
        }
    }

    fn create_client(&self, provider: OAuthProvider) -> Result<BasicClient, String> {
        let (client_id, client_secret, auth_url, token_url) = match provider {
            OAuthProvider::Google => (
                self.config
                    .google_client_id
                    .as_ref()
                    .ok_or("Google client ID not configured")?,
                self.config
                    .google_client_secret
                    .as_ref()
                    .ok_or("Google client secret not configured")?,
                "https://accounts.google.com/o/oauth2/v2/auth",
                "https://oauth2.googleapis.com/token",
            ),
            OAuthProvider::Github => (
                self.config
                    .github_client_id
                    .as_ref()
                    .ok_or("GitHub client ID not configured")?,
                self.config
                    .github_client_secret
                    .as_ref()
                    .ok_or("GitHub client secret not configured")?,
                "https://github.com/login/oauth/authorize",
                "https://github.com/login/oauth/access_token",
            ),
            OAuthProvider::Discord => (
                self.config
                    .discord_client_id
                    .as_ref()
                    .ok_or("Discord client ID not configured")?,
                self.config
                    .discord_client_secret
                    .as_ref()
                    .ok_or("Discord client secret not configured")?,
                "https://discord.com/api/oauth2/authorize",
                "https://discord.com/api/oauth2/token",
            ),
        };

        let redirect_url = format!(
            "{}/auth/callback/{}",
            self.config.redirect_base_url,
            provider.as_str()
        );

        Ok(BasicClient::new(
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
            AuthUrl::new(auth_url.to_string()).map_err(|e| e.to_string())?,
            Some(TokenUrl::new(token_url.to_string()).map_err(|e| e.to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| e.to_string())?))
    }

    async fn fetch_google_user_info(&self, access_token: &str) -> Result<OAuthUserInfo, String> {
        #[derive(Deserialize)]
        struct GoogleUser {
            id: String,
            email: String,
            name: Option<String>,
        }

        let response = self
            .http_client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Google user info: {}", e))?;

        let user: GoogleUser = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Google user info: {}", e))?;

        Ok(OAuthUserInfo {
            provider: OAuthProvider::Google,
            provider_user_id: user.id,
            email: user.email,
            username: user.name.unwrap_or_else(|| "GoogleUser".to_string()),
        })
    }

    async fn fetch_github_user_info(&self, access_token: &str) -> Result<OAuthUserInfo, String> {
        #[derive(Deserialize)]
        struct GithubUser {
            id: i64,
            login: String,
            email: Option<String>,
        }

        #[derive(Deserialize)]
        struct GithubEmail {
            email: String,
            primary: bool,
            verified: bool,
        }

        // Fetch user
        let response = self
            .http_client
            .get("https://api.github.com/user")
            .bearer_auth(access_token)
            .header("User-Agent", "TakeItEasy-App")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch GitHub user info: {}", e))?;

        let user: GithubUser = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse GitHub user info: {}", e))?;

        // If email is not public, fetch from emails endpoint
        let email = if let Some(email) = user.email {
            email
        } else {
            let emails_response = self
                .http_client
                .get("https://api.github.com/user/emails")
                .bearer_auth(access_token)
                .header("User-Agent", "TakeItEasy-App")
                .send()
                .await
                .map_err(|e| format!("Failed to fetch GitHub emails: {}", e))?;

            let emails: Vec<GithubEmail> = emails_response
                .json()
                .await
                .map_err(|e| format!("Failed to parse GitHub emails: {}", e))?;

            emails
                .into_iter()
                .find(|e| e.primary && e.verified)
                .map(|e| e.email)
                .ok_or("No verified primary email found")?
        };

        Ok(OAuthUserInfo {
            provider: OAuthProvider::Github,
            provider_user_id: user.id.to_string(),
            email,
            username: user.login,
        })
    }

    async fn fetch_discord_user_info(&self, access_token: &str) -> Result<OAuthUserInfo, String> {
        #[derive(Deserialize)]
        struct DiscordUser {
            id: String,
            username: String,
            email: Option<String>,
        }

        let response = self
            .http_client
            .get("https://discord.com/api/users/@me")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Discord user info: {}", e))?;

        let user: DiscordUser = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Discord user info: {}", e))?;

        Ok(OAuthUserInfo {
            provider: OAuthProvider::Discord,
            provider_user_id: user.id,
            email: user.email.ok_or("Discord email not available")?,
            username: user.username,
        })
    }

    /// Check which providers are configured
    pub fn get_configured_providers(&self) -> Vec<OAuthProvider> {
        let mut providers = Vec::new();
        if self.config.is_provider_configured(OAuthProvider::Google) {
            providers.push(OAuthProvider::Google);
        }
        if self.config.is_provider_configured(OAuthProvider::Github) {
            providers.push(OAuthProvider::Github);
        }
        if self.config.is_provider_configured(OAuthProvider::Discord) {
            providers.push(OAuthProvider::Discord);
        }
        providers
    }
}

impl Clone for OAuthManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            http_client: HttpClient::new(),
        }
    }
}
