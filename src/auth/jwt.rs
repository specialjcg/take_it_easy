//! JWT token handling

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::Claims;

/// JWT configuration
pub struct JwtConfig {
    secret: String,
    expiration_hours: u64,
}

impl JwtConfig {
    pub fn new(secret: String, expiration_hours: u64) -> Self {
        Self {
            secret,
            expiration_hours,
        }
    }

    pub fn from_env() -> Self {
        let secret =
            std::env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-me".to_string());
        let expiration_hours = std::env::var("JWT_EXPIRATION_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24);
        Self::new(secret, expiration_hours)
    }
}

/// JWT manager
pub struct JwtManager {
    config: JwtConfig,
}

impl JwtManager {
    pub fn new(config: JwtConfig) -> Self {
        Self { config }
    }

    /// Create a new JWT token for a user
    pub fn create_token(
        &self,
        user_id: &str,
        email: &str,
        username: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as usize;

        let expiration = now + (self.config.expiration_hours as usize * 3600);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            username: username.to_string(),
            exp: expiration,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.secret.as_bytes()),
        )
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &Validation::default(),
        )
    }

    /// Extract claims from token without full verification (for debugging)
    pub fn decode_token_unsafe(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut validation = Validation::default();
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &validation,
        )?;

        Ok(token_data.claims)
    }
}

impl Clone for JwtManager {
    fn clone(&self) -> Self {
        Self {
            config: JwtConfig::new(
                self.config.secret.clone(),
                self.config.expiration_hours,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify_token() {
        let config = JwtConfig::new("test-secret".to_string(), 24);
        let manager = JwtManager::new(config);

        let token = manager
            .create_token("user_123", "test@example.com", "testuser")
            .unwrap();

        let verified = manager.verify_token(&token).unwrap();
        assert_eq!(verified.claims.sub, "user_123");
        assert_eq!(verified.claims.email, "test@example.com");
        assert_eq!(verified.claims.username, "testuser");
    }

    #[test]
    fn test_invalid_token() {
        let config = JwtConfig::new("test-secret".to_string(), 24);
        let manager = JwtManager::new(config);

        let result = manager.verify_token("invalid.token.here");
        assert!(result.is_err());
    }
}
