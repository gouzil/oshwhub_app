//! Test-only browser entry to the actual core, without an application runtime.
#![cfg(target_arch = "wasm32")]

use oshwhub_core::client::HttpClient;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

thread_local! {
    static CANCEL: std::cell::RefCell<Option<futures_util::future::AbortHandle>> = const { std::cell::RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn fetch_probe(url: &str, limit: u32, mode: &str) -> String {
    let result = async {
        let (client, mode) = if let Some(mode) = mode.strip_prefix("local-") {
            (HttpClient::via_local_service(), mode)
        } else { (HttpClient::new()?, mode) };
        match mode {
            "json" => client.get_json(url).await,
            "post" => client.post_json(url, &json!({"probe": true})).await,
            _ => client.get_bytes(url, limit as usize).await.map(
                |bytes| json!({"length": bytes.len(), "text": String::from_utf8_lossy(&bytes)}),
            ),
        }
    }
    .await;
    match result {
        Ok(data) => json!({"ok": true, "data": data}),
        Err(error) => json!({"ok": false, "code": error.code}),
    }
    .to_string()
}

#[wasm_bindgen]
pub async fn command(input: &str) -> String {
    oshwhub_core::dispatch_json(input).await
}

#[wasm_bindgen]
pub fn start_probe(url: String) {
    use futures_util::future::{AbortHandle, Abortable};
    let (cancel, registration) = AbortHandle::new_pair();
    CANCEL.with(|slot| {
        if let Some(old) = slot.replace(Some(cancel)) {
            old.abort();
        }
    });
    wasm_bindgen_futures::spawn_local(async move {
        let _ = Abortable::new(
            async { HttpClient::new()?.get_bytes(&url, 32).await },
            registration,
        )
        .await;
    });
}

#[wasm_bindgen]
pub fn cancel_probe() {
    CANCEL.with(|slot| {
        if let Some(cancel) = slot.take() {
            cancel.abort();
        }
    });
}

#[wasm_bindgen]
pub fn validate_pick(scene: &str) -> String {
    let result = oshwhub_core::pcb::pick(scene, f64::NAN, 0.0, 1.0, None);
    match result {
        Ok(_) => Value::Null,
        Err(error) => json!(error.code),
    }
    .to_string()
}
