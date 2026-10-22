//! Stable C/JNI boundary; application behavior lives in oshwhub-core.
use std::{
    ffi::{CStr, CString, c_char},
    sync::OnceLock,
};

fn error_response(code: &str, message: &str) -> String {
    let request_id = uuid::Uuid::new_v4().to_string();
    tracing::error!(code, request_id, "ffi.request.failed");
    serde_json::json!({"ok": false, "error": {"code": code, "message": message, "requestId": request_id}}).to_string()
}

pub fn dispatch(input: &str) -> String {
    static RUNTIME: OnceLock<std::result::Result<tokio::runtime::Runtime, std::io::Error>> =
        OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("oshwhub-core")
            .enable_all()
            .build()
    });
    match runtime {
        Ok(runtime) => {
            match std::panic::catch_unwind(|| runtime.block_on(oshwhub_core::dispatch_json(input)))
            {
                Ok(response) => response,
                Err(_) => error_response("internal", "Rust 核心发生错误，请查看诊断日志"),
            }
        }
        Err(_) => error_response("internal", "无法启动 Rust 核心运行时"),
    }
}

/// # Safety
/// `input` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oshwhub_execute(input: *const c_char) -> *mut c_char {
    let output = if input.is_null() {
        error_response("invalid_request", "请求指针为空")
    } else {
        // SAFETY: The caller promises a NUL-terminated string valid for this call.
        match unsafe { CStr::from_ptr(input) }.to_str() {
            Ok(input) => dispatch(input),
            Err(_) => error_response("invalid_request", "请求必须使用 UTF-8"),
        }
    };
    CString::new(output)
        .expect("JSON encoding escapes NUL")
        .into_raw()
}

/// # Safety
/// `response` must be null or a pointer returned by oshwhub_execute, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oshwhub_free(response: *mut c_char) {
    if !response.is_null() {
        // SAFETY: This pointer was allocated by CString::into_raw above.
        drop(unsafe { CString::from_raw(response) });
    }
}

pub fn initialize_logging(directory: &str) {
    if directory.is_empty() {
        return;
    }
    if let Err(error) = oshwhub_core::diagnostics::initialize(std::path::Path::new(directory)) {
        eprintln!("Rust diagnostic log unavailable ({:?})", error.kind());
    }
}

/// # Safety
/// `directory` must be null or a NUL-terminated UTF-8 app cache directory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oshwhub_initialize(directory: *const c_char) {
    if !directory.is_null() {
        // SAFETY: The caller promises the same lifetime as this call.
        if let Ok(path) = unsafe { CStr::from_ptr(directory) }.to_str() {
            initialize_logging(path);
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::{
        JNIEnv,
        objects::{JObject, JString},
        sys::jstring,
    };

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_expo_modules_oshwhubcore_OshwhubCoreModule_nativeDispatch(
        mut env: JNIEnv,
        _object: JObject,
        input: JString,
    ) -> jstring {
        let response = match env.get_string(&input) {
            Ok(input) => super::dispatch(&String::from(input)),
            Err(_) => super::error_response("invalid_request", "无法读取原生请求"),
        };
        match env.new_string(response) {
            Ok(output) => output.into_raw(),
            Err(_) => {
                let _ = env.throw_new(
                    "java/lang/OutOfMemoryError",
                    "Unable to allocate Rust response",
                );
                std::ptr::null_mut()
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_expo_modules_oshwhubcore_OshwhubCoreModule_nativeInitialize(
        mut env: JNIEnv,
        _object: JObject,
        directory: JString,
    ) {
        if let Ok(path) = env.get_string(&directory) {
            super::initialize_logging(&String::from(path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_abi_roundtrip_and_owned_response() {
        let input = CString::new(r#"{"command":"health"}"#).unwrap();
        // SAFETY: The CString and response obey the documented C ABI contract.
        unsafe {
            let response = oshwhub_execute(input.as_ptr());
            let value: serde_json::Value =
                serde_json::from_slice(CStr::from_ptr(response).to_bytes()).unwrap();
            assert_eq!(value["data"]["protocol"], 1);
            oshwhub_free(response);
            let response = oshwhub_execute(std::ptr::null());
            assert!(
                CStr::from_ptr(response)
                    .to_str()
                    .unwrap()
                    .contains("invalid_request")
            );
            oshwhub_free(response);
            oshwhub_free(std::ptr::null_mut());
        }
    }
}
