use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use axum::{
    extract::State,
    headers::Cookie,
    http::{StatusCode, header::SET_COOKIE},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router, TypedHeader,
};
use leptos::{component, ssr::render_to_string, view, IntoAttribute, IntoView, Scope, For};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

type AppState = Arc<Mutex<HashMap<String, HashMap<String, bool>>>>;

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

#[component]
fn TextInput(cx: Scope, label: String, name: String) -> impl IntoView {
    view! {cx,
        <div>
            <label for=name.clone()>{label}</label>
            <input name=name.clone() id=name type="text" class="rounded bg-gray-200 m-2 px-4 p-2 outline-0 focus:outline-1 outline-offset-0 outline-gray-300" />
        </div>
    }
}

#[allow(unused_variables)]
#[component]
fn Button(cx: Scope, label: String) -> impl IntoView {
    view! {cx,
        <input type="submit" class="rounded bg-blue-500 text-white px-6 p-2 m-2 shadow hover:bg-blue-600 cursor-pointer">{label}</input>
    }
}

#[component]
fn Task(cx: Scope, name: String, completed: bool) -> impl IntoView {
    let data = serde_json::to_string(&CheckedQuery {
        task: name.clone(),
        checked: completed
    }).unwrap();

    view! { cx,
        <li class="flex">
            <input type="checkbox" class="mr-2" checked=completed hx-post="/toggle" hx-trigger="change" hx-vals=data hx-swap="outerHTML" hx-target="closest li" _="on change toggle @disabled until htmx:afterOnLoad" />
            <p>{name}</p>
        </li>
    }
}

async fn login() -> Html<String> {
    Html(render_to_string(|cx| {
        view! { cx,
            <html>
                <head>
                    <title>Test</title>
                    <script src="https://unpkg.com/htmx.org@1.9.4"></script>
                    <script src="https://unpkg.com/hyperscript.org@0.9.11"></script>
                    <link href="/public/out.css" rel="stylesheet" />
                    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                </head>

                <body>
                    <div class="p-24 bg-gray-50 absolute rounded shadow -translate-y-1/2 -translate-x-1/2 top-1/2 left-1/2 text-center">
                        <form action="/submit-name" method="POST">
                            <TextInput label="Your name:".to_owned() name="name".to_owned() />
                            <Button label="Submit".to_owned() />
                        </form>
                    </div>
                </body>
            </html>
        }
    }))
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
        tasks.to_owned()
    } else {
        return Redirect::to("/login").into_response();
    };

    Html(render_to_string(|cx| {
        view! { cx,
            <html>
                <head>
                    <title>Test</title>
                    <script src="https://unpkg.com/htmx.org@1.9.4"></script>
                    <script src="https://unpkg.com/hyperscript.org@0.9.11"></script>
                    <link href="/public/out.css" rel="stylesheet" />
                    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                </head>

                <body>
                    <div class="p-4">
                        <form hx-post="/add-task" hx-target="#list" hx-swap="beforeend" class="flex">
                            <TextInput label="Task name:".to_owned() name="task".to_owned() />
                            <Button label="Add".to_owned() />
                        </form>
                        <ul id="list">
                            <For
                                each=move || tasks.clone()
                                key=move |(key, _)| key.clone()
                                view=|cx, (key, val)| {
                                    view! { cx, 
                                        <Task name=key.to_owned() completed=val.to_owned() />
                                    }
                                }
                            />
                        </ul>
                    </div>
                </body>
            </html>
        }
    })).into_response()
}

#[derive(Deserialize)]
struct NameQuery {
    name: String,
}

async fn submit(State(state): State<AppState>, Form(name): Form<NameQuery>) -> impl IntoResponse {
    let mut state = state.lock().await;

    state.entry(name.name.clone()).or_insert_with(HashMap::new);

    drop(state);

    ([(SET_COOKIE.as_str(), format!("TasksLoginName={};", name.name))], Redirect::to("/"))
}

#[derive(Deserialize)]
struct AddTaskQuery {
    task: String,
}

async fn add_task(State(state): State<AppState>, TypedHeader(cookies): TypedHeader<Cookie>, Form(task_name): Form<AddTaskQuery>) -> Response {
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

    Html(render_to_string(|cx| {
        view! { cx,
            <Task name=task_name.task completed=false />
        }
    }))
    .into_response()
}

#[derive(Deserialize, Serialize)]
struct CheckedQuery {
    task: String,
    checked: bool
}

async fn set_checked(State(state): State<AppState>, TypedHeader(cookies): TypedHeader<Cookie>, Form(check_info): Form<CheckedQuery>) -> Response {
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

    Html(render_to_string(move |cx| {
        view! { cx,
            <Task name=check_info.task completed=new_checked />
        }
    })).into_response()
}
