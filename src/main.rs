use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
    time::Instant,
};

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
use tera::{Context, Tera};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, services::ServeDir};

type AppState = Arc<Mutex<HashMap<String, HashMap<String, (bool, Instant)>>>>;

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
            Ok(serde_json::Value::String(dbg!(serde_json::to_string(
                args
            )?)))
        },
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

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

    let mut sorted_tasks = tasks.iter().collect::<Vec<_>>();

    sorted_tasks.sort_by_key(|val| val.1 .1);

    let mut context = Context::new();

    context.insert(
        "tasks",
        &sorted_tasks
            .iter()
            .map(|(name, (done, _instant))| (name, done))
            .collect::<Vec<_>>(),
    );

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
            vacant_entry.insert((false, Instant::now()));
        }
    }

    drop(state);

    (
        StatusCode::CREATED,
        [("HX-Trigger", "reload-incompleted, clear-task-form")],
    )
        .into_response()
}

#[derive(Deserialize, Serialize, Debug)]
struct CheckedQuery {
    name: String,
    completed: bool,
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

    let new_checked = !check_info.completed;

    let Some(task) = tasks.get_mut(&check_info.name) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    task.0 = new_checked;

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

#[derive(Debug, Deserialize, Serialize)]
struct TaskInfo {
    completed: bool,
}

async fn get_tasks(
    State(state): State<AppState>,
    TypedHeader(cookies): TypedHeader<Cookie>,
    Query(info): Query<TaskInfo>,
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

    let mut sorted_tasks = tasks
        .iter()
        .filter(|(_name, (done, _instant))| if info.completed { *done } else { !*done })
        .collect::<Vec<_>>();

    sorted_tasks.sort_by_key(|val| val.1 .1);

    let data = sorted_tasks
        .iter()
        .map(|(name, (done, _time))| (name, *done))
        .filter(|(_name, done)| if info.completed { *done } else { !*done })
        .collect::<Vec<_>>();

    let mut context = Context::new();

    context.insert("complete", &info.completed);
    context.insert("tasks", &data);

    Html(TEMPLATES.render("partials/list.html", &context).unwrap()).into_response()
}

async fn delete_task(
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

    if tasks.remove(&check_info.name).is_none() {
        return StatusCode::NOT_MODIFIED.into_response();
    }

    (StatusCode::OK, [("HX-Trigger", "reload-completed")]).into_response()
}
