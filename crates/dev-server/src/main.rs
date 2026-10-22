use axum::{
    Router,
    body::Bytes,
    http::{HeaderValue, Method},
    response::IntoResponse,
    routing::{get, post},
};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
        ])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([axum::http::header::CONTENT_TYPE]);
    let app = Router::new()
        .route("/health", get(|| async { "oshwhub-core protocol=1" }))
        .route(
            "/bridge",
            post(|body: Bytes| async move {
                let input = std::str::from_utf8(&body).unwrap_or("");
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    oshwhub_core::dispatch_json(input).await,
                )
                    .into_response()
            }),
        )
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(cors);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4318").await?;
    tracing::info!(address = "127.0.0.1:4318", "dev_server.ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
