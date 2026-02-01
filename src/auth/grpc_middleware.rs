//! gRPC authentication middleware
//!
//! Extracts and validates JWT tokens from gRPC request metadata.

use tonic::{Request, Status};

use super::jwt::JwtManager;
use super::models::Claims;

/// Key for the authorization metadata
pub const AUTH_HEADER: &str = "authorization";

/// Extract JWT token from gRPC request metadata
pub fn extract_token<T>(request: &Request<T>) -> Option<&str> {
    request
        .metadata()
        .get(AUTH_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Validate JWT token and return claims
pub fn validate_token(jwt: &JwtManager, token: &str) -> Result<Claims, Status> {
    jwt.verify_token(token)
        .map(|data| data.claims)
        .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))
}

/// Extract and validate token from request, returning claims
pub fn authenticate_request<T>(jwt: &JwtManager, request: &Request<T>) -> Result<Claims, Status> {
    let token = extract_token(request)
        .ok_or_else(|| Status::unauthenticated("Missing authorization token"))?;

    validate_token(jwt, token)
}

/// Optional authentication - returns None if no token, error if invalid token
pub fn try_authenticate_request<T>(
    jwt: &JwtManager,
    request: &Request<T>,
) -> Result<Option<Claims>, Status> {
    match extract_token(request) {
        Some(token) => validate_token(jwt, token).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtConfig;
    use tonic::metadata::MetadataValue;

    fn create_test_jwt() -> JwtManager {
        JwtManager::new(JwtConfig::new("test-secret".to_string(), 24))
    }

    #[test]
    fn test_extract_token_valid() {
        let jwt = create_test_jwt();
        let token = jwt
            .create_token("user_123", "test@example.com", "testuser")
            .unwrap();

        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTH_HEADER,
            MetadataValue::try_from(format!("Bearer {}", token)).unwrap(),
        );

        let extracted = extract_token(&request);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap(), token);
    }

    #[test]
    fn test_extract_token_missing() {
        let request = Request::new(());
        let extracted = extract_token(&request);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_authenticate_request_valid() {
        let jwt = create_test_jwt();
        let token = jwt
            .create_token("user_123", "test@example.com", "testuser")
            .unwrap();

        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTH_HEADER,
            MetadataValue::try_from(format!("Bearer {}", token)).unwrap(),
        );

        let claims = authenticate_request(&jwt, &request).unwrap();
        assert_eq!(claims.sub, "user_123");
        assert_eq!(claims.email, "test@example.com");
    }

    #[test]
    fn test_authenticate_request_invalid() {
        let jwt = create_test_jwt();

        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTH_HEADER,
            MetadataValue::try_from("Bearer invalid.token.here").unwrap(),
        );

        let result = authenticate_request(&jwt, &request);
        assert!(result.is_err());
    }
}
