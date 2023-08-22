use std::sync::Arc;

use crate::{Result, Task, User};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension, Form,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, PgPool};
use tera::{Context, Tera};

pub mod auth;

pub async fn login(State(templates): State<Arc<Tera>>) -> Result<Html<String>> {
    Ok(Html(templates.render("login.html", &Context::new())?))
}

pub async fn tasks(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
    State(templates): State<Arc<Tera>>,
) -> Result<Response> {
    let tasks: Vec<(String, bool, Uuid)> = sqlx::query_as(
        "SELECT name, completed, id FROM tasks WHERE username = $1 ORDER BY created ASC",
    )
    .bind(user.name)
    .fetch_all(&db)
    .await?;

    let mut context = Context::new();

    context.insert("tasks", &tasks);

    Ok(Html(templates.render("index.html", &context)?).into_response())
}

#[derive(Deserialize)]
pub struct AddTaskQuery {
    task: String,
}

pub async fn add_task(
    State(db): State<PgPool>,
    State(templates): State<Arc<Tera>>,
    Extension(user): Extension<User>,
    Form(task_name): Form<AddTaskQuery>,
) -> Result<impl IntoResponse> {
    let result: Task =
        sqlx::query_as("INSERT INTO tasks (name, username) VALUES ($1, $2) RETURNING *")
            .bind(task_name.task)
            .bind(user.name)
            .fetch_one(&db)
            .await?;

    Ok((
        [("HX-Trigger", "clear-task-form")],
        Html(templates.render("partials/task.html", &Context::from_serialize(result)?)?),
    ))
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CheckedQuery {
    id: Uuid,
    completed: bool,
}

pub async fn set_checked(
    State(db): State<PgPool>,
    State(templates): State<Arc<Tera>>,
    Extension(user): Extension<User>,
    Form(check_info): Form<CheckedQuery>,
) -> Result<Response> {
    let new_checked = !check_info.completed;

    sqlx::query("UPDATE tasks SET completed = $1 WHERE username = $2 AND id = $3")
        .bind(new_checked)
        .bind(&user.name)
        .bind(check_info.id)
        .execute(&db)
        .await?;

    let tasks: Vec<(String, bool, Uuid)> = sqlx::query_as(
        "SELECT name, completed, id FROM tasks WHERE username = $1 AND completed = $2 ORDER BY created",
    )
    .bind(&user.name)
    .bind(new_checked)
    .fetch_all(&db)
    .await?;

    let mut context = Context::new();

    context.insert("tasks", &tasks);

    Ok(Html(templates.render("partials/list.html", &context)?).into_response())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskInfo {
    completed: bool,
}

pub async fn delete_task(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
    Form(check_info): Form<CheckedQuery>,
) -> Result<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM tasks WHERE username = $1 AND id = $2")
        .bind(user.name)
        .bind(check_info.id)
        .execute(&db)
        .await?;

    Ok(if result.rows_affected() == 0 {
        StatusCode::NOT_MODIFIED
    } else {
        StatusCode::OK
    })
}
