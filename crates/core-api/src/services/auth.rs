use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User, UserResponse};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // user id
    email: String,
    role: String,
    exp: usize, // expiration time
}

pub async fn register(db: &PgPool, req: RegisterRequest, jwt_secret: &str) -> Result<AuthResponse, AppError> {
    // 1. Hash password dengan Argon2
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();

    // 2. Insert ke database
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash, full_name) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.full_name)
    .fetch_optional(db)
    .await?;

    let user = user.ok_or(AppError::Internal)?;

    // 3. Generate JWT
    let token = generate_jwt(&user, jwt_secret)?;

    Ok(AuthResponse {
        token,
        user: UserResponse::from(user),
    })
}

pub async fn login(db: &PgPool, req: LoginRequest, jwt_secret: &str) -> Result<AuthResponse, AppError> {
    // 1. Cari user by email
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND status = 'active'")
        .bind(&req.email)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::InvalidCredentials)?;

    // 2. Verify password
    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|_| AppError::Internal)?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::InvalidCredentials)?;

    // 3. Generate JWT
    let token = generate_jwt(&user, jwt_secret)?;

    Ok(AuthResponse {
        token,
        user: UserResponse::from(user),
    })
}

fn generate_jwt(user: &User, secret: &str) -> Result<String, AppError> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize + 86400; // 24 jam

    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal)
}
