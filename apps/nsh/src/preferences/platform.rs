#[cfg(target_arch = "wasm32")]
pub fn system_store() -> super::WebStore {
    super::WebStore
}

#[cfg(any(target_os = "android", test))]
pub struct NshStore {
    legacy: super::Result<[Option<String>; 2]>,
}

#[cfg(any(target_os = "android", test))]
impl NshStore {
    fn new(legacy: super::Result<[Option<String>; 2]>) -> Self {
        Self { legacy }
    }
}

#[cfg(target_os = "android")]
pub fn system_store() -> NshStore {
    NshStore::new(super::android::legacy_preferences())
}

#[cfg(any(target_os = "android", test))]
impl super::PreferenceStore for NshStore {
    fn read(&self, key: &str) -> super::Result<Option<String>> {
        if let Some(value) = nsh_core::storage::Storage::get_item(key) {
            return Ok(Some(value));
        }
        let values = self.legacy.as_ref().map_err(Clone::clone)?;
        let legacy = match key {
            super::THEME_MODE_KEY => values[0]
                .as_deref()
                .filter(|value| super::parse_appearance(value).is_some()),
            super::THEME_COLOR_KEY => values[1]
                .as_deref()
                .filter(|value| super::parse_accent(value).is_some()),
            _ => None,
        };
        Ok(legacy.map(str::to_owned))
    }

    fn write(&mut self, key: &str, value: &str) -> super::Result<()> {
        // The Android host supplies its private files directory to NSH storage.
        nsh_core::storage::Storage::set_item(key, value)
            .map_err(|_| super::AppError::new("PREFERENCES_SAVE", "无法保存外观偏好"))
    }
}

#[cfg(target_os = "ios")]
pub struct IosStore;

#[cfg(target_os = "ios")]
pub fn system_store() -> IosStore {
    IosStore
}

#[cfg(target_os = "ios")]
impl super::PreferenceStore for IosStore {
    fn read(&self, key: &str) -> super::Result<Option<String>> {
        use objc2_foundation::{NSString, NSUserDefaults};
        Ok(NSUserDefaults::standardUserDefaults()
            .stringForKey(&NSString::from_str(key.trim_start_matches("oshwhub.")))
            .map(|value| value.to_string()))
    }

    fn write(&mut self, key: &str, value: &str) -> super::Result<()> {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        let value = NSString::from_str(value);
        let key = NSString::from_str(key.trim_start_matches("oshwhub."));
        // SAFETY: NSString is a valid property-list object and both arguments outlive the call.
        unsafe { defaults.setObject_forKey(Some(&value), &key) };
        if defaults.synchronize() {
            Ok(())
        } else {
            Err(super::AppError::new("PREFERENCES_SAVE", "无法保存外观偏好"))
        }
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "ios", target_os = "android")))]
pub fn system_store() -> super::Result<super::FileStore> {
    use std::path::PathBuf;
    let missing = || super::AppError::new("PREFERENCES_UNAVAILABLE", "无法定位应用偏好目录");
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME").ok_or_else(missing)?)
        .join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").ok_or_else(missing)?);
    #[cfg(target_os = "linux")]
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(std::env::var_os("HOME").ok_or_else(missing)?).join(".config"),
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let base = PathBuf::from(std::env::var_os("OSHWHUB_DATA_DIR").ok_or_else(missing)?);
    Ok(super::FileStore::new(
        base.join("com.gouzil.oshwhub/preferences.json"),
    ))
}

#[cfg(test)]
mod tests {
    use nsh_core::{Size, element::Root, prelude::*};
    use nsh_engine::EngineConfig;
    use nsh_testing::Headless;

    #[component]
    fn StoredApp() -> impl IntoElement {
        crate::app_with_store(super::NshStore::new(Ok([None, None])))
    }

    #[component]
    fn LegacyApp() -> impl IntoElement {
        crate::app_with_store(super::NshStore::new(Ok([
            Some("dark".into()),
            Some("purple".into()),
        ])))
    }

    #[component]
    fn LegacyProbe() -> impl IntoElement {
        let value = crate::preferences::ThemePreferences::load(&super::NshStore::new(Ok([
            Some("dark".into()),
            Some("purple".into()),
        ])))
        .unwrap();
        text(format!("{:?} {}", value.appearance, value.accent.id()))
    }

    #[component]
    fn FailedLegacyApp() -> impl IntoElement {
        crate::app_with_store(super::NshStore::new(Err(super::super::AppError::new(
            "PREFERENCES_READ",
            "原有外观偏好读取失败",
        ))))
    }

    #[test]
    fn failed_legacy_read_does_not_block_current_preferences_or_new_saves() {
        for (fixture, expected_color) in [
            (
                r#"{"oshwhub.themeMode":"light","oshwhub.themeColor":"orange"}"#,
                Some("orange"),
            ),
            (r#"{"oshwhub.themeMode":"light"}"#, None),
        ] {
            let directory = std::env::temp_dir().join(format!(
                "oshwhub-failed-legacy-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("storage.json"), fixture).unwrap();
            let open = || {
                Headless::start(EngineConfig {
                    viewport: Size::new(px(390.), px(844.)),
                    storage: Some(nsh_engine::FileStorage::open(&directory)),
                    ..EngineConfig::new(
                        Root::of::<FailedLegacyApp>().tokens(crate::app_theme().into_tokens()),
                    )
                })
                .unwrap()
            };
            {
                let mut app = open();
                app.first_render().unwrap();
                app.press("label:打开设置").unwrap();
                app.wait_for("text:当前外观：浅色模式").unwrap();
                if expected_color.is_none() {
                    app.wait_for("text:原有外观偏好读取失败").unwrap();
                }
                app.press("label:暗色模式").unwrap();
            }
            {
                let mut app = open();
                app.first_render().unwrap();
                app.press("label:打开设置").unwrap();
                app.wait_for("text:当前外观：暗色模式").unwrap();
            }
            let stored: serde_json::Value =
                serde_json::from_slice(&std::fs::read(directory.join("storage.json")).unwrap())
                    .unwrap();
            assert_eq!(stored["oshwhub.themeMode"], "dark");
            assert_eq!(
                stored
                    .get("oshwhub.themeColor")
                    .and_then(|value| value.as_str()),
                expected_color
            );
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn legacy_android_theme_is_used_until_the_user_saves_a_new_choice() {
        let directory = std::env::temp_dir().join(format!(
            "oshwhub-legacy-preferences-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let open = |root| {
            Headless::start(EngineConfig {
                viewport: Size::new(px(390.), px(844.)),
                storage: Some(nsh_engine::FileStorage::open(&directory)),
                ..EngineConfig::new(root)
            })
            .unwrap()
        };
        {
            let mut app = open(Root::of::<LegacyProbe>());
            app.first_render().unwrap();
            app.wait_for("text:Dark purple").unwrap();
            assert!(!directory.join("storage.json").exists());
        }
        {
            let mut app = open(Root::of::<LegacyApp>().tokens(crate::app_theme().into_tokens()));
            app.first_render().unwrap();
            app.press("label:打开设置").unwrap();
            app.wait_for("text:当前外观：暗色模式").unwrap();
            app.press("label:浅色模式").unwrap();
            app.press("label:橙色").unwrap();
        }
        {
            let mut app = open(Root::of::<LegacyProbe>());
            app.first_render().unwrap();
            app.wait_for("text:Light orange").unwrap();
        }
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("storage.json")).unwrap())
                .unwrap();
        assert_eq!(stored["oshwhub.themeMode"], "light");
        assert_eq!(stored["oshwhub.themeColor"], "orange");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn host_storage_keeps_theme_after_relaunch_without_an_environment_path() {
        let directory = std::env::temp_dir().join(format!(
            "oshwhub-host-preferences-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let open = || {
            Headless::start(EngineConfig {
                viewport: Size::new(px(390.), px(844.)),
                data_dir: Some(directory.clone()),
                ..EngineConfig::new(
                    Root::of::<StoredApp>().tokens(crate::app_theme().into_tokens()),
                )
            })
            .unwrap()
        };
        {
            let mut app = open();
            app.first_render().unwrap();
            app.press("label:打开设置").unwrap();
            app.press("label:暗色模式").unwrap();
            app.wait_for("text:当前外观：暗色模式").unwrap();
        }
        {
            let mut app = open();
            app.first_render().unwrap();
            app.press("label:打开设置").unwrap();
            app.wait_for("text:当前外观：暗色模式").unwrap();
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
