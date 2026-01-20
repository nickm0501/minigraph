use axum::{routing::get, Router};
use tower_http::services::ServeDir;

pub mod metrics;
pub mod postgres;
pub mod types;
pub mod wal;

mod logging;
mod rooms;
mod state;
mod websocket;

pub async fn run_server() {
    let metrics = metrics::Metrics::new();

    let pg_config = postgres::PostgresConfig::from_env();
    if let Err(err) = postgres::setup_postgres(&pg_config).await {
        eprintln!("[PG][ERR] setup failed: {err}");
        std::process::exit(1);
    }

    let rooms = rooms::RoomsHandle::start(metrics.clone());
    let sink = std::sync::Arc::new(wal::RoomsInvalidationSink::new(rooms.clone()));
    let wal_reader = wal::WalReaderHandle::start(pg_config.clone(), sink, metrics.clone());

    tokio::spawn(metrics::run_resource_sampler(metrics.clone()));

    let state = state::AppState::new(rooms, wal_reader, metrics);

    let app = Router::new()
        .route("/ws", get(websocket::websocket_handler))
        .route("/debug/metrics", get(debug_metrics_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030")
        .await
        .expect("Failed to bind to 0.0.0.0:3030");

    println!("Server running on http://localhost:3030");

    axum::serve(listener, app)
        .await
        .expect("Server exited unexpectedly");
}

async fn debug_metrics_handler(
    axum::extract::State(state): axum::extract::State<state::AppState>,
) -> axum::Json<metrics::MetricsSnapshot> {
    axum::Json(state.metrics.snapshot())
}
