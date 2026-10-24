use crate::error::{AppError, Result};
use serde_json::Value;
use url::Url;

pub(super) fn invalid_response() -> AppError {
    AppError::new("INVALID_RESPONSE", "站点数据格式不完整或已变化")
}

pub(super) fn payload(response: &Value) -> Result<&Value> {
    ensure_success(response)?;
    response
        .get("result")
        .filter(|v| v.is_object())
        .ok_or_else(invalid_response)
}

pub(super) fn ensure_success(response: &Value) -> Result<()> {
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
    Ok(())
}

pub(super) fn required<'a>(data: &'a Value, field: &str) -> Result<&'a str> {
    data[field]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid_response)
}

pub(super) fn number(data: &Value, field: &str) -> Result<u32> {
    data[field]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(invalid_response)
}

pub(super) fn optional_array<'a>(data: &'a Value, field: &str) -> Result<&'a [Value]> {
    match data.get(field) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(rows)) => Ok(rows),
        _ => Err(invalid_response()),
    }
}

pub(super) fn web_url(value: &str, base: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_business_errors_without_exposing_response_details() {
        for (code, expected) in [
            (401, "AUTH_REQUIRED"),
            (403, "BLOCKED"),
            (429, "RATE_LIMITED"),
        ] {
            let response = json!({"success": false, "code": code, "result": null});
            assert_eq!(ensure_success(&response).unwrap_err().code, expected);
        }
        let malformed = json!({"success": true, "code": 0, "result": "unexpected"});
        assert_eq!(payload(&malformed).unwrap_err().code, "INVALID_RESPONSE");
    }
}
