use crate::{templates, Result, Task, User};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Form};
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, PgPool};

pub mod auth;

pub async fn login() -> impl IntoResponse {
    templates::Login {}
}

pub async fn tasks(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
) -> Result<impl IntoResponse> {
    let tasks: Vec<Task> =
        sqlx::query_as("SELECT * FROM tasks WHERE username = $1 ORDER BY created ASC")
            .bind(user.name)
            .fetch_all(&db)
            .await?;

    Ok(templates::Tasks { tasks })
}

#[derive(Deserialize)]
pub struct AddTaskQuery {
    task: String,
}

pub async fn add_task(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
    Form(task_name): Form<AddTaskQuery>,
) -> Result<impl IntoResponse> {
    let task: Task =
        sqlx::query_as("INSERT INTO tasks (name, username) VALUES ($1, $2) RETURNING *")
            .bind(task_name.task)
            .bind(user.name)
            .fetch_one(&db)
            .await?;

    Ok((
        [("HX-Trigger", "clear-task-form")],
        templates::Task { task },
    ))
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CheckedQuery {
    id: Uuid,
    completed: bool,
}

pub async fn set_checked(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
    Form(check_info): Form<CheckedQuery>,
) -> Result<impl IntoResponse> {
    let new_checked = !check_info.completed;

    sqlx::query("UPDATE tasks SET completed = $1 WHERE username = $2 AND id = $3")
        .bind(new_checked)
        .bind(&user.name)
        .bind(check_info.id)
        .execute(&db)
        .await?;

    let tasks: Vec<Task> = sqlx::query_as(
        "SELECT * FROM tasks WHERE username = $1 ORDER BY completed, created",
    )
    .bind(&user.name)
    .bind(new_checked)
    .fetch_all(&db)
    .await?;

    Ok(templates::List { tasks })
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
