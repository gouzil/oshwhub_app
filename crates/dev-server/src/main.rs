use axum::{
    Router,
    body::Bytes,
    extract::rejection::BytesRejection,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use oshwhub_core::client::HttpClient;
use tower_http::{cors::CorsLayer, services::ServeDir};

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
    let mut router = app();
    let mut args = std::env::args_os().skip(1);
    if let Some(flag) = args.next() {
        if flag != "--web-root" {
            return Err("expected --web-root DIRECTORY".into());
        }
        let directory = args.next().ok_or("missing Web package directory")?;
        if args.next().is_some() {
            return Err("unexpected argument".into());
        }
        router = router.fallback_service(ServeDir::new(directory));
    }
    axum::serve(listener, router)
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
        .route("/fetch", post(fetch))
        .with_state(HttpClient::new().expect("initialize public HTTP client"))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(cors)
}

async fn fetch(
    axum::extract::State(client): axum::extract::State<HttpClient>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> axum::response::Response {
    use oshwhub_core::{client::PublicRequest, error::AppError};
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let local = host.is_some_and(|host| {
        host.parse::<std::net::SocketAddr>()
            .is_ok_and(|address| address.ip().is_loopback())
            || host
                .strip_prefix("localhost:")
                .is_some_and(|port| port.parse::<u16>().is_ok())
    });
    if !local
        || origin.is_some_and(|origin| host.is_none_or(|host| origin != format!("http://{host}")))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = async {
        let body = body.map_err(|_| AppError::new("INVALID_REQUEST", "无法读取下载请求"))?;
        let request: PublicRequest = serde_json::from_slice(&body)
            .map_err(|_| AppError::new("INVALID_REQUEST", "下载请求格式无效"))?;
        let json = request.json;
        Ok::<_, AppError>((client.relay(request).await?, json))
    }
    .await;
    match result {
        Ok((bytes, json)) => (
            [(
                axum::http::header::CONTENT_TYPE,
                if json {
                    "application/json"
                } else {
                    "application/octet-stream"
                },
            )],
            bytes,
        )
            .into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, axum::Json(error)).into_response(),
    }
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
    async fn local_fetch_preserves_errors_and_refuses_foreign_origins() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/fetch");
        let server = tokio::spawn(async move { axum::serve(listener, app()).await.unwrap() });
        let client = reqwest::Client::new();
        let request =
            serde_json::json!({"url":"http://127.0.0.1/private", "maxBytes":32, "json":false});
        let response = client
            .post(&url)
            .header("Origin", format!("http://{address}"))
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error["code"], "INVALID_URL");
        let rejected = client
            .post(&url)
            .header("Origin", "https://example.org")
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        let rebound = client
            .post(&url)
            .header("Host", "example.org")
            .header("Origin", "http://example.org")
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(rebound.status(), StatusCode::FORBIDDEN);
        server.abort();
    }

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
