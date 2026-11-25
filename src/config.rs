//! Small JSON settings file stored in the launcher's data folder.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub last_version: String,
    /// Path to a Java executable. On Android this must point to a bundled runtime.
    pub java_path: String,
    pub ram_mb: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: String::new(),
            last_version: String::new(),
            java_path: "java".into(),
            ram_mb: 1536,
        }
    }
}

impl Config {
    fn file(root: &Path) -> PathBuf {
        root.join("launcher.json")
    }

    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(Self::file(root))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(root)?;
        std::fs::write(Self::file(root), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
