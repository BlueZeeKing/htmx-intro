use std::net::SocketAddr;

use axum::{
    response::Html,
    routing::{get, post},
    Router,
};
use leptos::{ssr::render_to_string, view};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(index))
        .route("/clicked", post(clicked));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn index() -> Html<String> {
    Html(leptos::ssr::render_to_string(|cx| {
        view! { cx,
            <html>
                <head>
                    <title>Test</title>
                    <script src="https://unpkg.com/htmx.org@1.9.4"></script>
                </head>

                <body>
                    <button hx-post="/clicked" hx-swap="outerHTML">Click me!</button>
                </body>
            </html>
        }
    }))
}

async fn clicked() -> Html<String> {
    Html(render_to_string(|cx| {
        view! { cx,
            <p>Hello, World!</p>
        }
    }))
}
