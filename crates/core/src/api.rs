use crate::{
    client::HttpClient,
    error::{AppError, Result},
    model::{
        Attachment, Board, BomItem, ContentBlock, FeedPage, FeedQuery, ProjectDetail,
        ProjectSummary,
    },
};
use scraper::ElementRef;
use serde_json::{Value, json};
use std::collections::HashSet;
use url::Url;

pub struct Api {
    client: HttpClient,
}

impl Api {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn feed(&self, query: FeedQuery) -> Result<FeedPage> {
        let (url, body) = feed_request(&query)?;
        let response = if let Some(body) = body {
            self.client.post_json(&url, &body).await?
        } else {
            self.client.get_json(&url).await?
        };
        normalize_feed(&response)
    }

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

pub fn validate_id(id: &str) -> Result<()> {
    if !matches!(id.len(), 16 | 32) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::new("INVALID_ID", "工程或文档 ID 格式无效"));
    }
    Ok(())
}

pub fn feed_request(query: &FeedQuery) -> Result<(String, Option<Value>)> {
    let keywords = query.query.trim();
    let sort = if query.sort.is_empty() {
        "recommend"
    } else {
        query.sort.as_str()
    };
    if !(1..=10_000).contains(&query.page)
        || !(1..=50).contains(&query.page_size)
        || keywords.chars().count() > 100
        || query.query.chars().any(char::is_control)
        || !matches!(sort, "recommend" | "new" | "like")
    {
        return Err(AppError::new(
            "INVALID_QUERY",
            "页码、每页数量、关键词或排序无效",
        ));
    }
    if !keywords.is_empty() {
        return Ok((
            "https://oshwhub.com/api/search/data".to_owned(),
            Some(json!({
                "type": "project", "keywords": keywords, "category": "",
                "sort": if sort == "recommend" { "" } else { sort },
                "page": query.page, "pageSize": query.page_size
            })),
        ));
    }
    let mut url = Url::parse("https://oshwhub.com/api/project").map_err(|_| invalid_response())?;
    url.query_pairs_mut()
        .append_pair("page", &query.page.to_string())
        .append_pair("pageSize", &query.page_size.to_string())
        .append_pair("type", sort)
        .append_pair("origin", "all")
        .append_pair("projectTag", "")
        .append_pair("keywords", "")
        .append_pair("public", "true");
    Ok((url.into(), None))
}

pub fn normalize_feed(response: &Value) -> Result<FeedPage> {
    let data = payload(response)?;
    let page = number(data, "page")?;
    let page_size = number(data, "pageSize")?;
    let total_page = number(data, "totalPage")?;
    let total = data["total"].as_u64().ok_or_else(invalid_response)?;
    if page == 0 || page_size == 0 || (total > 0 && total_page == 0) {
        return Err(invalid_response());
    }
    let rows = data["lists"].as_array().ok_or_else(invalid_response)?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for row in rows {
        let item = summary(row)?;
        if seen.insert(item.id.clone()) {
            items.push(item);
        }
    }
    Ok(FeedPage {
        items,
        page,
        total,
        has_more: page < total_page,
    })
}

pub fn normalize_project(detail: &Value, previews: Option<&Value>) -> Result<ProjectDetail> {
    let data = payload(detail)?;
    let summary = summary(data)?;
    let content = content_blocks(data["content"].as_str().unwrap_or_default())?;
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

fn invalid_response() -> AppError {
    AppError::new("INVALID_RESPONSE", "站点数据格式不完整或已变化")
}

fn payload(response: &Value) -> Result<&Value> {
    let code = response["code"].as_u64().ok_or_else(invalid_response)?;
    if code != 0 {
        let (kind, message) = match code {
            401 => ("AUTH_REQUIRED", "该内容需要登录后访问"),
            403 => ("BLOCKED", "站点暂时拒绝了请求"),
            429 | 100001 => ("RATE_LIMITED", "请求过于频繁，请稍后重试"),
            404 | 104001 | 104002 => ("NOT_FOUND", "工程不存在或未公开"),
            104006 => ("UNSUPPORTED_PROJECT", "该工程不支持专业版预览接口"),
            104018 => ("PREVIEW_UNAVAILABLE", "站点无法提供工程预览信息"),
            _ => {
                return Err(AppError::new(
                    "API_ERROR",
                    format!("站点返回错误（代码 {code}）"),
                ));
            }
        };
        return Err(AppError::new(kind, message));
    }
    if response["success"].as_bool() != Some(true) {
        return Err(invalid_response());
    }
    response
        .get("result")
        .filter(|v| v.is_object())
        .ok_or_else(invalid_response)
}

fn required<'a>(data: &'a Value, field: &str) -> Result<&'a str> {
    data[field]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid_response)
}

fn number(data: &Value, field: &str) -> Result<u32> {
    data[field]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(invalid_response)
}

fn optional_array<'a>(data: &'a Value, field: &str) -> Result<&'a [Value]> {
    match data.get(field) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(rows)) => Ok(rows),
        _ => Err(invalid_response()),
    }
}

fn summary(data: &Value) -> Result<ProjectSummary> {
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

fn web_url(value: &str, base: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let url = if value.starts_with("//") {
        Url::parse(&format!("https:{value}")).ok()?
    } else {
        Url::parse(base).ok()?.join(value).ok()?
    };
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.into())
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
                supplier_part: row["Supplier Part"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

fn content_blocks(html: &str) -> Result<Vec<ContentBlock>> {
    if html.len() > 2 * 1024 * 1024 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文超过安全大小上限"));
    }
    let fragment = scraper::Html::parse_fragment(html);
    let mut blocks = Vec::new();
    let mut text = String::new();
    walk_html(fragment.root_element(), 0, &mut blocks, &mut text)?;
    flush_text(&mut blocks, &mut text)?;
    Ok(blocks)
}

fn excluded(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "iframe"
            | "object"
            | "embed"
            | "noscript"
            | "template"
            | "svg"
            | "canvas"
    )
}

fn inline_text(element: ElementRef<'_>, depth: usize) -> Result<String> {
    check_depth(depth)?;
    let mut text = String::new();
    for node in element.children() {
        if let Some(value) = node.value().as_text() {
            text.push_str(value);
        } else if let Some(child) = ElementRef::wrap(node)
            && !excluded(child.value().name())
        {
            if child.value().name() == "br" {
                text.push('\n');
            } else {
                text.push_str(&inline_text(child, depth + 1)?);
            }
        }
    }
    Ok(text)
}

fn check_depth(depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文嵌套过深"));
    }
    Ok(())
}

fn push_block(
    blocks: &mut Vec<ContentBlock>,
    kind: &str,
    text: String,
    url: Option<String>,
) -> Result<()> {
    let text = if kind == "code" {
        text.trim_matches('\n').to_owned()
    } else {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    if text.is_empty() && kind != "image" {
        return Ok(());
    }
    if blocks.len() >= 2048 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文内容块过多"));
    }
    blocks.push(ContentBlock {
        kind: kind.to_owned(),
        text,
        url,
    });
    Ok(())
}

fn flush_text(blocks: &mut Vec<ContentBlock>, text: &mut String) -> Result<()> {
    push_block(blocks, "paragraph", std::mem::take(text), None)
}

fn walk_html(
    element: ElementRef<'_>,
    depth: usize,
    blocks: &mut Vec<ContentBlock>,
    text: &mut String,
) -> Result<()> {
    check_depth(depth)?;
    for node in element.children() {
        if let Some(value) = node.value().as_text() {
            text.push_str(value);
            continue;
        }
        let Some(child) = ElementRef::wrap(node) else {
            continue;
        };
        let tag = child.value().name();
        if excluded(tag) {
            continue;
        }
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "pre" => {
                flush_text(blocks, text)?;
                let kind = if tag == "pre" { "code" } else { "heading" };
                push_block(blocks, kind, inline_text(child, depth + 1)?, None)?;
            }
            "img" => {
                flush_text(blocks, text)?;
                if let Some(url) = child
                    .value()
                    .attr("src")
                    .and_then(|s| web_url(s, "https://oshwhub.com"))
                {
                    push_block(
                        blocks,
                        "image",
                        child.value().attr("alt").unwrap_or_default().to_owned(),
                        Some(url),
                    )?;
                }
            }
            "a" => {
                if child.descendants().any(|node| {
                    node.value()
                        .as_element()
                        .is_some_and(|element| element.name() == "img")
                }) {
                    flush_text(blocks, text)?;
                    walk_html(child, depth + 1, blocks, text)?;
                    continue;
                }
                let label = inline_text(child, depth + 1)?;
                if let Some(url) = child
                    .value()
                    .attr("href")
                    .and_then(|s| web_url(s, "https://oshwhub.com"))
                {
                    flush_text(blocks, text)?;
                    let label = if label.trim().is_empty() {
                        url.clone()
                    } else {
                        label
                    };
                    push_block(blocks, "link", label, Some(url))?;
                } else {
                    text.push_str(&label);
                }
            }
            "br" => text.push('\n'),
            "div" | "section" | "article" | "p" | "li" | "blockquote" | "ul" | "ol" | "table"
            | "thead" | "tbody" | "tr" | "td" | "th" | "figure" | "figcaption" | "header"
            | "footer" | "main" | "hr" => {
                flush_text(blocks, text)?;
                let mut child_text = String::new();
                walk_html(child, depth + 1, blocks, &mut child_text)?;
                flush_text(blocks, &mut child_text)?;
            }
            _ => walk_html(child, depth + 1, blocks, text)?,
        }
    }
    Ok(())
}
