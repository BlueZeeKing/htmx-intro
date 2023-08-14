use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use axum::{
    extract::State,
    headers::Cookie,
    http::{header::SET_COOKIE, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router, TypedHeader,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

type AppState = Arc<Mutex<HashMap<String, HashMap<String, bool>>>>;

lazy_static! {
    pub static ref TEMPLATES: Tera = Tera::new("templates/**/*.html").unwrap();
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(tasks))
        .route("/login", get(login))
        .route("/submit-name", post(submit))
        .route("/add-task", post(add_task))
        .route("/toggle", post(set_checked))
        .nest_service("/public", ServeDir::new("public"))
        .with_state(Arc::new(Mutex::new(HashMap::new())));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn login() -> Html<String> {
    Html(TEMPLATES.render("login.html", &Context::new()).unwrap())
}

async fn tasks(
    State(state): State<AppState>,
    TypedHeader(cookies): TypedHeader<Cookie>,
) -> Response {
    let state = state.lock().await;

    let tasks = if let Some(tasks) = cookies
        .get("TasksLoginName")
        .and_then(|name| state.get(name))
    {
        tasks
    } else {
        return Redirect::to("/login").into_response();
    };

    let mut context = Context::new();

    context.insert("tasks", tasks);

    Html(TEMPLATES.render("index.html", &context).unwrap()).into_response()
}

#[derive(Deserialize)]
struct NameQuery {
    name: String,
}

async fn submit(State(state): State<AppState>, Form(name): Form<NameQuery>) -> impl IntoResponse {
    let mut state = state.lock().await;

    state.entry(name.name.clone()).or_insert_with(HashMap::new);

    drop(state);

    (
        [(
            SET_COOKIE.as_str(),
            format!("TasksLoginName={};", name.name),
        )],
        Redirect::to("/"),
    )
}

#[derive(Serialize)]
struct Task<'a> {
    name: &'a str,
    completed: bool,
}

#[derive(Deserialize)]
struct AddTaskQuery {
    task: String,
}

async fn add_task(
    State(state): State<AppState>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Form(task_name): Form<AddTaskQuery>,
) -> Response {
    let mut state = state.lock().await;

    let tasks = if let Some(tasks) = cookies
        .get("TasksLoginName")
        .and_then(|name| state.get_mut(name))
    {
        tasks
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let entry = tasks.entry(task_name.task.clone());

    match entry {
        Entry::Occupied(_) => return StatusCode::CONFLICT.into_response(),
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(false);
        }
    }

    drop(state);

    Html(
        TEMPLATES
            .render(
                "partials/task.html",
                &Context::from_serialize(Task {
                    name: &task_name.task,
                    completed: false,
                })
                .unwrap(),
            )
            .unwrap(),
    )
    .into_response()
}

#[derive(Deserialize, Serialize)]
struct CheckedQuery {
    task: String,
    checked: bool,
}

async fn set_checked(
    State(state): State<AppState>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Form(check_info): Form<CheckedQuery>,
) -> Response {
    let mut state = state.lock().await;

    let tasks = if let Some(tasks) = cookies
        .get("TasksLoginName")
        .and_then(|name| state.get_mut(name))
    {
        tasks
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let new_checked = !check_info.checked;

    *tasks.get_mut(&check_info.task).unwrap() = new_checked;

    Html(
        TEMPLATES
            .render(
                "partials/task.html",
                &Context::from_serialize(Task {
                    name: &check_info.task,
                    completed: false,
                })
                .unwrap(),
            )
            .unwrap(),
    )
    .into_response()
}
