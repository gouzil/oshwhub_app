//! Browser transport with the same public-request limits as the native client.

use std::{cell::Cell, rc::Rc};

use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AbortController, ReadableStreamDefaultReader, Request, RequestCredentials, RequestInit,
    RequestMode, RequestRedirect, Response, ResponseType, Window,
};

use super::{HttpClient, append_chunk, validate_response, validate_url};
use crate::error::{AppError, Result};

struct Deadline {
    window: Window,
    controller: AbortController,
    expired: Rc<Cell<bool>>,
    timer: i32,
    _callback: Closure<dyn FnMut()>,
}

impl Deadline {
    fn new() -> Result<Self> {
        let window = web_sys::window().ok_or_else(network_error)?;
        let controller = AbortController::new().map_err(|_| network_error())?;
        let expired = Rc::new(Cell::new(false));
        let flag = Rc::clone(&expired);
        let abort = controller.clone();
        let callback = Closure::new(move || {
            flag.set(true);
            abort.abort();
        });
        let timer = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                30_000,
            )
            .map_err(|_| network_error())?;
        Ok(Self {
            window,
            controller,
            expired,
            timer,
            _callback: callback,
        })
    }

    fn error(&self) -> AppError {
        if self.expired.get() {
            AppError::new("TIMEOUT", "网络请求超时，请重试")
        } else {
            network_error()
        }
    }
}

impl Drop for Deadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}

fn network_error() -> AppError {
    AppError::new("NETWORK", "无法连接站点，请检查网络后重试")
}

impl HttpClient {
    pub(super) async fn request(
        &self,
        url: &str,
        body: Option<&serde_json::Value>,
        max_bytes: usize,
        json: bool,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        let url = validate_url(url)?;
        if max_bytes == 0 {
            return Err(AppError::new("INVALID_LIMIT", "下载大小上限必须大于零"));
        }
        let deadline = Deadline::new()?;
        let init = RequestInit::new();
        init.set_method(if body.is_some() { "POST" } else { "GET" });
        init.set_mode(RequestMode::Cors);
        init.set_credentials(RequestCredentials::Omit);
        // Manual yields an opaque redirect without fetching its destination.
        init.set_redirect(RequestRedirect::Manual);
        init.set_signal(Some(&deadline.controller.signal()));
        if let Some(body) = body {
            init.set_body(&JsValue::from_str(&body.to_string()));
        }
        let request =
            Request::new_with_str_and_init(url.as_str(), &init).map_err(|_| network_error())?;
        if json {
            request
                .headers()
                .set("Accept", "application/json")
                .map_err(|_| network_error())?;
        }
        if body.is_some() {
            request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| network_error())?;
        }
        for &(name, value) in headers {
            request
                .headers()
                .set(name, value)
                .map_err(|_| network_error())?;
        }
        let response: Response = JsFuture::from(deadline.window.fetch_with_request(&request))
            .await
            .map_err(|_| deadline.error())?
            .dyn_into()
            .map_err(|_| network_error())?;
        if response.type_() == ResponseType::Opaqueredirect {
            return Err(AppError::new("REDIRECT_BLOCKED", "站点返回了不允许的跳转"));
        }
        let mime = response
            .headers()
            .get("Content-Type")
            .map_err(|_| network_error())?;
        validate_response(response.status(), mime.as_deref(), json)?;
        if response
            .headers()
            .get("Content-Length")
            .map_err(|_| network_error())?
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(AppError::new("RESPONSE_TOO_LARGE", "站点响应超过大小上限"));
        }
        let mut bytes = Vec::new();
        if let Some(body) = response.body() {
            let reader: ReadableStreamDefaultReader = body.get_reader().unchecked_into();
            loop {
                let chunk = JsFuture::from(reader.read())
                    .await
                    .map_err(|_| deadline.error())?;
                if Reflect::get(&chunk, &JsValue::from_str("done"))
                    .map_err(|_| network_error())?
                    .as_bool()
                    == Some(true)
                {
                    break;
                }
                let value = Reflect::get(&chunk, &JsValue::from_str("value"))
                    .map_err(|_| network_error())?;
                let value = Uint8Array::new(&value);
                if bytes
                    .len()
                    .checked_add(value.length() as usize)
                    .is_none_or(|length| length > max_bytes)
                {
                    return Err(AppError::new("RESPONSE_TOO_LARGE", "站点响应超过大小上限"));
                }
                append_chunk(&mut bytes, &value.to_vec(), max_bytes)?;
            }
            reader.release_lock();
        }
        Ok(bytes)
    }
}
