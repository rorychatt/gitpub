use anyhow::{anyhow, Result};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use gitpub_core::User;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;

const JWT_EXPIRATION_HOURS: i64 = 24;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub username: String,
    pub exp: i64,
}

pub fn create_jwt(user_id: &str, username: &str) -> Result<String> {
    let secret = env::var("JWT_SECRET")
        .map_err(|_| anyhow!("JWT_SECRET environment variable not set"))?;

    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(JWT_EXPIRATION_HOURS))
        .ok_or_else(|| anyhow!("Failed to calculate expiration time"))?
        .timestamp();

    let claims = Claims {
        user_id: user_id.to_string(),
        username: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow!("Failed to create JWT: {}", e))
}

pub fn validate_jwt(token: &str) -> Result<Claims> {
    let secret = env::var("JWT_SECRET")
        .map_err(|_| anyhow!("JWT_SECRET environment variable not set"))?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| anyhow!("Failed to validate JWT: {}", e))?;

    Ok(token_data.claims)
}

pub struct AuthUser(pub User);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidToken)?;

        let claims = validate_jwt(token).map_err(|_| AuthError::InvalidToken)?;

        let user = User {
            id: claims.user_id,
            username: claims.username,
            email: String::new(),
            password_hash: String::new(),
            created_at: 0,
        };

        Ok(AuthUser(user))
    }
}

pub enum AuthError {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authorization token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid authorization token"),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_jwt_valid_token() {
        env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");
        let result = create_jwt("user123", "testuser");
        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn test_validate_jwt_valid_token() {
        env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");
        let token = create_jwt("user123", "testuser").expect("Failed to create JWT");
        let result = validate_jwt(&token);
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.user_id, "user123");
        assert_eq!(claims.username, "testuser");
    }

    #[test]
    fn test_validate_jwt_invalid_signature() {
        env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");
        let token = create_jwt("user123", "testuser").expect("Failed to create JWT");

        env::set_var("JWT_SECRET", "different_secret_key_at_least_32_characters");
        let result = validate_jwt(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jwt_expired_token() {
        env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");

        let expired_claims = Claims {
            user_id: "user123".to_string(),
            username: "testuser".to_string(),
            exp: chrono::Utc::now().timestamp() - 3600,
        };

        let token = encode(
            &Header::default(),
            &expired_claims,
            &EncodingKey::from_secret(b"test_secret_key_at_least_32_characters_long"),
        )
        .expect("Failed to create expired token");

        let result = validate_jwt(&token);
        assert!(result.is_err());
    }
}
