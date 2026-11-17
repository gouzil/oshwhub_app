use std::sync::Mutex;

#[cfg(target_os = "android")]
use nsh::jni::{
    JNIEnv,
    objects::{JClass, JString},
};

use super::{AppError, Result};

static LEGACY: Mutex<Option<Result<[Option<String>; 2]>>> = Mutex::new(None);

pub(super) fn legacy_preferences() -> Result<[Option<String>; 2]> {
    LEGACY
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
        .unwrap_or_else(|| {
            Err(AppError::new(
                "PREFERENCES_UNAVAILABLE",
                "尚未读取 Android 原有外观偏好",
            ))
        })
}

fn initialize_legacy_preferences(values: Result<[Option<String>; 2]>) {
    *LEGACY.lock().unwrap_or_else(|error| error.into_inner()) = Some(values);
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_longbridge_nsh_AppActivity_initializeLegacyPreferences(
    mut env: JNIEnv,
    _class: JClass,
    mode: JString,
    color: JString,
) {
    let mut read = |value: JString| {
        if value.is_null() {
            return Ok(None);
        }
        env.get_string(&value)
            .map(|value| Some(String::from(value)))
            .map_err(|_| AppError::new("PREFERENCES_READ", "无法读取 Android 原有外观偏好"))
    };
    let values = read(mode).and_then(|mode| read(color).map(|color| [mode, color]));
    if values.is_err() {
        // Surface the read failure through the Rust settings page instead of a JNI exception.
        let _ = env.exception_clear();
    }
    initialize_legacy_preferences(values);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_legacy_read_can_be_reinitialized_after_activity_recreation() {
        initialize_legacy_preferences(Err(AppError::new("PREFERENCES_READ", "read failed")));
        assert!(legacy_preferences().is_err());
        initialize_legacy_preferences(Ok([Some("dark".into()), Some("purple".into())]));
        assert_eq!(
            legacy_preferences().unwrap(),
            [Some("dark".into()), Some("purple".into())]
        );
    }
}
