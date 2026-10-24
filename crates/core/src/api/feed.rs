use super::{
    Api,
    project::summary,
    response::{invalid_response, number, payload},
};
use crate::{
    error::{AppError, Result},
    model::{FeedPage, FeedQuery},
};
use serde_json::{Value, json};
use std::collections::HashSet;
use url::Url;

impl Api {
    pub async fn feed(&self, query: FeedQuery) -> Result<FeedPage> {
        let (url, body) = feed_request(&query)?;
        let response = if let Some(body) = body {
            self.client.post_json(&url, &body).await?
        } else {
            self.client.get_json(&url).await?
        };
        normalize_feed(&response)
    }
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
