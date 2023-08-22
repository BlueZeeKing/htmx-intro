use axum::{http::StatusCode, response::IntoResponse};
use serde::Serialize;
use sqlx::{types::Uuid, FromRow};

pub mod auth;
pub mod routes;
pub mod templates;

#[derive(Serialize, FromRow)]
pub struct Task {
    name: String,
    completed: bool,
    id: Uuid,
}

#[derive(Serialize, FromRow, Clone)]
pub struct User {
    name: String,
    id: Uuid,
}

pub struct Error(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("Server error: {}", self.0.to_string());

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Server error: {}", self.0.to_string()),
        )
            .into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
