use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOpenMode {
    #[default]
    SameWindow,
    SeparateWindow,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub file_open_mode: FileOpenMode,
}

impl AppSettings {
    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    fn load_from(path: &Path) -> Self {
        let Ok(contents) = std::fs::read(path) else {
            return Self::default();
        };
        serde_json::from_slice(&contents).unwrap_or_default()
    }

    fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create settings directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let contents = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Failed to serialize settings: {error}"))?;
        let temporary_path = path.with_extension("json.tmp");
        std::fs::write(&temporary_path, contents).map_err(|error| {
            format!(
                "Failed to write settings file '{}': {error}",
                temporary_path.display()
            )
        })?;

        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path).map_err(|error| {
                format!(
                    "Failed to replace settings file '{}': {error}",
                    path.display()
                )
            })?;
        }

        std::fs::rename(&temporary_path, path)
            .map_err(|error| format!("Failed to save settings file '{}': {error}", path.display()))
    }

    fn path() -> PathBuf {
        crate::context_menu::ContextMenuManager::config_dir().join("settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, FileOpenMode};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempSettings {
        root: std::path::PathBuf,
    }

    impl TempSettings {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after Unix epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "paa-viewer-settings-{}-{nonce}",
                std::process::id()
            ));
            Self { root }
        }

        fn path(&self) -> std::path::PathBuf {
            self.root.join("settings.json")
        }
    }

    impl Drop for TempSettings {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn separate_window_preference_survives_restart() {
        let temp = TempSettings::new();
        let settings = AppSettings {
            file_open_mode: FileOpenMode::SeparateWindow,
        };

        settings
            .save_to(&temp.path())
            .expect("settings should be saved");

        assert_eq!(AppSettings::load_from(&temp.path()), settings);
    }

    #[test]
    fn unreadable_settings_fall_back_to_same_window() {
        let temp = TempSettings::new();
        std::fs::create_dir_all(&temp.root).expect("temporary directory should be created");
        std::fs::write(temp.path(), b"not json").expect("invalid settings should be written");

        assert_eq!(AppSettings::load_from(&temp.path()), AppSettings::default());
    }
}
