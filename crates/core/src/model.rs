use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: String,
    pub cover: Option<String>,
    pub author: String,
    pub author_avatar: Option<String>,
    pub likes: u64,
    pub views: u64,
    pub tags: Vec<String>,
    pub updated_at: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContentBlock {
    pub kind: String,
    pub text: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BomItem {
    pub name: String,
    pub designator: String,
    pub footprint: String,
    pub quantity: u32,
    pub part_number: String,
    pub manufacturer: String,
    pub supplier: String,
    pub supplier_part: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub name: String,
    pub pcb_id: Option<String>,
    pub thumbnail: Option<String>,
    pub bom: Vec<BomItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub summary: ProjectSummary,
    pub content: Vec<ContentBlock>,
    pub boards: Vec<Board>,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeedPage {
    pub items: Vec<ProjectSummary>,
    pub page: u32,
    pub total: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedQuery {
    pub page: u32,
    pub page_size: u32,
    pub query: String,
    pub sort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub album_collect: bool,
    pub like: bool,
    pub collect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub uuid: String,
    pub content: String,
    pub creator: User,
    pub created_at: String,
    pub like: bool,
    pub replies: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommentsPage {
    pub items: Vec<Comment>,
    pub page: u32,
    pub total: u64,
    pub has_more: bool,
}
