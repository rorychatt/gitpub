use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json, RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use gitpub_core::User;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;

const JWT_SECRET_ENV: &str = "JWT_SECRET";
const TOKEN_EXPIRATION_HOURS: i64 = 24;
const BCRYPT_COST: u32 = 12;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub username: String,
    pub exp: i64,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    TokenExpired,
    InvalidToken,
    UserAlreadyExists,
    MissingToken,
    HashingError,
    JwtSecretMissing,
    JwtSecretTooShort,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::TokenExpired => write!(f, "Token expired"),
            AuthError::InvalidToken => write!(f, "Invalid token"),
            AuthError::UserAlreadyExists => write!(f, "User already exists"),
            AuthError::MissingToken => write!(f, "Missing authorization token"),
            AuthError::HashingError => write!(f, "Password hashing failed"),
            AuthError::JwtSecretMissing => write!(f, "JWT_SECRET environment variable not set"),
            AuthError::JwtSecretTooShort => {
                write!(f, "JWT_SECRET must be at least 32 bytes")
            }
        }
    }
}

impl std::error::Error for AuthError {}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::UserAlreadyExists => (StatusCode::CONFLICT, self.to_string()),
            AuthError::HashingError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            ),
            AuthError::JwtSecretMissing => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration error".to_string(),
            ),
            AuthError::JwtSecretTooShort => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration error".to_string(),
            ),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    bcrypt::hash(password, BCRYPT_COST).map_err(|_| AuthError::HashingError)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    bcrypt::verify(password, hash).map_err(|_| AuthError::HashingError)
}

pub fn get_jwt_secret() -> Result<String, AuthError> {
    let secret = std::env::var(JWT_SECRET_ENV).map_err(|_| AuthError::JwtSecretMissing)?;

    if secret.len() < 32 {
        return Err(AuthError::JwtSecretTooShort);
    }

    Ok(secret)
}

pub fn generate_jwt(user: &User) -> Result<String, AuthError> {
    let secret = get_jwt_secret()?;
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(TOKEN_EXPIRATION_HOURS))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        user_id: user.id.clone(),
        username: user.username.clone(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AuthError::InvalidToken)
}

pub fn validate_jwt(token: &str) -> Result<Claims, AuthError> {
    let secret = get_jwt_secret()?;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|err| {
        if err.to_string().contains("ExpiredSignature") {
            AuthError::TokenExpired
        } else {
            AuthError::InvalidToken
        }
    })
}

pub struct RequireAuth {
    pub claims: Claims,
}

#[async_trait]
impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::MissingToken)?;

        let claims = validate_jwt(bearer.token())?;

        Ok(RequireAuth { claims })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password_generates_valid_bcrypt() {
        let password = "test_password123";
        let hash = hash_password(password).expect("hashing should succeed");
        assert!(hash.starts_with("$2b$") || hash.starts_with("$2a$"));
        assert!(hash.len() > 50);
    }

    #[test]
    fn test_verify_password_accepts_correct_password() {
        let password = "test_password123";
        let hash = hash_password(password).expect("hashing should succeed");
        let result = verify_password(password, &hash).expect("verification should succeed");
        assert!(result);
    }

    #[test]
    fn test_verify_password_rejects_wrong_password() {
        let password = "test_password123";
        let wrong_password = "wrong_password";
        let hash = hash_password(password).expect("hashing should succeed");
        let result = verify_password(wrong_password, &hash).expect("verification should succeed");
        assert!(!result);
    }

    #[test]
    fn test_generate_jwt_creates_valid_token() {
        std::env::set_var(
            JWT_SECRET_ENV,
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hash123".to_string(),
        );

        let token = generate_jwt(&user).expect("JWT generation should succeed");
        assert!(!token.is_empty());
        assert!(token.split('.').count() == 3);
    }

    #[test]
    fn test_validate_jwt_accepts_valid_token() {
        std::env::set_var(
            JWT_SECRET_ENV,
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hash123".to_string(),
        );

        let token = generate_jwt(&user).expect("JWT generation should succeed");
        let claims = validate_jwt(&token).expect("JWT validation should succeed");

        assert_eq!(claims.user_id, user.id);
        assert_eq!(claims.username, user.username);
    }

    #[test]
    fn test_validate_jwt_rejects_invalid_token() {
        std::env::set_var(
            JWT_SECRET_ENV,
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let result = validate_jwt("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_secret_validation() {
        std::env::remove_var(JWT_SECRET_ENV);
        assert!(matches!(get_jwt_secret(), Err(AuthError::JwtSecretMissing)));

        std::env::set_var(JWT_SECRET_ENV, "short");
        assert!(matches!(
            get_jwt_secret(),
            Err(AuthError::JwtSecretTooShort)
        ));

        std::env::set_var(
            JWT_SECRET_ENV,
            "this_is_a_valid_secret_that_is_at_least_32_bytes",
        );
        assert!(get_jwt_secret().is_ok());
    }
}
