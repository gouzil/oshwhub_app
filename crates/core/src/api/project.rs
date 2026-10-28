use super::{
    Api,
    content::content_blocks,
    response::{invalid_response, optional_array, payload, required, web_url},
    validate_id,
};
use crate::{
    error::{AppError, Result},
    model::{Attachment, Board, BomItem, ProjectDetail, ProjectSummary},
};
use serde_json::Value;
use std::collections::HashSet;
use url::Url;

impl Api {
    pub async fn project(&self, id: &str) -> Result<ProjectDetail> {
        validate_id(id)?;
        let url = format!("https://oshwhub.com/api/project/{id}");
        let detail = self.client.get_json(&url).await?;
        let payload = payload(&detail)?;
        let previews = if payload["origin"].as_str() == Some("pro") {
            Some(self.client.get_json(&format!("{url}/proImages")).await?)
        } else {
            None
        };
        normalize_project(&detail, previews.as_ref())
    }
}

pub fn normalize_project(detail: &Value, previews: Option<&Value>) -> Result<ProjectDetail> {
    let data = payload(detail)?;
    let summary = summary(data)?;
    let markdown = data["text_editor_type"]
        .as_str()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("markdown"));
    let content = content_blocks(data["content"].as_str().unwrap_or_default(), markdown)?;
    let mut boards = Vec::new();
    let mut seen = HashSet::new();
    if let Some(previews) = previews {
        let data = payload(previews)?;
        for (field, standalone) in [("boards", false), ("out_pcbs", true)] {
            for row in optional_array(data, field)? {
                let board = board(row, standalone)?;
                if seen.insert(board.id.clone()) {
                    boards.push(board);
                }
            }
        }
    } else if data["origin"].as_str() == Some("pro") {
        return Err(AppError::new(
            "INVALID_RESPONSE",
            "专业版工程缺少板子预览信息",
        ));
    }
    let mut attachments = Vec::new();
    let mut seen = HashSet::new();
    for field in ["attachments", "attachments_3d"] {
        for row in optional_array(data, field)? {
            if let Some(url) = row["src"]
                .as_str()
                .and_then(|s| web_url(s, "https://image.lceda.cn"))
                && seen.insert(url.clone())
            {
                attachments.push(Attachment {
                    name: row["name"].as_str().unwrap_or("附件").to_owned(),
                    url,
                });
            }
        }
    }
    Ok(ProjectDetail {
        summary,
        content,
        boards,
        attachments,
    })
}

pub(super) fn summary(data: &Value) -> Result<ProjectSummary> {
    let id = required(data, "uuid")?;
    validate_id(id).map_err(|_| invalid_response())?;
    let path = required(data, "path")?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 2
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'))
    {
        return Err(invalid_response());
    }
    let mut url = Url::parse("https://oshwhub.com").map_err(|_| invalid_response())?;
    url.path_segments_mut()
        .map_err(|_| invalid_response())?
        .extend(parts);
    let owner = &data["owner"];
    let tags = data["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| {
            tag.as_str()
                .or_else(|| tag["name"].as_str())
                .map(str::to_owned)
        })
        .collect();
    Ok(ProjectSummary {
        id: id.to_owned(),
        name: required(data, "name")?.to_owned(),
        path: path.to_owned(),
        description: data["introduction"].as_str().unwrap_or_default().to_owned(),
        cover: data["thumb"]
            .as_str()
            .and_then(|s| web_url(s, "https://oshwhub.com")),
        author: owner["nickname"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| owner["username"].as_str())
            .unwrap_or("未知作者")
            .to_owned(),
        author_avatar: owner["avatar"].as_str().and_then(avatar_url),
        likes: data["count"]["like"].as_u64().unwrap_or_default(),
        views: data["count"]["views"].as_u64().unwrap_or_default(),
        tags,
        updated_at: data["updated_at"].as_str().map(str::to_owned),
        url: url.into(),
    })
}

fn avatar_url(value: &str) -> Option<String> {
    let url = Url::parse(&web_url(value, "https://oshwhub.com")?).ok()?;
    if url.host_str() == Some("oshwhub.com")
        && matches!(
            url.path(),
            "/images/avatar-default.png" | "/images/team-avatar-default.png"
        )
    {
        return None;
    }
    Some(url.into())
}

fn board(data: &Value, standalone: bool) -> Result<Board> {
    let id = required(data, "uuid")?;
    validate_id(id).map_err(|_| invalid_response())?;
    let pcb_id = if standalone {
        Some(id)
    } else {
        data["pcb"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| data["pcb_info"]["uuid"].as_str().filter(|s| !s.is_empty()))
    };
    if let Some(id) = pcb_id {
        validate_id(id).map_err(|_| invalid_response())?;
    }
    let direct_schematic = match data.get("sch") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value.is_empty() => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => return Err(invalid_response()),
    };
    let nested_schematic = match data.pointer("/schematic/uuid") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value.is_empty() => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => return Err(invalid_response()),
    };
    if direct_schematic.is_some()
        && nested_schematic.is_some()
        && direct_schematic != nested_schematic
    {
        return Err(invalid_response());
    }
    let schematic_id = direct_schematic.or(nested_schematic);
    if let Some(id) = schematic_id {
        validate_id(id).map_err(|_| invalid_response())?;
    }
    // shortcut: open the first published page until the viewer supports page switching.
    let schematic_id = if let Some(page) = optional_array(&data["schematic"], "documents")?.first()
    {
        let id = required(page, "uuid")?;
        validate_id(id).map_err(|_| invalid_response())?;
        Some(id)
    } else {
        schematic_id
    };
    let thumbnail = if standalone {
        &data["thumb"]
    } else {
        &data["pcb_info"]["thumb"]
    };
    let raw_bom = data
        .pointer("/pcb_info/boms/boms")
        .or_else(|| data.pointer("/schematic/boms/boms"))
        .or_else(|| data.pointer("/boms/boms"));
    Ok(Board {
        id: id.to_owned(),
        name: data["name"]
            .as_str()
            .or_else(|| data["display_title"].as_str())
            .unwrap_or("未命名板子")
            .to_owned(),
        pcb_id: pcb_id.map(str::to_owned),
        schematic_id: schematic_id.map(str::to_owned),
        thumbnail: thumbnail
            .as_str()
            .and_then(|s| web_url(s, "https://oshwhub.com")),
        bom: parse_bom(raw_bom)?,
    })
}

fn parse_bom(raw: Option<&Value>) -> Result<Vec<BomItem>> {
    let invalid = || AppError::new("INVALID_BOM", "物料清单损坏，无法完整解析");
    let parsed;
    let raw = match raw {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::String(s)) if s.trim().is_empty() => return Ok(Vec::new()),
        Some(Value::String(s)) => {
            parsed = serde_json::from_str::<Value>(s).map_err(|_| invalid())?;
            &parsed
        }
        Some(value) => value,
    };
    raw.as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|row| {
            if !row.is_object() {
                return Err(invalid());
            }
            let quantity = row["Quantity"]
                .as_u64()
                .or_else(|| row["Quantity"].as_str().and_then(|s| s.trim().parse().ok()))
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(invalid)?;
            Ok(BomItem {
                name: row["Name"].as_str().unwrap_or_default().to_owned(),
                designator: row["Designator"].as_str().unwrap_or_default().to_owned(),
                footprint: row["Footprint"].as_str().unwrap_or_default().to_owned(),
                quantity,
                part_number: row["Manufacturer Part"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                manufacturer: row["Manufacturer"].as_str().unwrap_or_default().to_owned(),
                supplier: row["Supplier"].as_str().unwrap_or_default().to_owned(),
                supplier_part: row["Supplier Part"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}
