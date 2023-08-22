use std::path::PathBuf;

use axum::{
    extract::FromRef,
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use htmx_intro::{
    auth::Auth,
    routes::{
        auth::{finish_register, finish_signin, start_register, start_signin},
        *,
    },
};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    db: PgPool,
    auth: Auth,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(app_state: &AppState) -> PgPool {
        app_state.db.clone()
    }
}

impl FromRef<AppState> for Auth {
    fn from_ref(app_state: &AppState) -> Auth {
        app_state.auth.clone()
    }
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_static_folder::StaticFolder] static_folder: PathBuf,
) -> shuttle_axum::ShuttleAxum {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS users (
            name VARCHAR(200) NOT NULL UNIQUE,
            id UUID PRIMARY KEY DEFAULT uuid_generate_v4()
        );
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS passkeys (
            id BYTEA PRIMARY KEY,
            data VARCHAR(1000) NOT NULL,            
            username VARCHAR(200) NOT NULL REFERENCES users(name) ON DELETE CASCADE
        );
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS session_tokens (
            id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
            username VARCHAR(200) NOT NULL REFERENCES users (name) ON DELETE CASCADE
        );
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS tasks (
            name VARCHAR(500) NOT NULL,
            id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
            completed BOOL NOT NULL DEFAULT FALSE,
            username VARCHAR(200) NOT NULL REFERENCES users (name) ON DELETE CASCADE,
            created TIMESTAMP NOT NULL DEFAULT current_timestamp
        );
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    let auth = Auth::new(pool.clone());

    let app = Router::new()
        .route("/", get(tasks))
        .route("/add-task", post(add_task))
        .route("/toggle", put(set_checked))
        .route("/delete", delete(delete_task))
        .layer(middleware::from_fn_with_state(auth.clone(), Auth::layer))
        .route("/login", get(login))
        .route("/start-login", post(start_signin))
        .route("/finish-login", post(finish_signin))
        .route("/start-register", post(start_register))
        .route("/finish-register", post(finish_register))
        .nest_service("/static", ServeDir::new(static_folder))
        .layer(
            ServiceBuilder::new()
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(AppState { db: pool, auth });

    Ok(app.into())
}
