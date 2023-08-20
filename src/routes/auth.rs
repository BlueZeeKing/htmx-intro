use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

use crate::{auth::Auth, Error};

#[derive(Deserialize, Serialize)]
pub struct SigninData {
    name: String,
}

pub async fn start_signin(
    State(auth): State<Auth>,
    Json(data): Json<SigninData>,
) -> Result<impl IntoResponse, Error> {
    Ok(Json(auth.start_login(&data.name).await?))
}

pub async fn finish_signin(
    State(auth): State<Auth>,
    Json((id, credential)): Json<(Uuid, PublicKeyCredential)>,
) -> Result<impl IntoResponse, Error> {
    let session = auth.finish_login(id, credential).await?;
    Ok(session.to_string())
}

pub async fn start_register(
    State(auth): State<Auth>,
    Json(data): Json<SigninData>,
) -> Result<impl IntoResponse, Error> {
    Ok(Json(auth.start_register(&data.name).await?))
}

pub async fn finish_register(
    State(auth): State<Auth>,
    Json((id, credential)): Json<(Uuid, RegisterPublicKeyCredential)>,
) -> Result<impl IntoResponse, Error> {
    Ok(Json(auth.finish_register(id, credential).await?))
}
