use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    headers::Cookie,
    http::{header::SET_COOKIE, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
    Form, Router, TypedHeader,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, FromRow, PgPool};
use tera::{Context, Tera};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, services::ServeDir};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::new("templates/**/*.html").unwrap();

        tera.register_function("to_struct_json", make_to_struct_json());

        tera
    };
}

fn make_to_struct_json() -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, serde_json::Value>| -> tera::Result<serde_json::Value> {
            Ok(serde_json::Value::String(serde_json::to_string(args)?))
        },
    )
}

#[shuttle_runtime::main]
async fn main(#[shuttle_shared_db::Postgres] pool: PgPool) -> shuttle_axum::ShuttleAxum {
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
        .nest_service("/public", ServeDir::new("public"))
        .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
        .with_state(pool.clone());

    Ok(app.into())
}

async fn login() -> Html<String> {
    Html(TEMPLATES.render("login.html", &Context::new()).unwrap())
}

async fn tasks(State(db): State<PgPool>, TypedHeader(cookies): TypedHeader<Cookie>) -> Response {
    let name = if let Some(name) = cookies.get("TasksLoginName") {
        name
    } else {
        return Redirect::to("/login").into_response();
    };

    let tasks: Vec<(String, bool, Uuid)> = dbg!(sqlx::query_as(
        "SELECT name, completed, id FROM tasks WHERE username = $1 ORDER BY created ASC",
    )
    .bind(name)
    .fetch_all(&db)
    .await
    .unwrap());

    let mut context = Context::new();

    context.insert("tasks", &tasks);

    Html(TEMPLATES.render("index.html", &context).unwrap()).into_response()
}

#[derive(Deserialize)]
struct NameQuery {
    name: String,
}

async fn submit(State(db): State<PgPool>, Form(name): Form<NameQuery>) -> impl IntoResponse {
    sqlx::query("INSERT INTO users VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&name.name)
        .execute(&db)
        .await
        .unwrap();

    drop(db);

    (
        [(
            SET_COOKIE.as_str(),
            format!("TasksLoginName={};", name.name),
        )],
        Redirect::to("/"),
    )
}

#[derive(Serialize, FromRow)]
struct Task<'a> {
    name: &'a str,
    completed: bool,
}

#[derive(Deserialize)]
struct AddTaskQuery {
    task: String,
}

async fn add_task(
    State(db): State<PgPool>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Form(task_name): Form<AddTaskQuery>,
) -> Response {
    let name = if let Some(name) = cookies.get("TasksLoginName") {
        name
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let result = sqlx::query("INSERT INTO tasks (name, username) VALUES ($1, $2)")
        .bind(task_name.task)
        .bind(name)
        .execute(&db)
        .await
        .unwrap();

    if result.rows_affected() == 0 {
        StatusCode::CONFLICT.into_response()
    } else {
        (
            StatusCode::CREATED,
            [("HX-Trigger", "reload-incompleted, clear-task-form")],
        )
            .into_response()
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct CheckedQuery {
    id: Uuid,
    completed: bool,
}

async fn set_checked(
    State(db): State<PgPool>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Form(check_info): Form<CheckedQuery>,
) -> Response {
    let name = if let Some(name) = cookies.get("TasksLoginName") {
        name
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    dbg!(&check_info);

    let new_checked = !check_info.completed;

    let result = sqlx::query("UPDATE tasks SET completed = $1 WHERE username = $2 AND id = $3")
        .bind(new_checked)
        .bind(name)
        .bind(dbg!(check_info.id))
        .execute(&db)
        .await
        .unwrap();

    if result.rows_affected() == 0 {
        StatusCode::BAD_REQUEST.into_response()
    } else {
        (
            StatusCode::OK,
            [(
                "HX-Trigger",
                if new_checked {
                    "reload-completed"
                } else {
                    "reload-incompleted"
                },
            )],
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct TaskInfo {
    completed: bool,
}

async fn get_tasks(
    State(db): State<PgPool>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Query(info): Query<TaskInfo>,
) -> Response {
    let name = if let Some(name) = cookies.get("TasksLoginName") {
        name
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let tasks: Vec<(String, bool, Uuid)> = sqlx::query_as(
        "SELECT name, completed, id FROM tasks WHERE username = $1 ORDER BY created",
    )
    .bind(name)
    .bind(info.completed)
    .fetch_all(&db)
    .await
    .unwrap();

    let mut context = Context::new();

    context.insert("complete", &info.completed);
    context.insert("tasks", &tasks);

    Html(TEMPLATES.render("partials/list.html", &context).unwrap()).into_response()
}

async fn delete_task(
    State(db): State<PgPool>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Form(check_info): Form<CheckedQuery>,
) -> Response {
    let name = if let Some(name) = cookies.get("TasksLoginName") {
        name
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let result = sqlx::query("DELETE FROM tasks WHERE username = $1 AND id = $2")
        .bind(name)
        .bind(check_info.id)
        .execute(&db)
        .await
        .unwrap();

    if result.rows_affected() == 0 {
        return StatusCode::NOT_MODIFIED.into_response();
    } else {
        (StatusCode::OK, [("HX-Trigger", "reload-completed")]).into_response()
    }
}
