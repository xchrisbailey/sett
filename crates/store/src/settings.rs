use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Space;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorMode {
    /// The Notion-style block view (ADR 0003).
    #[default]
    Block,
    /// Plain Markdown source.
    Raw,
}

/// App settings stored in `settings.toml`, outside the data root.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    pub data_root: PathBuf,
    pub editor_mode: EditorMode,
    pub last_space: Space,
}

impl Settings {
    /// `~/Library/Application Support/sett/settings.toml`, or `None` when
    /// `$HOME` isn't set.
    pub fn default_path() -> Option<PathBuf> {
        Some(home_dir()?.join("Library/Application Support/sett/settings.toml"))
    }

    pub fn defaults(home: &Path) -> Settings {
        Settings {
            data_root: home.join("sett"),
            editor_mode: EditorMode::default(),
            last_space: Space::Personal,
        }
    }

    /// Never fails: a missing or unreadable file gives defaults, and each
    /// field that is missing or invalid falls back on its own so one bad
    /// hand edit doesn't reset the rest.
    pub fn load(path: &Path, home: &Path) -> Settings {
        let mut settings = Settings::defaults(home);
        let Some(table) = fs::read_to_string(path)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
        else {
            return settings;
        };
        if let Some(v) = field(&table, "data_root") {
            settings.data_root = v;
        }
        if let Some(v) = field(&table, "editor_mode") {
            settings.editor_mode = v;
        }
        if let Some(v) = field(&table, "last_space") {
            settings.last_space = v;
        }
        settings
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self).map_err(io::Error::other)?;
        fs::write(path, text)
    }
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn field<T: DeserializeOwned>(table: &toml::Table, key: &str) -> Option<T> {
    table.get(key)?.clone().try_into().ok()
}
