use std::{
    cell::OnceCell,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use rusty_leveldb::{DB, Options};

use super::{AppError, FileStore, PreferenceStore, Result, THEME_COLOR_KEY, THEME_MODE_KEY};

pub struct DesktopStore {
    current: FileStore,
    legacy_directory: PathBuf,
    legacy: OnceCell<Result<[Option<String>; 2]>>,
}

impl DesktopStore {
    pub fn new(current: PathBuf, legacy_directory: PathBuf) -> Self {
        Self {
            current: FileStore::new(current),
            legacy_directory,
            legacy: OnceCell::new(),
        }
    }
}

impl PreferenceStore for DesktopStore {
    fn read(&self, key: &str) -> Result<Option<String>> {
        if let Some(value) = self.current.read(key)? {
            return Ok(Some(value));
        }
        let field = match key {
            THEME_MODE_KEY => 0,
            THEME_COLOR_KEY => 1,
            _ => return Ok(None),
        };
        let values = self.legacy.get_or_init(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                read_legacy(&self.legacy_directory)
            }))
            .unwrap_or_else(|_| Err(read_error("旧设置数据损坏")))
        });
        Ok(values.as_ref().map_err(Clone::clone)?[field].clone())
    }

    fn write(&mut self, key: &str, value: &str) -> Result<()> {
        self.current.write(key, value)
    }
}

fn read_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("PREFERENCES_READ", format!("无法读取原有外观偏好：{error}"))
}

fn read_legacy(directory: &Path) -> Result<[Option<String>; 2]> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok([None, None]),
        Err(error) => return Err(read_error(error)),
    };
    let options = Options {
        create_if_missing: false,
        paranoid_checks: true,
        automatic_compactions: false,
        ..rusty_leveldb::in_memory()
    };
    // shortcut: legacy snapshots are capped at 64 MiB; raise only after measuring migration cost.
    let mut remaining = 64 * 1024 * 1024;
    for entry in entries {
        let entry = entry.map_err(read_error)?;
        let name = entry.file_name();
        let path = Path::new(&name);
        if name != "CURRENT"
            && !name
                .to_str()
                .is_some_and(|name| name.starts_with("MANIFEST-"))
            && !matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("log" | "ldb" | "sst")
            )
        {
            continue;
        }
        if !entry.file_type().map_err(read_error)?.is_file() {
            return Err(read_error("旧设置包含非普通数据文件"));
        }
        if entry.metadata().map_err(read_error)?.len() > remaining {
            return Err(read_error("旧设置数据过大"));
        }
        let source = fs::File::open(entry.path()).map_err(read_error)?;
        let mut target = options
            .env
            .open_writable_file(&Path::new("legacy").join(name))
            .map_err(read_error)?;
        let copied =
            std::io::copy(&mut source.take(remaining + 1), &mut target).map_err(read_error)?;
        if copied > remaining {
            return Err(read_error("旧设置数据过大"));
        }
        remaining -= copied;
    }
    // LevelDB recovery may write; its entire environment is the memory copy.
    let mut database = DB::open("legacy", options).map_err(read_error)?;
    let snapshot = database.get_snapshot();
    let mut values = [None, None];
    for (field, key) in [THEME_MODE_KEY, THEME_COLOR_KEY].into_iter().enumerate() {
        let mut encoded = b"_oshwhub://app\0\x01".to_vec();
        encoded.extend_from_slice(key.as_bytes());
        values[field] = database
            .get_at(&snapshot, &encoded)
            .map_err(read_error)?
            .as_deref()
            .and_then(decode_value);
    }
    values[0] = values[0]
        .take()
        .filter(|value| super::parse_appearance(value).is_some());
    values[1] = values[1]
        .take()
        .filter(|value| super::parse_accent(value).is_some());
    Ok(values)
}

fn decode_value(value: &[u8]) -> Option<String> {
    // The longest accepted enum is seven characters (plus Chromium's encoding byte).
    if value.len() > 15 {
        return None;
    }
    let (&encoding, value) = value.split_first()?;
    match encoding {
        1 => Some(value.iter().map(|&byte| char::from(byte)).collect()),
        0 if value.len().is_multiple_of(2) => String::from_utf16(
            &value
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>(),
        )
        .ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::{THEME_COLOR_KEY, THEME_MODE_KEY};
    use std::{
        collections::BTreeMap,
        fs,
        path::Path,
        sync::atomic::{AtomicU64, Ordering},
    };

    fn directory() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "oshwhub-desktop-theme-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn entries(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name().into(), fs::read(entry.path()).unwrap())
            })
            .collect()
    }

    #[test]
    fn missing_legacy_database_is_a_normal_first_install() {
        let directory = directory();
        let legacy = directory.join("absent");
        let store = DesktopStore::new(directory.join("preferences.json"), legacy.clone());
        assert_eq!(store.read(THEME_MODE_KEY).unwrap(), None);
        assert_eq!(store.read(THEME_COLOR_KEY).unwrap(), None);
        assert!(!legacy.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn damaged_deletion_log_never_resurrects_an_old_theme() {
        let directory = directory();
        let legacy = directory.join("legacy");
        fs::create_dir(&legacy).unwrap();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/electron-theme/deleted");
        for (name, mut bytes) in entries(&fixture) {
            if name == Path::new("000003.log") {
                bytes[407] ^= 1;
            }
            fs::write(legacy.join(name), bytes).unwrap();
        }
        let store = DesktopStore::new(directory.join("preferences.json"), legacy);
        assert!(
            store.read(THEME_MODE_KEY).is_err(),
            "a corrupt deletion must be visible, not a stale value"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    fn compacted_fixture(directory: &Path) -> PathBuf {
        let legacy = directory.join("legacy");
        fs::create_dir(&legacy).unwrap();
        let options = rusty_leveldb::in_memory();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/electron-theme/saved");
        for (name, bytes) in entries(&fixture) {
            use std::io::Write;
            options
                .env
                .open_writable_file(&Path::new("legacy").join(name))
                .unwrap()
                .write_all(&bytes)
                .unwrap();
        }
        let mut database = DB::open("legacy", options.clone()).unwrap();
        database.compact_range(b"", b"\xff").unwrap();
        database.close().unwrap();
        for path in options.env.children(Path::new("legacy")).unwrap() {
            let name = path.file_name().unwrap();
            let mut bytes = Vec::new();
            options
                .env
                .open_sequential_file(&Path::new("legacy").join(name))
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            fs::write(legacy.join(name), bytes).unwrap();
        }
        legacy
    }

    fn damaged_sst_is_a_visible_read_error(footer: bool) {
        let directory = directory();
        let legacy = compacted_fixture(&directory);
        let table = fs::read_dir(&legacy)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|value| value == "ldb"))
            .expect("compaction must create a live SST");
        let mut bytes = fs::read(&table).unwrap();
        let at = if footer {
            bytes.len() - 1
        } else {
            bytes
                .windows(6)
                .position(|value| value == b"purple")
                .expect("uncompressed table contains the theme")
        };
        bytes[at] ^= 1;
        fs::write(table, bytes).unwrap();
        let store = DesktopStore::new(directory.join("preferences.json"), legacy);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| store.read(THEME_COLOR_KEY)));
        assert!(
            matches!(result, Ok(Err(_))),
            "corrupt SST must report a read error without panicking: {result:?}"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn damaged_sst_checksum_is_a_visible_read_error() {
        damaged_sst_is_a_visible_read_error(false);
    }

    #[test]
    fn damaged_sst_footer_is_a_visible_read_error() {
        damaged_sst_is_a_visible_read_error(true);
    }

    #[test]
    fn chromium_preferences_respect_origin_latest_values_and_deletions_without_writes() {
        for (fixture, mode) in [("saved", Some("dark")), ("deleted", None)] {
            let legacy = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/electron-theme")
                .join(fixture);
            let before = entries(&legacy);
            let directory = directory();
            let store = DesktopStore::new(directory.join("preferences.json"), legacy.clone());
            assert_eq!(store.read(THEME_MODE_KEY).unwrap().as_deref(), mode);
            assert_eq!(
                store.read(THEME_COLOR_KEY).unwrap().as_deref(),
                Some("purple")
            );
            assert_eq!(
                entries(&legacy),
                before,
                "legacy files must remain unchanged"
            );
            assert!(!directory.join("preferences.json").exists());
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn broken_legacy_source_does_not_block_existing_preferences_or_new_saves() {
        let directory = directory();
        let legacy = directory.join("legacy");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("CURRENT"), "missing-manifest\n").unwrap();
        let path = directory.join("preferences.json");
        fs::write(&path, r#"{"oshwhub.themeMode":"light","unrelated":7}"#).unwrap();
        let mut store = DesktopStore::new(path.clone(), legacy);
        assert_eq!(
            store.read(THEME_MODE_KEY).unwrap().as_deref(),
            Some("light")
        );
        assert!(store.read(THEME_COLOR_KEY).is_err());
        store.write(THEME_COLOR_KEY, "orange").unwrap();
        assert_eq!(
            store.read(THEME_COLOR_KEY).unwrap().as_deref(),
            Some("orange")
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()["unrelated"],
            7
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn chromium_strings_decode_latin1_and_utf16_without_accepting_corrupt_values() {
        assert_eq!(decode_value(b"\x01dark").as_deref(), Some("dark"));
        assert_eq!(decode_value(b"\0d\0a\0r\0k\0").as_deref(), Some("dark"));
        assert_eq!(decode_value(b"\0d"), None);
        assert_eq!(decode_value(b"\x02dark"), None);
        assert_eq!(decode_value(b"\x01value-too-large-for-an-enum"), None);
    }

    #[test]
    fn oversized_legacy_data_is_rejected_before_database_recovery() {
        let directory = directory();
        let legacy = directory.join("legacy");
        fs::create_dir(&legacy).unwrap();
        fs::File::create(legacy.join("000099.log"))
            .unwrap()
            .set_len(64 * 1024 * 1024 + 1)
            .unwrap();
        let store = DesktopStore::new(directory.join("preferences.json"), legacy);
        assert!(
            store
                .read(THEME_MODE_KEY)
                .unwrap_err()
                .message
                .contains("过大")
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
