//! Shared application business logic. Platform adapters only transport JSON.
pub mod api;
mod bom;
pub mod client;
pub mod diagnostics;
pub mod error;
pub mod model;
pub mod pcb;

use api::Api;
use client::HttpClient;
use error::{AppError, Result};
use model::FeedQuery;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::OnceLock, time::Instant};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum Command {
    Health,
    Feed {
        #[serde(default = "first_page")]
        page: u32,
        #[serde(default = "page_size", rename = "pageSize")]
        page_size: u32,
        #[serde(default)]
        query: String,
        #[serde(default = "default_sort")]
        sort: String,
    },
    Project {
        id: String,
    },
    Login {
        code: String,
    },
    CurrentUser,
    Logout,
    ProjectStatus {
        id: String,
    },
    ToggleProjectCollection {
        id: String,
        collect: bool,
    },
    Comments {
        id: String,
        #[serde(default = "default_comment_sort")]
        sort: String,
        #[serde(default = "first_page")]
        page: u32,
        #[serde(default = "comment_page_size", rename = "pageSize")]
        page_size: u32,
    },
    CreateComment {
        id: String,
        content: String,
    },
    SearchBom {
        items: Vec<model::BomItem>,
        query: String,
    },
    Pcb {
        #[serde(rename = "projectId")]
        project_id: String,
        #[serde(rename = "documentId")]
        document_id: String,
    },
    Demo,
    Pcb3d {
        #[serde(rename = "sceneId")]
        scene_id: String,
    },
    Model3d {
        #[serde(rename = "sceneId")]
        scene_id: String,
        key: String,
    },
    Pick {
        #[serde(rename = "sceneId")]
        scene_id: String,
        x: f64,
        y: f64,
        tolerance: f64,
        #[serde(default)]
        layers: Option<Vec<i32>>,
    },
}

fn first_page() -> u32 {
    1
}
fn page_size() -> u32 {
    20
}
fn default_sort() -> String {
    "recommend".into()
}
fn default_comment_sort() -> String {
    "hot".into()
}
fn comment_page_size() -> u32 {
    20
}

fn http_client() -> Result<HttpClient> {
    static CLIENT: OnceLock<std::result::Result<HttpClient, AppError>> = OnceLock::new();
    match CLIENT.get_or_init(HttpClient::new) {
        Ok(client) => Ok(client.clone()),
        Err(error) => Err(AppError::new(&error.code, error.message.clone())),
    }
}

async fn execute(command: Command) -> Result<Value> {
    match command {
        Command::Health => {
            Ok(json!({"version": env!("CARGO_PKG_VERSION"), "protocol": 1, "engine": "smt-view"}))
        }
        Command::Feed {
            page,
            page_size,
            query,
            sort,
        } => {
            let data = Api::new(http_client()?)
                .feed(FeedQuery {
                    page,
                    page_size,
                    query,
                    sort,
                })
                .await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::Project { id } => {
            let data = Api::new(http_client()?).project(&id).await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::Login { code } => {
            let data = Api::new(http_client()?).login(&code).await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::CurrentUser => {
            let data = Api::new(http_client()?).current_user().await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::Logout => {
            Api::new(http_client()?).logout().await?;
            Ok(json!({}))
        }
        Command::ProjectStatus { id } => {
            let data = Api::new(http_client()?).project_status(&id).await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::ToggleProjectCollection { id, collect } => {
            let data = Api::new(http_client()?)
                .set_project_collection(&id, collect)
                .await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::Comments {
            id,
            sort,
            page,
            page_size,
        } => {
            let data = Api::new(http_client()?)
                .comments(&id, &sort, page, page_size)
                .await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::CreateComment { id, content } => {
            let data = Api::new(http_client()?)
                .create_comment(&id, &content)
                .await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::SearchBom { items, query } => {
            serde_json::to_value(bom::search(&items, &query)?).map_err(serialization_error)
        }
        Command::Pcb {
            project_id,
            document_id,
        } => {
            api::validate_id(&project_id)?;
            api::validate_id(&document_id)?;
            let data = pcb::load(&http_client()?, &project_id, &document_id).await?;
            serde_json::to_value(data).map_err(serialization_error)
        }
        Command::Demo => serde_json::to_value(pcb::demo()).map_err(serialization_error),
        Command::Pcb3d { scene_id } => {
            serde_json::to_value(pcb::prepare_3d(scene_id).await?).map_err(serialization_error)
        }
        Command::Model3d { scene_id, key } => {
            serde_json::to_value(pcb::model_3d(&http_client()?, &scene_id, &key).await?)
                .map_err(serialization_error)
        }
        Command::Pick {
            scene_id,
            x,
            y,
            tolerance,
            layers,
        } => {
            if !x.is_finite()
                || !y.is_finite()
                || !tolerance.is_finite()
                || tolerance <= 0.0
                || tolerance > 1e6
            {
                return Err(AppError::new("invalid_request", "拾取坐标或范围无效"));
            }
            serde_json::to_value(pcb::pick(&scene_id, x, y, tolerance, layers.as_deref())?)
                .map_err(serialization_error)
        }
    }
}

fn serialization_error(_: serde_json::Error) -> AppError {
    AppError::new("internal", "无法编码核心响应")
}

/// Every native/web/desktop adapter uses the same bounded protocol and errors.
pub async fn dispatch_json(input: &str) -> String {
    let request_id = uuid::Uuid::new_v4().to_string();
    let start = Instant::now();
    let result = if input.len() > 1024 * 1024 {
        Err(AppError::new("invalid_request", "请求超过 1 MiB 限制"))
    } else {
        match serde_json::from_str::<Command>(input) {
            Ok(command) => execute(command).await,
            Err(_) => Err(AppError::new(
                "invalid_request",
                "请求格式无效或命令不受支持",
            )),
        }
    };
    match result {
        Ok(data) => {
            tracing::info!(
                request_id,
                elapsed_ms = start.elapsed().as_millis() as u64,
                "core.request.complete"
            );
            json!({"ok": true, "data": data, "requestId": request_id}).to_string()
        }
        Err(error) => {
            tracing::warn!(
                request_id,
                code = error.code,
                elapsed_ms = start.elapsed().as_millis() as u64,
                "core.request.failed"
            );
            json!({"ok": false, "error": {"code": error.code, "message": error.message, "requestId": request_id}}).to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_works_without_network_and_reports_protocol() {
        let value: Value =
            serde_json::from_str(&dispatch_json(r#"{"command":"health"}"#).await).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["protocol"], 1);
        assert!(
            value["requestId"]
                .as_str()
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        );
    }

    #[tokio::test]
    async fn three_dimensional_commands_reject_expired_scenes_without_network() {
        for input in [
            r#"{"command":"pcb3d","sceneId":"expired"}"#,
            r#"{"command":"model3d","sceneId":"expired","key":"untrusted"}"#,
        ] {
            let value: Value = serde_json::from_str(&dispatch_json(input).await).unwrap();
            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "scene_expired");
            assert!(value["error"]["requestId"].is_string());
        }
    }

    #[tokio::test]
    async fn bom_search_matches_each_field_without_network_and_keeps_original_indices() {
        let items = json!([
            {"name":"电容 10nF", "designator":"C6, C16", "footprint":"C0402",
             "quantity":0, "partNumber":"CL05B103KB5NNNC", "manufacturer":"SAMSUNG(三星)",
             "supplier":"LCSC", "supplierPart":"C15195"},
            {"name":"电阻", "designator":"R1", "footprint":"0603", "quantity":7,
             "partNumber":"", "manufacturer":"", "supplier":"", "supplierPart":""}
        ]);
        for (query, expected) in [
            ("电容", json!([0])),
            (" c16 ", json!([0])),
            ("c0402", json!([0])),
            ("0", json!([0, 1])),
            ("7", json!([1])),
            ("cl05b103", json!([0])),
            ("samsung", json!([0])),
            ("lcsc", json!([0])),
            ("c15195", json!([0])),
            ("R1", json!([1])),
            ("  \u{3000}", json!([0, 1])),
            ("未提供", json!([])),
            ("does-not-exist", json!([])),
        ] {
            let input = json!({"command":"searchBom", "items":items, "query":query});
            let value: Value =
                serde_json::from_str(&dispatch_json(&input.to_string()).await).unwrap();
            assert_eq!(value["ok"], true, "query {query}: {value}");
            assert_eq!(value["data"], expected, "query {query}");
        }
        for query in ["x".repeat(101), "C6\n".into()] {
            let input = json!({"command":"searchBom", "items":items, "query":query});
            let value: Value =
                serde_json::from_str(&dispatch_json(&input.to_string()).await).unwrap();
            assert_eq!(value["error"]["code"], "INVALID_QUERY");
        }
        let input = json!({"command":"searchBom", "items":[], "query":"C6"});
        let value: Value = serde_json::from_str(&dispatch_json(&input.to_string()).await).unwrap();
        assert_eq!(value["data"], json!([]));
    }

    #[tokio::test]
    async fn malformed_and_oversized_requests_share_error_protocol() {
        for input in [
            "not json".to_owned(),
            r#"{"command":"write"}"#.to_owned(),
            " ".repeat(1024 * 1024 + 1),
        ] {
            let value: Value = serde_json::from_str(&dispatch_json(&input).await).unwrap();
            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "invalid_request");
            assert!(value["error"]["requestId"].is_string());
            assert!(!value.to_string().contains(&input));
        }
    }
}
