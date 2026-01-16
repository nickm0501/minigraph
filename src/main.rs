use axum::{routing::get, Router};
use tower_http::services::ServeDir;

mod rooms;
mod state;
mod types;
mod websocket;

#[tokio::main]
async fn main() {
    let (rooms_tx, rooms_rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(rooms::rooms_actor(rooms_rx));

    let state = state::AppState::new(rooms_tx);

    let app = Router::new()
        .route("/ws", get(websocket::websocket_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await.unwrap();

    println!("Server running on http://localhost:3030");

    axum::serve(listener, app).await.unwrap();
}
