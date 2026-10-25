use crate::error::{AppError, Result};
use serde_json::Value;
use std::time::{Duration, Instant};
use url::Url;

#[derive(Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
}

const JSON_LIMIT: usize = 8 * 1024 * 1024;
const ALLOWED_HOSTS: &[&str] = &[
    "oshwhub.com",
    "pro.lceda.cn",
    "modules.lceda.cn",
    "image-pro.lceda.cn",
    "image.lceda.cn",
    "static.oshwhub.com",
];

impl HttpClient {
    pub fn new() -> Result<Self> {
        let inner = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AppError::new("NETWORK", "无法初始化网络客户端"))?;
        Ok(Self { inner })
    }

    pub async fn get_json(&self, url: &str) -> Result<Value> {
        let bytes = self.request(url, None, JSON_LIMIT, true, &[]).await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("INVALID_RESPONSE", "站点返回了无法解析的 JSON"))
    }

    pub(crate) async fn get_json_with_headers(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let bytes = self.request(url, None, JSON_LIMIT, true, headers).await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("INVALID_RESPONSE", "站点返回了无法解析的 JSON"))
    }

    pub async fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let bytes = self.request(url, Some(body), JSON_LIMIT, true, &[]).await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("INVALID_RESPONSE", "站点返回了无法解析的 JSON"))
    }

    pub async fn get_bytes(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>> {
        self.request(url, None, max_bytes, false, &[]).await
    }

    async fn request(
        &self,
        url: &str,
        body: Option<&Value>,
        max_bytes: usize,
        json: bool,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        let url = validate_url(url)?;
        if max_bytes == 0 {
            return Err(AppError::new("INVALID_LIMIT", "下载大小上限必须大于零"));
        }
        let started = Instant::now();
        let request = if let Some(body) = body {
            self.inner.post(url.clone()).json(body)
        } else {
            self.inner.get(url.clone())
        };
        let mut request = if json {
            request.header(reqwest::header::ACCEPT, "application/json")
        } else {
            request
        };
        for &(name, value) in headers {
            request = request.header(name, value);
        }
        let mut response = request.send().await.map_err(network_error)?;
        let status = response.status().as_u16();
        tracing::debug!(
            method = if body.is_some() { "POST" } else { "GET" },
            host = url.host_str().unwrap_or_default(),
            status,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "public request response"
        );
        validate_response(
            status,
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            json,
        )?;
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(AppError::new("RESPONSE_TOO_LARGE", "站点响应超过大小上限"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            append_chunk(&mut bytes, &chunk, max_bytes)?;
        }
        Ok(bytes)
    }
}

pub fn validate_url(url: &str) -> Result<Url> {
    let invalid = || AppError::new("INVALID_URL", "下载地址不在允许的 HTTPS 站点范围");
    if url.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = Url::parse(url).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !url
            .host_str()
            .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
    {
        return Err(invalid());
    }
    Ok(url)
}

fn network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::new("TIMEOUT", "网络请求超时，请重试")
    } else {
        AppError::new("NETWORK", "无法连接站点，请检查网络后重试")
    }
}

fn validate_response(status: u16, mime: Option<&str>, json: bool) -> Result<()> {
    match status {
        401 => return Err(AppError::new("AUTH_REQUIRED", "该内容需要登录后访问")),
        403 | 418 => return Err(AppError::new("BLOCKED", "站点暂时拒绝了请求，请稍后重试")),
        429 => return Err(AppError::new("RATE_LIMITED", "请求过于频繁，请稍后重试")),
        300..=399 => {
            return Err(AppError::new("REDIRECT_BLOCKED", "站点返回了不允许的跳转"));
        }
        200..=299 => {}
        _ => {
            return Err(AppError::new(
                "HTTP_ERROR",
                format!("站点请求失败（HTTP {status}）"),
            ));
        }
    }
    let mime = mime
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if mime == "text/html" || mime == "application/xhtml+xml" {
        return Err(AppError::new("BLOCKED", "站点返回了验证或拦截页面"));
    }
    if json
        && mime != "application/json"
        && !(mime.starts_with("application/") && mime.ends_with("+json"))
    {
        return Err(AppError::new("INVALID_RESPONSE", "站点没有返回 JSON 数据"));
    }
    Ok(())
}

fn append_chunk(bytes: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<()> {
    if bytes
        .len()
        .checked_add(chunk.len())
        .is_none_or(|length| length > limit)
    {
        return Err(AppError::new("RESPONSE_TOO_LARGE", "站点响应超过大小上限"));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_rate_limited_and_redirect_responses_remain_errors() {
        for (status, code) in [
            (418, "BLOCKED"),
            (403, "BLOCKED"),
            (429, "RATE_LIMITED"),
            (302, "REDIRECT_BLOCKED"),
            (401, "AUTH_REQUIRED"),
            (503, "HTTP_ERROR"),
        ] {
            assert_eq!(
                validate_response(status, Some("application/json"), true)
                    .unwrap_err()
                    .code,
                code
            );
        }
    }

    #[test]
    fn refuses_html_as_json_but_accepts_binary_pcb_mime() {
        assert_eq!(
            validate_response(200, Some("text/html; charset=utf-8"), true)
                .unwrap_err()
                .code,
            "BLOCKED"
        );
        assert_eq!(
            validate_response(200, Some("text/html"), false)
                .unwrap_err()
                .code,
            "BLOCKED"
        );
        assert_eq!(
            validate_response(200, Some("text/plain"), true)
                .unwrap_err()
                .code,
            "INVALID_RESPONSE"
        );
        validate_response(200, Some("application/json; charset=UTF-8"), true).unwrap();
        validate_response(200, Some("application/problem+json"), true).unwrap();
        validate_response(200, Some("application/octet-stream"), false).unwrap();
    }

    #[test]
    fn response_limit_is_checked_before_extending_the_buffer() {
        let mut bytes = vec![1, 2];
        assert_eq!(
            append_chunk(&mut bytes, &[3, 4], 3).unwrap_err().code,
            "RESPONSE_TOO_LARGE"
        );
        assert_eq!(bytes, vec![1, 2]);
        append_chunk(&mut bytes, &[3], 3).unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
    }
}
