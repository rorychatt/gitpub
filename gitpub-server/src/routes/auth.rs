use crate::auth;
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::RegisterRequest>,
) -> Result<(StatusCode, Json<auth::LoginResponse>), auth::AuthError> {
    let users = state.users.read().await;
    if users.contains_key(&req.username) {
        return Err(auth::AuthError::UserAlreadyExists);
    }
    drop(users);

    let password_hash = auth::hash_password(&req.password)?;
    let user = gitpub_core::User::new(req.username.clone(), req.email.clone(), password_hash);

    let token = auth::generate_jwt(&user)?;

    let mut users = state.users.write().await;
    users.insert(req.username.clone(), user.clone());

    Ok((
        StatusCode::CREATED,
        Json(auth::LoginResponse {
            token,
            user: user.into(),
        }),
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::LoginRequest>,
) -> Result<Json<auth::LoginResponse>, auth::AuthError> {
    let users = state.users.read().await;
    let user = users
        .get(&req.username)
        .ok_or(auth::AuthError::InvalidCredentials)?;

    let is_valid = auth::verify_password(&req.password, &user.password_hash)?;
    if !is_valid {
        return Err(auth::AuthError::InvalidCredentials);
    }

    let token = auth::generate_jwt(user)?;

    Ok(Json(auth::LoginResponse {
        token,
        user: user.clone().into(),
    }))
}

pub async fn get_current_user(
    State(state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
) -> Result<Json<auth::UserInfo>, auth::AuthError> {
    let users = state.users.read().await;
    let user = users
        .get(&auth.claims.username)
        .ok_or(auth::AuthError::InvalidToken)?;

    Ok(Json(user.clone().into()))
}
