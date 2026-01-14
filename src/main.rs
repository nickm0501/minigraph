use axum::{routing::get, Router};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/ws", get(|| async { axum::http::StatusCode::NOT_FOUND }))
        .nest_service("/", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030")
        .await
        .unwrap();

    println!("Server running on http://localhost:3030");

    axum::serve(listener, app).await.unwrap();
}
