//! Einstellungen aller Anbindungen in `connectors.json` im Konfigurationsordner.
//! Ab Werk: eine Anbindung ist aus; ist sie an, sind lesende Fähigkeiten an und schreibende aus.

use super::{Cap, Descriptor};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const FILE: &str = "connectors.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectorConfig {
    /// an/aus; fehlt der Wert, gilt der Werkszustand der Anbindung (`Descriptor::default_enabled`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// ausdrücklich gesetzte Schalter; fehlt eine Fähigkeit, gilt der Werkszustand
    pub caps: BTreeMap<Cap, bool>,
    /// eigene Optionen der Anbindung (z.B. gewählte Sammlungen, eigene Client-ID)
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectorsConfig {
    pub connectors: BTreeMap<String, ConnectorConfig>,
}

impl ConnectorsConfig {
    /// Liest die Datei tolerant: fehlt sie oder ist sie kaputt, gilt der Werkszustand.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join(FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Schreibt atomar (temporäre Datei, dann umbenennen).
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(tmp, dir.join(FILE))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> ConnectorConfig {
        self.connectors.get(id).cloned().unwrap_or_default()
    }

    pub fn entry(&mut self, id: &str) -> &mut ConnectorConfig {
        self.connectors.entry(id.to_string()).or_default()
    }

    /// An oder aus: ausdrücklich gesetzt, sonst der Werkszustand der Anbindung
    pub fn is_enabled(&self, d: &Descriptor) -> bool {
        self.connectors.get(d.id).and_then(|c| c.enabled).unwrap_or(d.default_enabled)
    }

    /// Ist die Fähigkeit nutzbar? Nur wenn die Anbindung sie hat, die Anbindung an ist und der Schalter
    /// (bzw. Werkszustand: Lesen an, Schreiben aus) es erlaubt.
    pub fn cap_on(&self, d: &Descriptor, cap: Cap) -> bool {
        if !d.has(cap) {
            return false;
        }
        if !self.is_enabled(d) {
            return false;
        }
        self.connectors.get(d.id).and_then(|c| c.caps.get(&cap).copied()).unwrap_or(cap.default_on())
    }
}
