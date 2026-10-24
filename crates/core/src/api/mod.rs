mod content;
mod feed;
mod project;
mod response;

pub use feed::{feed_request, normalize_feed};
pub use project::normalize_project;

use crate::{
    client::HttpClient,
    error::{AppError, Result},
};

pub struct Api {
    client: HttpClient,
}

impl Api {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }
}

pub fn validate_id(id: &str) -> Result<()> {
    if !matches!(id.len(), 16 | 32) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::new("INVALID_ID", "工程或文档 ID 格式无效"));
    }
    Ok(())
}
