use axum::{
    Router,
    body::Bytes,
    extract::rejection::BytesRejection,
    http::{HeaderValue, Method, StatusCode},
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4318").await?;
    tracing::info!(address = "127.0.0.1:4318", "dev_server.ready");
    axum::serve(listener, app())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

fn app() -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
        ])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([axum::http::header::CONTENT_TYPE]);
    Router::new()
        .route("/health", get(|| async { "oshwhub-core protocol=1" }))
        .route("/bridge", post(bridge))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(cors)
}

async fn bridge(body: Result<Bytes, BytesRejection>) -> impl IntoResponse {
    let response = match body {
        Ok(body) => {
            let input = std::str::from_utf8(&body).unwrap_or("");
            oshwhub_core::dispatch_json(input).await
        }
        Err(rejection) => {
            let request_id = uuid::Uuid::new_v4().to_string();
            let message = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                "请求超过 1 MiB 限制"
            } else {
                "无法读取请求正文"
            };
            tracing::warn!(
                request_id,
                code = "invalid_request",
                "dev_server.request.rejected"
            );
            serde_json::json!({
                "ok": false,
                "error": {"code": "invalid_request", "message": message, "requestId": request_id}
            })
            .to_string()
        }
    };
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        response,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn small_and_oversized_http_requests_share_the_error_protocol() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/bridge", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app()).await.unwrap() });
        let client = reqwest::Client::new();
        let mut request_ids = Vec::new();
        for body in [
            r#"{"command":"unknown"}"#.to_owned(),
            " ".repeat(1024 * 1024 + 1),
        ] {
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(response.headers()["Content-Type"], "application/json");
            let envelope: serde_json::Value = response.json().await.unwrap();
            assert_eq!(envelope["ok"], false);
            assert_eq!(envelope["error"]["code"], "invalid_request");
            let request_id = envelope["error"]["requestId"].as_str().unwrap();
            uuid::Uuid::parse_str(request_id).unwrap();
            request_ids.push(request_id.to_owned());
        }
        assert_ne!(request_ids[0], request_ids[1]);

        let mut accepted_body = r#"{"command":"health"}"#.to_owned();
        accepted_body.push_str(&" ".repeat(1024 * 1024 - accepted_body.len()));
        let response: serde_json::Value = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(accepted_body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["protocol"], 1);
        server.abort();
    }
}
