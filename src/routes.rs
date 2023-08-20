use std::sync::Arc;

use axum::{
    extract::{Query, State},
    headers::Cookie,
    http::{header::SET_COOKIE, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form, TypedHeader,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, PgPool};
use tera::{Context, Tera};

pub async fn login(State(templates): State<Arc<Tera>>) -> Html<String> {
    Html(templates.render("login.html", &Context::new()).unwrap())
}

pub async fn tasks(
    State(db): State<PgPool>,
    State(templates): State<Arc<Tera>>,
    TypedHeader(cookies): TypedHeader<Cookie>,
) -> Response {
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

    Html(templates.render("index.html", &context).unwrap()).into_response()
}

#[derive(Deserialize)]
pub struct NameQuery {
    name: String,
}

pub async fn submit(State(db): State<PgPool>, Form(name): Form<NameQuery>) -> impl IntoResponse {
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

#[derive(Deserialize)]
pub struct AddTaskQuery {
    task: String,
}

pub async fn add_task(
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
pub struct CheckedQuery {
    id: Uuid,
    completed: bool,
}

pub async fn set_checked(
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
pub struct TaskInfo {
    completed: bool,
}

pub async fn get_tasks(
    State(db): State<PgPool>,
    State(templates): State<Arc<Tera>>,
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

    Html(templates.render("partials/list.html", &context).unwrap()).into_response()
}

pub async fn delete_task(
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
