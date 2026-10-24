mod content;
mod feed;
mod project;
mod response;

pub use feed::{feed_request, normalize_feed};
pub use project::normalize_project;

use crate::{
    client::HttpClient,
    error::{AppError, Result},
    model::{Comment, CommentsPage, ProjectStatus, User},
};
use serde_json::{Value, json};

pub struct Api {
    client: HttpClient,
}

impl Api {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn login(&self, code: &str) -> Result<User> {
        validate_login_code(code)?;
        let response = self
            .client
            .post_json(
                "https://oshwhub.com/api/login/login-by-code",
                &json!({"code": code}),
            )
            .await?;
        ensure_success(&response)?;
        if response["result"].is_string() {
            return Err(AppError::new("AUTH_REQUIRED", "登录授权码无效或已过期"));
        }
        self.current_user().await
    }

    pub async fn current_user(&self) -> Result<User> {
        let response = self
            .client
            .get_json("https://oshwhub.com/api/users")
            .await?;
        user(payload(&response)?)
    }

    pub async fn logout(&self) -> Result<()> {
        let result = self
            .client
            .get_json("https://oshwhub.com/api/logout")
            .await
            .and_then(|response| ensure_success(&response));
        self.client.clear_cookies();
        result
    }

    pub async fn project_status(&self, id: &str) -> Result<ProjectStatus> {
        validate_id(id)?;
        let response = self
            .client
            .get_json(&format!(
                "https://oshwhub.com/api/common/projects/{id}/status"
            ))
            .await?;
        let data = payload(&response)?;
        Ok(ProjectStatus {
            album_collect: required_bool(data, "albumCollect")?,
            like: required_bool(data, "like")?,
            collect: required_bool(data, "collect")?,
        })
    }

    pub async fn set_project_collection(&self, id: &str, collect: bool) -> Result<ProjectStatus> {
        validate_id(id)?;
        let url = format!("https://oshwhub.com/api/common/projects/{id}/collect");
        let response = if collect {
            self.client.post_json(&url, &json!({})).await?
        } else {
            self.client.delete_json(&url).await?
        };
        ensure_success(&response)?;
        self.project_status(id).await
    }

    pub async fn comments(
        &self,
        id: &str,
        sort: &str,
        page: u32,
        page_size: u32,
    ) -> Result<CommentsPage> {
        validate_id(id)?;
        if !matches!(sort, "hot" | "new")
            || !(1..=10_000).contains(&page)
            || !(1..=50).contains(&page_size)
        {
            return Err(AppError::new("INVALID_QUERY", "评论查询参数无效"));
        }
        let url = format!(
            "https://oshwhub.com/api/common/projects/{id}/comments?sort={sort}&page={page}&pageSize={page_size}"
        );
        let response = self.client.get_json(&url).await?;
        let data = payload(&response)?;
        let total = data["total"].as_u64().ok_or_else(invalid_response)?;
        let total_page = data["totalPage"].as_u64().unwrap_or_default();
        let items = optional_array(data, "lists")?
            .iter()
            .map(|row| comment(row, 0))
            .collect::<Result<Vec<_>>>()?;
        Ok(CommentsPage {
            items,
            page: data["page"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(invalid_response)?,
            total,
            has_more: total_page > 0 && u64::from(page) < total_page,
        })
    }

    pub async fn create_comment(&self, id: &str, content: &str) -> Result<Comment> {
        validate_id(id)?;
        validate_comment(content)?;
        let response = self
            .client
            .post_json(
                &format!("https://oshwhub.com/api/common/projects/{id}/comments/create"),
                &json!({"content": content.trim()}),
            )
            .await?;
        comment(payload(&response)?, 0)
    }
}

fn required_bool(data: &Value, field: &str) -> Result<bool> {
    data[field].as_bool().ok_or_else(invalid_response)
}

fn user(data: &Value) -> Result<User> {
    let id = data["uuid"]
        .as_str()
        .or_else(|| data["id"].as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_response)?;
    let username = data["username"].as_str().unwrap_or_default().to_owned();
    let nickname = data["nickname"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or(&username)
        .to_owned();
    if nickname.is_empty() {
        return Err(invalid_response());
    }
    Ok(User {
        id: id.to_owned(),
        username,
        nickname,
        avatar: data["avatar"]
            .as_str()
            .and_then(|value| web_url(value, "https://oshwhub.com")),
    })
}

fn comment(data: &Value, depth: usize) -> Result<Comment> {
    if depth > 4 {
        return Err(invalid_response());
    }
    Ok(Comment {
        uuid: required(data, "uuid")?.to_owned(),
        content: required(data, "content")?.to_owned(),
        creator: user(data.get("creator").ok_or_else(invalid_response)?)?,
        created_at: required(data, "created_at")?.to_owned(),
        like: data["is_like"].as_bool().unwrap_or(false),
        replies: optional_array(data, "replies")?
            .iter()
            .map(|reply| comment(reply, depth + 1))
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn validate_id(id: &str) -> Result<()> {
    if !matches!(id.len(), 16 | 32) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::new("INVALID_ID", "工程或文档 ID 格式无效"));
    }
    Ok(())
}

pub fn validate_login_code(code: &str) -> Result<()> {
    if !(1..=512).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
    {
        return Err(AppError::new("INVALID_CODE", "登录授权码格式无效"));
    }
    Ok(())
}

pub fn validate_comment(content: &str) -> Result<()> {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 2000 || content.chars().any(char::is_control)
    {
        return Err(AppError::new("INVALID_COMMENT", "评论内容无效"));
    }
    Ok(())
}

use response::{ensure_success, invalid_response, optional_array, payload, required, web_url};
