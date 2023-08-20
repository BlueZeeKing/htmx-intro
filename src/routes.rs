use std::sync::Arc;

use crate::{Result, User};
use axum::{
    extract::{Query, State},
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
    Extension(user): Extension<User>,
    Form(task_name): Form<AddTaskQuery>,
) -> Result<Response> {
    let result = sqlx::query("INSERT INTO tasks (name, username) VALUES ($1, $2)")
        .bind(task_name.task)
        .bind(user.name)
        .execute(&db)
        .await?;

    Ok(if result.rows_affected() == 0 {
        StatusCode::CONFLICT.into_response()
    } else {
        (
            StatusCode::CREATED,
            [("HX-Trigger", "reload-incompleted, clear-task-form")],
        )
            .into_response()
    })
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
) -> Result<Response> {
    let new_checked = !check_info.completed;

    let result = sqlx::query("UPDATE tasks SET completed = $1 WHERE username = $2 AND id = $3")
        .bind(new_checked)
        .bind(user.name)
        .bind(check_info.id)
        .execute(&db)
        .await?;

    Ok(if result.rows_affected() == 0 {
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
    })
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskInfo {
    completed: bool,
}

pub async fn get_tasks(
    State(db): State<PgPool>,
    State(templates): State<Arc<Tera>>,
    Extension(user): Extension<User>,
    Query(info): Query<TaskInfo>,
) -> Result<Response> {
    let tasks: Vec<(String, bool, Uuid)> = sqlx::query_as(
        "SELECT name, completed, id FROM tasks WHERE username = $1 ORDER BY created",
    )
    .bind(user.name)
    .bind(info.completed)
    .fetch_all(&db)
    .await?;

    let mut context = Context::new();

    context.insert("complete", &info.completed);
    context.insert("tasks", &tasks);

    Ok(Html(templates.render("partials/list.html", &context)?).into_response())
}

pub async fn delete_task(
    State(db): State<PgPool>,
    Extension(user): Extension<User>,
    Form(check_info): Form<CheckedQuery>,
) -> Result<Response> {
    let result = sqlx::query("DELETE FROM tasks WHERE username = $1 AND id = $2")
        .bind(user.name)
        .bind(check_info.id)
        .execute(&db)
        .await?;

    Ok(if result.rows_affected() == 0 {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (StatusCode::OK, [("HX-Trigger", "reload-completed")]).into_response()
    })
}
