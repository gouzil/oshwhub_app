use nsh_core::platform::Appearance;
#[cfg(any(target_os = "android", test))]
mod android;
mod platform;
use oshwhub_core::error::{AppError, Result};
pub use platform::system_store;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const THEME_MODE_KEY: &str = "oshwhub.themeMode";
pub const THEME_COLOR_KEY: &str = "oshwhub.themeColor";

pub const APPEARANCE_CHOICES: [(Appearance, &str); 3] = [
    (Appearance::System, "跟随系统"),
    (Appearance::Light, "亮色模式"),
    (Appearance::Dark, "暗色模式"),
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Oshwhub,
    Purple,
    Orange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentPalette {
    pub primary: &'static str,
    pub light: &'static str,
    pub dark_primary: &'static str,
    pub dark: &'static str,
}

impl Accent {
    pub const ALL: [Self; 3] = [Self::Oshwhub, Self::Purple, Self::Orange];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Oshwhub => "oshwhub",
            Self::Purple => "purple",
            Self::Orange => "orange",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Oshwhub => "OSHWHub 蓝",
            Self::Purple => "紫色",
            Self::Orange => "橙色",
        }
    }

    pub const fn palette(self) -> AccentPalette {
        match self {
            Self::Oshwhub => AccentPalette {
                primary: "#2c65e8",
                light: "#edf3ff",
                dark_primary: "#8bb3ff",
                dark: "#1d2d4b",
            },
            Self::Purple => AccentPalette {
                primary: "#7654d1",
                light: "#f4efff",
                dark_primary: "#bfa5ff",
                dark: "#302342",
            },
            Self::Orange => AccentPalette {
                primary: "#b45309",
                light: "#fff4e6",
                dark_primary: "#ffbd7a",
                dark: "#3b281a",
            },
        }
    }
}

fn parse_appearance(value: &str) -> Option<Appearance> {
    match value {
        "system" => Some(Appearance::System),
        "light" => Some(Appearance::Light),
        "dark" => Some(Appearance::Dark),
        _ => None,
    }
}

fn appearance_id(value: Appearance) -> &'static str {
    match value {
        Appearance::System => "system",
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    }
}

fn parse_accent(value: &str) -> Option<Accent> {
    Accent::ALL.into_iter().find(|accent| accent.id() == value)
}

pub trait PreferenceStore {
    fn read(&self, key: &str) -> Result<Option<String>>;
    fn write(&mut self, key: &str, value: &str) -> Result<()>;
}

impl<T: PreferenceStore> PreferenceStore for Result<T> {
    fn read(&self, key: &str) -> Result<Option<String>> {
        self.as_ref().map_err(Clone::clone)?.read(key)
    }

    fn write(&mut self, key: &str, value: &str) -> Result<()> {
        self.as_mut()
            .map_err(|error| error.clone())?
            .write(key, value)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FileStore {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn entries(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| AppError::new("PREFERENCES_READ", error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
            Err(error) => Err(AppError::new("PREFERENCES_READ", error.to_string())),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PreferenceStore for FileStore {
    fn read(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .entries()?
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned))
    }

    fn write(&mut self, key: &str, value: &str) -> Result<()> {
        // shortcut: concurrent processes can lose updates; add locking before supporting multiple writers.
        let mut entries = self.entries()?;
        let permissions = match fs::metadata(&self.path) {
            Ok(metadata) => {
                if metadata.permissions().readonly() {
                    return Err(AppError::new(
                        "PREFERENCES_SAVE",
                        "外观偏好文件为只读，未保存更改",
                    ));
                }
                Some(metadata.permissions())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(AppError::new("PREFERENCES_SAVE", error.to_string())),
        };
        entries.insert(key.into(), serde_json::Value::String(value.into()));
        let bytes = serde_json::to_vec(&entries)
            .map_err(|error| AppError::new("PREFERENCES_SAVE", error.to_string()))?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| AppError::new("PREFERENCES_SAVE", error.to_string()))?;
        let mut name = self
            .path
            .file_name()
            .ok_or_else(|| AppError::new("PREFERENCES_SAVE", "外观偏好文件名无效"))?
            .to_os_string();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        name.push(format!(
            ".tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&temporary)
            .map_err(|error| AppError::new("PREFERENCES_SAVE", error.to_string()))?;
        let result = (|| -> std::io::Result<()> {
            let mut file = file;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions)?;
            }
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)?;
            #[cfg(unix)]
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|error| AppError::new("PREFERENCES_SAVE", error.to_string()))
    }
}

#[cfg(target_arch = "wasm32")]
pub struct WebStore;

#[cfg(target_arch = "wasm32")]
impl PreferenceStore for WebStore {
    fn read(&self, key: &str) -> Result<Option<String>> {
        web_storage()?.get_item(key).map_err(|error| {
            AppError::new("PREFERENCES_READ", format!("无法读取外观偏好：{error:?}"))
        })
    }

    fn write(&mut self, key: &str, value: &str) -> Result<()> {
        web_storage()?.set_item(key, value).map_err(|error| {
            AppError::new("PREFERENCES_SAVE", format!("无法保存外观偏好：{error:?}"))
        })
    }
}

#[cfg(target_arch = "wasm32")]
fn web_storage() -> Result<web_sys::Storage> {
    web_sys::window()
        .ok_or_else(|| AppError::new("PREFERENCES_UNAVAILABLE", "浏览器窗口不可用"))?
        .local_storage()
        .map_err(|error| {
            AppError::new(
                "PREFERENCES_UNAVAILABLE",
                format!("无法访问外观偏好：{error:?}"),
            )
        })?
        .ok_or_else(|| AppError::new("PREFERENCES_UNAVAILABLE", "浏览器存储不可用"))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThemePreferences {
    pub appearance: Appearance,
    pub accent: Accent,
}

impl ThemePreferences {
    pub fn from_values(appearance: Option<&str>, accent: Option<&str>) -> Self {
        Self {
            appearance: appearance.and_then(parse_appearance).unwrap_or_default(),
            accent: accent.and_then(parse_accent).unwrap_or_default(),
        }
    }

    pub fn load(store: &impl PreferenceStore) -> Result<Self> {
        let appearance = store.read(THEME_MODE_KEY)?;
        let accent = store.read(THEME_COLOR_KEY)?;
        Ok(Self::from_values(appearance.as_deref(), accent.as_deref()))
    }

    pub fn set_appearance(
        &mut self,
        store: &mut impl PreferenceStore,
        appearance: Appearance,
    ) -> Result<()> {
        self.appearance = appearance;
        store.write(THEME_MODE_KEY, appearance_id(appearance))
    }

    pub fn set_accent(&mut self, store: &mut impl PreferenceStore, accent: Accent) -> Result<()> {
        self.accent = accent;
        store.write(THEME_COLOR_KEY, accent.id())
    }
}
