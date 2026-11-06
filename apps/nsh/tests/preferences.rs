use oshwhub_nsh::preferences;

use nsh_core::platform::Appearance;
use oshwhub_core::error::{AppError, Result};
use preferences::{Accent, FileStore, PreferenceStore, ThemePreferences};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct Store {
    entries: BTreeMap<String, String>,
    fail_read: bool,
    fail_write: Option<&'static str>,
}

impl PreferenceStore for Store {
    fn read(&self, key: &str) -> Result<Option<String>> {
        if self.fail_read {
            return Err(AppError::new("read_failure", "无法读取存储"));
        }
        Ok(self.entries.get(key).cloned())
    }

    fn write(&mut self, key: &str, value: &str) -> Result<()> {
        if self.fail_write == Some(key) {
            return Err(AppError::new("write_failure", "无法写入存储"));
        }
        self.entries.insert(key.into(), value.into());
        Ok(())
    }
}

fn store(entries: &[(&str, &str)]) -> Store {
    Store {
        entries: entries
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect(),
        ..Store::default()
    }
}

#[test]
fn parses_existing_appearance_and_accent_values_exactly() {
    for (mode, expected) in [
        ("system", Appearance::System),
        ("light", Appearance::Light),
        ("dark", Appearance::Dark),
    ] {
        assert_eq!(
            ThemePreferences::from_values(Some(mode), None).appearance,
            expected
        );
    }
    for (color, expected, light, dark) in [
        ("oshwhub", Accent::Oshwhub, "#2c65e8", "#8bb3ff"),
        ("purple", Accent::Purple, "#7654d1", "#bfa5ff"),
        ("orange", Accent::Orange, "#b45309", "#ffbd7a"),
    ] {
        let parsed = ThemePreferences::from_values(None, Some(color));
        assert_eq!(parsed.accent, expected);
        assert_eq!(parsed.accent.id(), color);
        assert_eq!(parsed.accent.palette().primary, light);
        assert_eq!(parsed.accent.palette().dark_primary, dark);
    }
}

#[test]
fn missing_unknown_or_corrupt_values_fall_back_without_normalizing() {
    for value in [
        None,
        Some(""),
        Some("DARK"),
        Some(" dark "),
        Some("{broken"),
        Some("\0"),
    ] {
        let preferences = ThemePreferences::from_values(value, value);
        assert_eq!(preferences.appearance, Appearance::System);
        assert_eq!(preferences.accent, Accent::Oshwhub);
    }
    let partial = ThemePreferences::from_values(Some("future-mode"), Some("orange"));
    assert_eq!(partial.appearance, Appearance::System);
    assert_eq!(partial.accent, Accent::Orange);
}

#[test]
fn loading_reads_original_keys_and_preserves_unknown_data() {
    let saved = store(&[
        ("oshwhub.themeMode", "dark"),
        ("oshwhub.themeColor", "purple"),
        ("other.application", "keep me"),
    ]);
    let loaded = ThemePreferences::load(&saved).unwrap();
    assert_eq!(loaded.appearance, Appearance::Dark);
    assert_eq!(loaded.accent, Accent::Purple);
    assert_eq!(saved.entries["other.application"], "keep me");

    let corrupt = store(&[
        ("oshwhub.themeMode", "future-mode"),
        ("oshwhub.themeColor", "{bad json"),
    ]);
    let loaded = ThemePreferences::load(&corrupt).unwrap();
    assert_eq!(loaded.appearance, Appearance::System);
    assert_eq!(loaded.accent, Accent::Oshwhub);
    assert_eq!(corrupt.entries["oshwhub.themeMode"], "future-mode");
    assert_eq!(corrupt.entries["oshwhub.themeColor"], "{bad json");
    assert_eq!(corrupt.entries.len(), 2);
}

#[test]
fn explicit_selections_save_only_the_changed_preference() {
    let mut saved = store(&[
        ("oshwhub.themeMode", "light"),
        ("oshwhub.themeColor", "future-accent"),
        ("other.application", "keep me"),
    ]);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    preferences
        .set_appearance(&mut saved, Appearance::Dark)
        .unwrap();
    assert_eq!(preferences.appearance, Appearance::Dark);
    assert_eq!(saved.entries["oshwhub.themeMode"], "dark");
    assert_eq!(saved.entries["oshwhub.themeColor"], "future-accent");
    preferences.set_accent(&mut saved, Accent::Orange).unwrap();
    assert_eq!(preferences.accent, Accent::Orange);
    assert_eq!(saved.entries["oshwhub.themeColor"], "orange");
    assert_eq!(saved.entries["oshwhub.themeMode"], "dark");
    assert_eq!(saved.entries["other.application"], "keep me");
    preferences
        .set_appearance(&mut saved, Appearance::System)
        .unwrap();
    assert_eq!(saved.entries["oshwhub.themeMode"], "system");
}

#[test]
fn failed_persistence_keeps_the_applied_selection_and_reports_failure() {
    let mut saved = store(&[
        ("oshwhub.themeMode", "light"),
        ("oshwhub.themeColor", "oshwhub"),
    ]);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    saved.fail_write = Some("oshwhub.themeMode");
    let error = preferences
        .set_appearance(&mut saved, Appearance::Dark)
        .unwrap_err();
    assert_eq!(error.code, "write_failure");
    assert_eq!(preferences.appearance, Appearance::Dark);
    assert_eq!(saved.entries["oshwhub.themeMode"], "light");
    saved.fail_write = Some("oshwhub.themeColor");
    let error = preferences
        .set_accent(&mut saved, Accent::Purple)
        .unwrap_err();
    assert_eq!(error.code, "write_failure");
    assert_eq!(preferences.accent, Accent::Purple);
    assert_eq!(saved.entries["oshwhub.themeColor"], "oshwhub");
    saved.fail_write = None;
    preferences.set_accent(&mut saved, Accent::Purple).unwrap();
    assert_eq!(saved.entries["oshwhub.themeColor"], "purple");
}

#[test]
fn failed_reads_are_visible_and_do_not_become_saved_defaults() {
    let mut saved = store(&[("oshwhub.themeMode", "dark")]);
    saved.fail_read = true;
    assert_eq!(
        ThemePreferences::load(&saved).unwrap_err().code,
        "read_failure"
    );
    assert_eq!(saved.entries["oshwhub.themeMode"], "dark");
    assert_eq!(saved.entries.len(), 1);
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "oshwhub-preferences-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_file_is_read_without_writes_and_explicit_changes_survive_reload() {
    let directory = TempDirectory::new();
    let path = directory.0.join("nested/preferences.json");
    let mut saved = FileStore::new(&path);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    assert_eq!(preferences.appearance, Appearance::System);
    assert_eq!(preferences.accent, Accent::Oshwhub);
    assert!(!path.parent().unwrap().exists());
    preferences
        .set_appearance(&mut saved, Appearance::Dark)
        .unwrap();
    preferences.set_accent(&mut saved, Accent::Purple).unwrap();
    let loaded = ThemePreferences::load(&FileStore::new(&path)).unwrap();
    assert_eq!(loaded.appearance, Appearance::Dark);
    assert_eq!(loaded.accent, Accent::Purple);
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        data,
        json!({"oshwhub.themeMode": "dark", "oshwhub.themeColor": "purple"})
    );
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn file_load_preserves_unknown_values_and_field_writes_keep_unrelated_json() {
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    let original = br#"{"oshwhub.themeMode":"future-mode","oshwhub.themeColor":"purple","unrelated":{"a":[1,null,true]}}"#;
    fs::write(&path, original).unwrap();
    let mut saved = FileStore::new(&path);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    assert_eq!(preferences.appearance, Appearance::System);
    assert_eq!(preferences.accent, Accent::Purple);
    assert_eq!(fs::read(&path).unwrap(), original);
    preferences.set_accent(&mut saved, Accent::Orange).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        data,
        json!({
            "oshwhub.themeMode": "future-mode", "oshwhub.themeColor": "orange",
            "unrelated": {"a": [1, null, true]},
        })
    );
}

#[test]
fn each_file_write_rereads_other_settings_changed_after_loading() {
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    let mut saved = FileStore::new(&path);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    fs::write(&path, br#"{"oshwhub.themeMode":"dark","external":42}"#).unwrap();
    preferences.set_accent(&mut saved, Accent::Purple).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        data,
        json!({"oshwhub.themeMode": "dark", "oshwhub.themeColor": "purple", "external": 42})
    );
}

#[test]
fn malformed_files_report_errors_and_are_never_replaced_by_defaults() {
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    for original in ["", "{broken", "[]", "null", "42"] {
        fs::write(&path, original).unwrap();
        let mut saved = FileStore::new(&path);
        assert_eq!(
            ThemePreferences::load(&saved).unwrap_err().code,
            "PREFERENCES_READ"
        );
        let mut preferences = ThemePreferences::default();
        assert_eq!(
            preferences
                .set_appearance(&mut saved, Appearance::Dark)
                .unwrap_err()
                .code,
            "PREFERENCES_READ"
        );
        assert_eq!(preferences.appearance, Appearance::Dark);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[test]
fn non_string_file_values_fall_back_without_changing_the_file() {
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    let original =
        br#"{"oshwhub.themeMode":99,"oshwhub.themeColor":{"id":"purple"},"future":[true]}"#;
    fs::write(&path, original).unwrap();
    let mut saved = FileStore::new(&path);
    let mut preferences = ThemePreferences::load(&saved).unwrap();
    assert_eq!(preferences.appearance, Appearance::System);
    assert_eq!(preferences.accent, Accent::Oshwhub);
    assert_eq!(fs::read(&path).unwrap(), original);
    preferences
        .set_appearance(&mut saved, Appearance::Light)
        .unwrap();
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        data,
        json!({"oshwhub.themeMode": "light", "oshwhub.themeColor": {"id": "purple"}, "future": [true]})
    );
}

#[test]
fn file_io_errors_are_visible_and_leave_existing_data_intact() {
    let directory = TempDirectory::new();
    let blocked = directory.0.join("blocked");
    fs::write(&blocked, "keep me").unwrap();
    let mut saved = FileStore::new(blocked.join("preferences.json"));
    assert_eq!(
        ThemePreferences::load(&saved).unwrap_err().code,
        "PREFERENCES_READ"
    );
    assert!(
        ThemePreferences::default()
            .set_accent(&mut saved, Accent::Orange)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&blocked).unwrap(), "keep me");
}

#[test]
fn read_only_preference_files_are_not_replaced() {
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    let original = br#"{"oshwhub.themeMode":"light"}"#;
    fs::write(&path, original).unwrap();
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).unwrap();
    let mut preferences = ThemePreferences::load(&FileStore::new(&path)).unwrap();
    let result = preferences.set_appearance(&mut FileStore::new(&path), Appearance::Dark);
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(result.unwrap_err().code, "PREFERENCES_SAVE");
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(preferences.appearance, Appearance::Dark);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn atomic_replacement_keeps_private_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TempDirectory::new();
    let path = directory.0.join("preferences.json");
    fs::write(&path, b"{}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    ThemePreferences::default()
        .set_accent(&mut FileStore::new(&path), Accent::Orange)
        .unwrap();
    assert_eq!(
        ThemePreferences::load(&FileStore::new(&path))
            .unwrap()
            .accent,
        Accent::Orange
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
