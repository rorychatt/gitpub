use axum::{extract::State, http::StatusCode, Json};
use gitpub_core::{Database, User};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::create_jwt;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn register(
    State(db): State<Arc<Database>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if payload.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Password must be at least 8 characters".to_string(),
            }),
        ));
    }

    if !payload.email.contains('@') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid email format".to_string(),
            }),
        ));
    }

    let user = User::new_with_password(
        payload.username.clone(),
        payload.email.clone(),
        &payload.password,
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create user: {}", e),
            }),
        )
    })?;

    let existing = sqlx::query("SELECT id FROM users WHERE username = $1 OR email = $2")
        .bind(&user.username)
        .bind(&user.email)
        .fetch_optional(db.pool())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Username or email already exists".to_string(),
            }),
        ));
    }

    sqlx::query("INSERT INTO users (id, username, email, password_hash, created_at) VALUES ($1, $2, $3, $4, $5)")
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(user.created_at)
        .execute(db.pool())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to insert user: {}", e),
                }),
            )
        })?;

    let token = create_jwt(&user.id, &user.username).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create token: {}", e),
            }),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
        },
    }))
}

pub async fn login(
    State(db): State<Arc<Database>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_record = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT id, username, email, password_hash, created_at FROM users WHERE username = $1",
    )
    .bind(&payload.username)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let user_record = user_record.ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "Invalid credentials".to_string(),
        }),
    ))?;

    let user = User {
        id: user_record.0,
        username: user_record.1,
        email: user_record.2,
        password_hash: user_record.3,
        created_at: user_record.4,
    };

    let is_valid = user.verify_password(&payload.password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Password verification error: {}", e),
            }),
        )
    })?;

    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid credentials".to_string(),
            }),
        ));
    }

    let token = create_jwt(&user.id, &user.username).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create token: {}", e),
            }),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
        },
    }))
}
