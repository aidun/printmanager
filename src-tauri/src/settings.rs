//! Einstellungen der App (liegen im Konfigurationsordner, nicht im Datenordner).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Ordner für Datenbank und Dateien. Leer = Standardordner der App.
    #[serde(default)]
    pub data_dir: Option<String>,
    /// Optional: eigene Datenbank, z.B. `postgres://user@nas/printmanager`. Leer = SQLite im Datenordner.
    #[serde(default)]
    pub database_url: Option<String>,
    /// Optional: Pfad zu Bambu Studio, falls es nicht am Standardort liegt.
    #[serde(default)]
    pub bambu_studio_path: Option<String>,
    /// Optional: Pfad zu `spools.json` von Bambu Studio, falls nicht am Standardort.
    #[serde(default)]
    pub spools_path: Option<String>,
}

impl Settings {
    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, serde_json::to_string_pretty(self)?)
    }

    pub fn data_dir(&self, default: &Path) -> PathBuf {
        self.data_dir.as_deref().filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| default.into())
    }
}
