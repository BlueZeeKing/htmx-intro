use std::{collections::HashMap, path::PathBuf, sync::Arc};

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
use tera::Tera;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, services::ServeDir};

fn make_to_struct_json() -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, serde_json::Value>| -> tera::Result<serde_json::Value> {
            Ok(serde_json::Value::String(serde_json::to_string(args)?))
        },
    )
}

#[derive(Clone)]
struct AppState {
    db: PgPool,
    templates: Arc<Tera>,
    auth: Auth,
}

impl FromRef<AppState> for Arc<Tera> {
    fn from_ref(app_state: &AppState) -> Arc<Tera> {
        app_state.templates.clone()
    }
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
        .route("/tasks", get(get_tasks))
        .route("/delete", delete(delete_task))
        .layer(middleware::from_fn_with_state(auth.clone(), Auth::layer))
        .route("/login", get(login))
        .route("/start-login", post(start_signin))
        .route("/finish-login", post(finish_signin))
        .route("/start-register", post(start_register))
        .route("/finish-register", post(finish_register))
        .nest_service("/static", ServeDir::new(static_folder))
        .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
        .with_state(AppState {
            db: pool,
            templates: {
                let mut tera = Tera::default();

                tera.add_raw_templates([
                    ("base.html", include_str!("../templates/base.html")),
                    ("macros.html", include_str!("../templates/macros.html")),
                    (
                        "partials/list.html",
                        include_str!("../templates/partials/list.html"),
                    ),
                    ("index.html", include_str!("../templates/index.html")),
                    ("login.html", include_str!("../templates/login.html")),
                ])
                .unwrap();

                tera.register_function("to_struct_json", make_to_struct_json());

                Arc::new(tera)
            },
            auth,
        });

    Ok(app.into())
}
