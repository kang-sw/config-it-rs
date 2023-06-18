use std::sync::atomic::AtomicUsize;

use axum::{response::IntoResponse, routing::get, Router, Server};

#[tokio::main]

async fn main() {
    let app = Router::new().route("/hello", get(hello));

    Server::bind(&"0.0.0.0:5944".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn hello() -> impl IntoResponse {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    format!("Hello, world! (count: {})", COUNT.load(std::sync::atomic::Ordering::Relaxed))
}
