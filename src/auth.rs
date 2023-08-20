use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use anyhow::anyhow;
use axum::{
    extract::State,
    headers::{Cookie, HeaderMapExt},
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use sqlx::{FromRow, PgPool};
use tokio::sync::Mutex;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::User;

#[derive(Clone)]
struct AuthHandler {
    registration_data: Arc<Mutex<HashMap<Uuid, (PasskeyRegistration, String)>>>,
    signin_data: Arc<Mutex<HashMap<Uuid, PasskeyAuthentication>>>,
    website_data: Arc<webauthn_rs::Webauthn>,
}

impl AuthHandler {
    async fn start_registration(
        &self,
        user: &User,
    ) -> anyhow::Result<(CreationChallengeResponse, Uuid)> {
        let (ccr, skr) = self
            .website_data
            .start_passkey_registration(user.id, &user.name, &user.name, None)?;

        let auth_id = Uuid::new_v4();

        self.registration_data
            .lock()
            .await
            .insert(auth_id, (skr, user.name.clone()));

        let data = self.registration_data.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            data.lock().await.remove(&auth_id);
        });

        Ok((ccr, auth_id))
    }

    async fn complete_registration(
        &self,
        id: Uuid,
        reg_info: &RegisterPublicKeyCredential,
    ) -> anyhow::Result<(Passkey, String)> {
        let mut state = self.registration_data.lock().await;

        let (data, username) = state
            .remove(&id)
            .ok_or(anyhow!("Could not find existing registration"))?;

        Ok((
            self.website_data
                .finish_passkey_registration(&reg_info, &data)?,
            username,
        ))
    }

    async fn start_login(
        &self,
        passkeys: &[Passkey],
    ) -> anyhow::Result<(RequestChallengeResponse, Uuid)> {
        let (rcr, state) = self.website_data.start_passkey_authentication(passkeys)?;

        let auth_id = Uuid::new_v4();

        self.signin_data.lock().await.insert(auth_id, state);

        let data = self.registration_data.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            data.lock().await.remove(&auth_id);
        });

        Ok((rcr, auth_id))
    }

    async fn complete_login(
        &self,
        id: Uuid,
        auth_info: &PublicKeyCredential,
    ) -> anyhow::Result<AuthenticationResult> {
        Ok(self.website_data.finish_passkey_authentication(
            &auth_info,
            &self
                .signin_data
                .lock()
                .await
                .remove(&id)
                .ok_or(anyhow!("Unable to find sign in state"))?,
        )?)
    }
}

#[derive(Clone)]
pub struct Auth {
    db: PgPool,
    auth: AuthHandler,
}

#[derive(FromRow)]
struct PasskeyRow {
    data: String,
    username: String,
}

#[derive(FromRow)]
struct SessionToken {
    id: Uuid,
}

impl Auth {
    pub fn new(db: PgPool) -> Self {
        let url;

        let builder = if cfg!(debug_assertions) {
            url = Url::parse("http://localhost:8000/").unwrap();
            WebauthnBuilder::new("localhost", &url)
        } else {
            url = Url::parse("https://htmx-intro.shuttleapp.rs/").unwrap();
            WebauthnBuilder::new("htmx-intro.shuttleapp.rs", &url)
        };

        Self {
            db,
            auth: AuthHandler {
                registration_data: Arc::new(Mutex::new(HashMap::new())),
                signin_data: Arc::new(Mutex::new(HashMap::new())),
                website_data: Arc::new(builder.unwrap().build().unwrap()),
            },
        }
    }
    pub async fn start_register(
        &self,
        name: &str,
    ) -> anyhow::Result<(CreationChallengeResponse, Uuid)> {
        let user: User = sqlx::query_as("INSERT INTO users (name) VALUES ($1) RETURNING *")
            .bind(name)
            .fetch_one(&self.db)
            .await?;

        self.auth.start_registration(&user).await
    }

    pub async fn finish_register(
        &self,
        id: Uuid,
        reg_info: RegisterPublicKeyCredential,
    ) -> anyhow::Result<()> {
        let (passkey, username) = self.auth.complete_registration(id, &reg_info).await?;

        sqlx::query("INSERT INTO passkeys (id, data, username) VALUES ($1, $2, $3)")
            .bind(&reg_info.raw_id.0)
            .bind(serde_json::to_string(&passkey)?)
            .bind(username)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    pub async fn start_login(
        &self,
        name: &str,
    ) -> anyhow::Result<(RequestChallengeResponse, Uuid)> {
        let raw_keys: Vec<PasskeyRow> =
            sqlx::query_as("SELECT * FROM passkeys WHERE username = $1")
                .bind(name)
                .fetch_all(&self.db)
                .await?;

        let keys = raw_keys
            .iter()
            .map(|row| serde_json::from_str(&row.data))
            .collect::<Result<Vec<Passkey>, serde_json::Error>>()?;

        self.auth.start_login(&keys).await
    }

    pub async fn finish_login(
        &self,
        id: Uuid,
        auth_info: PublicKeyCredential,
    ) -> anyhow::Result<Uuid> {
        let result = self.auth.complete_login(id, &auth_info).await?;

        let row: PasskeyRow = sqlx::query_as("SELECT * FROM passkeys WHERE id = $1")
            .bind(&result.cred_id().0)
            .fetch_one(&self.db)
            .await?;

        if result.needs_update() {
            let mut passkey: Passkey = serde_json::from_str(&row.data)?;

            if passkey
                .update_credential(&result)
                .ok_or(anyhow!("Credential id did not match when updating"))?
            {
                sqlx::query("UPDATE passkeys SET data = $1 WHERE id = $2")
                    .bind(serde_json::to_string(&passkey)?)
                    .bind(&result.cred_id().0)
                    .execute(&self.db)
                    .await?;
            }
        }

        let token: SessionToken =
            sqlx::query_as("INSERT INTO session_tokens (username) VALUES ($1) RETURNING *")
                .bind(row.username)
                .fetch_one(&self.db)
                .await?;

        Ok(token.id)
    }

    pub async fn verify_token(&self, id: Uuid) -> anyhow::Result<User> {
        Ok(sqlx::query_as(
            "SELECT * FROM users WHERE name = (SELECT username FROM session_tokens WHERE id = $1)",
        )
        .bind(id)
        .fetch_one(&self.db)
        .await?)
    }

    fn handle_unauthorized<B>(req: &Request<B>) -> Response {
        if req.method() == Method::GET {
            Redirect::to("/login").into_response()
        } else {
            StatusCode::UNAUTHORIZED.into_response()
        }
    }

    pub async fn layer<B>(
        State(auth): State<Auth>,
        mut req: Request<B>,
        next: Next<B>,
    ) -> Response {
        let Some(uuid) = req.headers().typed_get::<Cookie>().and_then(|cookies| cookies.get("SessionToken").map(|str| str.to_owned())).and_then(|token| Uuid::from_str(&token).ok()) else {
            return Self::handle_unauthorized(&req);
        };

        if let Ok(user) = auth.verify_token(uuid).await {
            req.extensions_mut().insert(user);

            next.run(req).await
        } else {
            Self::handle_unauthorized(&req)
        }
    }
}
