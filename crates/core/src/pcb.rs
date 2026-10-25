//! Read-only EasyEDA Pro state and independent 2D/3D scene compilers.
mod geometry;
mod history;
mod models;
mod scene;
mod solid;

use crate::{
    client::HttpClient,
    error::{AppError, Result},
};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

// History pages include opaque per-version metadata; keep their larger budget scoped to PCB loads.
const HISTORY_JSON_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneLayer {
    pub id: i32,
    pub name: String,
    pub color: String,
    pub visible: bool,
    pub order: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawBatch {
    pub layer: i32,
    pub path: String,
    pub fill: bool,
    pub width: f64,
    pub bounds: Bounds,
    pub color: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLabel {
    pub layer: i32,
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub size: f64,
    pub rotation: f64,
    pub align: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub load_ms: u64,
    pub parse_ms: u64,
    pub scene_ms: u64,
    pub record_count: usize,
    pub history_count: usize,
    pub download_bytes: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub revision: String,
    pub bounds: Bounds,
    pub layers: Vec<SceneLayer>,
    pub batches: Vec<DrawBatch>,
    pub texts: Vec<TextLabel>,
    pub object_count: usize,
    pub warnings: Vec<String>,
    pub metrics: Metrics,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickResult {
    pub id: String,
    pub kind: String,
    pub layer: i32,
    pub net: Option<String>,
    pub properties: BTreeMap<String, String>,
    pub bounds: Bounds,
}

fn result(value: &Value) -> Result<&Value> {
    if value["success"] == true && value.get("result").is_some() {
        Ok(&value["result"])
    } else {
        Err(AppError::new(
            "pcb_unavailable",
            "无法读取此公开工程的 PCB 历史",
        ))
    }
}

pub async fn load(client: &HttpClient, project_id: &str, document_id: &str) -> Result<Scene> {
    crate::api::validate_id(project_id)?;
    crate::api::validate_id(document_id)?;
    let start = Instant::now();
    let metadata = client
        .get_json(&format!(
            "https://pro.lceda.cn/api/v4/projects/{project_id}"
        ))
        .await?;
    let metadata = result(&metadata)?;
    let branch = metadata["branch_uuid"]
        .as_str()
        .ok_or_else(|| AppError::new("pcb_format", "工程没有默认分支"))?;
    crate::api::validate_id(branch)?;
    let path = metadata["path"]
        .as_str()
        .ok_or_else(|| AppError::new("pcb_format", "工程路径缺失"))?;
    let branch_data = client
        .get_json(&format!(
            "https://pro.lceda.cn/api/v4/projects/{project_id}/branches/{branch}"
        ))
        .await?;
    let head = result(&branch_data)?["history_uuid"]
        .as_str()
        .ok_or_else(|| AppError::new("pcb_format", "分支历史缺失"))?
        .to_owned();
    let mut next = Some(head.clone());
    let mut seen = HashSet::new();
    let mut histories = Vec::new();
    while let Some(id) = next.take() {
        let mut url = url::Url::parse("https://pro.lceda.cn/api/v4/projects/branches/histories")
            .expect("constant URL");
        url.query_pairs_mut().extend_pairs([
            ("path", path),
            ("project", project_id),
            ("branch", branch),
            ("history", id.as_str()),
            ("limit", "100"),
        ]);
        let page = client
            .get_json_with_limit(url.as_str(), HISTORY_JSON_LIMIT)
            .await?;
        let page: Vec<history::History> = serde_json::from_value(result(&page)?.clone())
            .map_err(|_| AppError::new("pcb_format", "历史索引无效"))?;
        next = history::extend_chain(&mut histories, &mut seen, page, id)?;
    }
    let history_count = histories.len();
    let mut plan = Vec::new();
    for h in histories.into_iter().rev() {
        for (url, iv) in history::parts(&h)? {
            plan.push((url, h.key.clone(), iv));
        }
        if plan.len() > 1024 {
            return Err(AppError::new("pcb_limit", "工程历史分块过多"));
        }
    }
    let downloaded_bytes = Arc::new(AtomicUsize::new(0));
    let payloads: Vec<_> = stream::iter(plan.into_iter().map(|(url, key, iv)| {
        let total = Arc::clone(&downloaded_bytes);
        async move {
            let bytes = client.get_bytes(&url, 16 * 1024 * 1024).await?;
            let before = total.fetch_add(bytes.len(), Ordering::Relaxed);
            if before.saturating_add(bytes.len()) > 128 * 1024 * 1024 {
                return Err(AppError::new("pcb_limit", "工程下载超过 128 MiB 限制"));
            }
            Ok::<_, AppError>((key, iv, bytes))
        }
    }))
    .buffered(4)
    .try_collect()
    .await?;
    let download_bytes = downloaded_bytes.load(Ordering::Relaxed);
    let document_id = document_id.to_owned();
    let mut scene = tokio::task::spawn_blocking(move || {
        let parse_start = Instant::now();
        let mut project = history::Project::default();
        let mut decoded = 0usize;
        for (key, iv, bytes) in payloads {
            let text = history::decode(
                &key,
                &iv,
                &bytes,
                history::MAX_DECODED.saturating_sub(decoded),
            )?;
            decoded = decoded.saturating_add(text.len());
            history::replay(&mut project, &text)?;
        }
        let metrics = Metrics {
            parse_ms: parse_start.elapsed().as_millis() as u64,
            record_count: project.record_count,
            history_count,
            download_bytes,
            ..Metrics::default()
        };
        scene::compile(project, &document_id, head, metrics)
    })
    .await
    .map_err(|_| AppError::new("internal", "PCB 场景处理失败"))??;
    scene.metrics.load_ms = start.elapsed().as_millis() as u64;
    tracing::info!(
        histories = history_count,
        download_bytes,
        records = scene.metrics.record_count,
        objects = scene.object_count,
        batches = scene.batches.len(),
        load_ms = scene.metrics.load_ms,
        parse_ms = scene.metrics.parse_ms,
        scene_ms = scene.metrics.scene_ms,
        "pcb.scene.complete"
    );
    Ok(scene)
}
pub fn demo() -> Scene {
    scene::demo()
}
pub async fn prepare_3d(scene_id: String) -> Result<solid::Scene3d> {
    tokio::task::spawn_blocking(move || scene::prepare_3d(&scene_id))
        .await
        .map_err(|_| AppError::new("internal", "三维板体处理失败"))?
}
pub async fn model_3d(client: &HttpClient, scene_id: &str, key: &str) -> Result<models::Model3d> {
    let (bindings, thickness) = scene::model_bindings(scene_id)?;
    models::load(client, key, bindings, thickness).await
}
pub fn pick(
    scene_id: &str,
    x: f64,
    y: f64,
    tolerance: f64,
    layers: Option<&[i32]>,
) -> Result<Option<PickResult>> {
    scene::pick(scene_id, x, y, tolerance, layers)
}
