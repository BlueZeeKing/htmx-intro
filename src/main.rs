use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    extract::FromRef,
    routing::{delete, get, post, put},
    Router,
};
use htmx_intro::routes::*;
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

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_static_folder::StaticFolder(folder = "public")] static_folder: PathBuf,
) -> shuttle_axum::ShuttleAxum {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS users (
            name VARCHAR(200) PRIMARY KEY
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

    let app = Router::new()
        .route("/", get(tasks))
        .route("/login", get(login))
        .route("/submit-name", post(submit))
        .route("/add-task", post(add_task))
        .route("/toggle", put(set_checked))
        .route("/tasks", get(get_tasks))
        .route("/delete", delete(delete_task))
        .nest_service("/public", ServeDir::new(static_folder))
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
        });

    Ok(app.into())
}
